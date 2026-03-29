"""Tests for MovingAverageCrossover strategy."""

import numpy as np
import pandas as pd
import pytest

from strategies.base import BaseStrategy, Signal, BacktestResult
from strategies.strategy_moving_average import MovingAverageCrossover


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def strategy():
    return MovingAverageCrossover(fast_period=3, slow_period=5)


@pytest.fixture
def default_strategy():
    return MovingAverageCrossover()


def _make_bars(prices: list[float]) -> pd.DataFrame:
    """Build a minimal OHLCV DataFrame from a list of close prices."""
    n = len(prices)
    return pd.DataFrame({
        "timestamp": pd.date_range("2025-01-01", periods=n, freq="D"),
        "open": prices,
        "high": [p * 1.01 for p in prices],
        "low": [p * 0.99 for p in prices],
        "close": prices,
        "volume": [1000] * n,
    })


# ---------------------------------------------------------------------------
# Interface compliance
# ---------------------------------------------------------------------------

class TestBaseStrategyContract:
    def test_extends_base_strategy(self):
        assert issubclass(MovingAverageCrossover, BaseStrategy)

    def test_name_property(self, strategy):
        assert strategy.name == "MovingAverageCrossover"

    def test_params_returns_dict(self, strategy):
        p = strategy.params()
        assert isinstance(p, dict)
        assert "fast_period" in p
        assert "slow_period" in p

    def test_default_params(self, default_strategy):
        assert default_strategy.params() == {"fast_period": 10, "slow_period": 30}

    def test_custom_params(self, strategy):
        assert strategy.params() == {"fast_period": 3, "slow_period": 5}


# ---------------------------------------------------------------------------
# Signal generation
# ---------------------------------------------------------------------------

class TestGenerateSignal:
    def test_insufficient_bars_returns_hold(self, strategy):
        bars = _make_bars([100.0] * 4)  # need slow_period+1 = 6
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "HOLD"
        assert signal.confidence == 0.0
        assert "Insufficient" in signal.reason

    def test_signal_returns_correct_type(self, strategy):
        bars = _make_bars([100.0] * 10)
        signal = strategy.generate_signal(bars, "SPY")
        assert isinstance(signal, Signal)
        assert signal.symbol == "SPY"
        assert signal.strategy_name == "MovingAverageCrossover"
        assert signal.direction in ("BUY", "SELL", "HOLD")
        assert 0.0 <= signal.confidence <= 1.0
        assert len(signal.timestamp) > 0

    def test_buy_signal_on_crossover(self, strategy):
        # fast=3, slow=5. Need crossover to happen exactly on the last bar.
        # At i=7: fast(92.33) < slow(92.60) — fast below slow
        # At i=8: fast(95.33) > slow(93.60) — fast crosses above slow
        prices = [100, 98, 96, 94, 92, 90, 92, 95, 99]
        bars = _make_bars(prices)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "BUY"
        assert signal.confidence > 0.5
        assert "crossed above" in signal.reason

    def test_sell_signal_on_crossunder(self, strategy):
        # Inverse: fast above slow, then fast crosses below on the last bar.
        # At i=7: fast(107.67) > slow(105.40) — fast above slow
        # At i=8: fast(104.67) < slow(106.40) — fast crosses below slow
        prices = [90, 92, 95, 98, 101, 104, 110, 109, 80]
        bars = _make_bars(prices)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "SELL"
        assert signal.confidence > 0.5
        assert "crossed below" in signal.reason

    def test_hold_signal_in_flat_market(self, strategy):
        prices = [100.0] * 10
        bars = _make_bars(prices)
        signal = strategy.generate_signal(bars, "FLAT")
        assert signal.direction == "HOLD"

    def test_confidence_capped_at_one(self, strategy):
        # Extreme divergence should still cap confidence at 1.0
        prices = [10, 10, 10, 10, 10, 10, 20, 40, 80, 160]
        bars = _make_bars(prices)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.confidence <= 1.0

    def test_signal_symbol_matches_input(self, strategy):
        bars = _make_bars([100.0] * 10)
        signal = strategy.generate_signal(bars, "AAPL")
        assert signal.symbol == "AAPL"


# ---------------------------------------------------------------------------
# Backtesting
# ---------------------------------------------------------------------------

class TestBacktest:
    @pytest.fixture
    def backtest_bars(self):
        """Generate synthetic price data with clear trends for backtesting."""
        np.random.seed(42)
        n = 200
        # Create trending data with mean reversion
        prices = [100.0]
        for i in range(1, n):
            trend = 0.05 * np.sin(i / 20)  # oscillating trend
            noise = np.random.normal(0, 0.5)
            prices.append(prices[-1] * (1 + trend + noise / 100))
        return _make_bars(prices)

    def test_backtest_returns_correct_type(self, strategy, backtest_bars):
        result = strategy.backtest(backtest_bars, "TEST")
        assert isinstance(result, BacktestResult)

    def test_backtest_result_fields(self, strategy, backtest_bars):
        result = strategy.backtest(backtest_bars, "TEST")
        assert result.strategy_name == "MovingAverageCrossover"
        assert result.symbol == "TEST"
        assert isinstance(result.total_return_pct, float)
        assert isinstance(result.sharpe_ratio, float)
        assert isinstance(result.max_drawdown_pct, float)
        assert isinstance(result.win_rate, float)
        assert isinstance(result.total_trades, int)
        assert isinstance(result.profit_factor, float)
        assert result.period_start is not None
        assert result.period_end is not None

    def test_backtest_win_rate_in_range(self, strategy, backtest_bars):
        result = strategy.backtest(backtest_bars, "TEST")
        assert 0.0 <= result.win_rate <= 1.0

    def test_backtest_max_drawdown_non_negative(self, strategy, backtest_bars):
        result = strategy.backtest(backtest_bars, "TEST")
        # vectorbt reports max drawdown as a positive percentage (magnitude of loss)
        assert result.max_drawdown_pct >= 0.0

    def test_backtest_generates_trades(self, strategy, backtest_bars):
        result = strategy.backtest(backtest_bars, "TEST")
        assert result.total_trades > 0

    def test_backtest_with_real_data(self, default_strategy):
        """Run backtest against actual SQLite data if available."""
        try:
            import sqlite3
            import os
            db_path = os.environ.get(
                "DB_PATH",
                os.environ.get("DUCKDB_PATH",
                               str(__import__("pathlib").Path(__file__).resolve().parent.parent.parent / "data" / "algotrader.sqlite")),
            )
            con = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
            rows = con.execute(
                "SELECT symbol, timestamp, open, high, low, close, volume "
                "FROM ohlcv_bars WHERE symbol = 'SPY' AND bar_size = '1d' ORDER BY timestamp"
            ).fetchall()
            con.close()
            if not rows:
                pytest.skip("No SPY data in database")
            import pandas as pd
            bars = pd.DataFrame(rows, columns=["symbol", "timestamp", "open", "high", "low", "close", "volume"])
        except Exception:
            pytest.skip("Database not available")

        result = default_strategy.backtest(bars, "SPY")
        assert result.total_trades > 0
        assert result.strategy_name == "MovingAverageCrossover"
        assert result.symbol == "SPY"
        # SPY over 2 years with MA crossover should have some return
        assert result.total_return_pct != 0.0

    def test_backtest_to_dict(self, strategy, backtest_bars):
        result = strategy.backtest(backtest_bars, "TEST")
        d = result.to_dict()
        assert isinstance(d, dict)
        assert d["strategy_name"] == "MovingAverageCrossover"
        assert "total_return_pct" in d
        assert "sharpe_ratio" in d
