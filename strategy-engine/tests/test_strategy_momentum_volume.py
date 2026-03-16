"""Tests for MomentumVolume strategy."""

import numpy as np
import pandas as pd
import pytest

from strategies.base import BaseStrategy, Signal, BacktestResult
from strategies.strategy_momentum_volume import MomentumVolume


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def strategy():
    return MomentumVolume(lookback=5, volume_multiplier=1.5)


@pytest.fixture
def default_strategy():
    return MomentumVolume()


def _make_bars(prices: list[float], volumes: list[int] | None = None) -> pd.DataFrame:
    """Build a minimal OHLCV DataFrame from close prices and optional volumes."""
    n = len(prices)
    if volumes is None:
        volumes = [1000] * n
    return pd.DataFrame({
        "timestamp": pd.date_range("2025-01-01", periods=n, freq="D"),
        "open": prices,
        "high": [p * 1.02 for p in prices],
        "low": [p * 0.98 for p in prices],
        "close": prices,
        "volume": volumes,
    })


# ---------------------------------------------------------------------------
# Interface compliance
# ---------------------------------------------------------------------------

class TestBaseStrategyContract:
    def test_extends_base_strategy(self):
        assert issubclass(MomentumVolume, BaseStrategy)

    def test_name_property(self, strategy):
        assert strategy.name == "MomentumVolume"

    def test_params_returns_dict(self, strategy):
        p = strategy.params()
        assert isinstance(p, dict)
        assert "lookback" in p
        assert "volume_multiplier" in p

    def test_default_params(self, default_strategy):
        assert default_strategy.params() == {"lookback": 20, "volume_multiplier": 1.5}

    def test_custom_params(self, strategy):
        assert strategy.params() == {"lookback": 5, "volume_multiplier": 1.5}


# ---------------------------------------------------------------------------
# Signal generation
# ---------------------------------------------------------------------------

class TestGenerateSignal:
    def test_insufficient_bars_returns_hold(self, strategy):
        bars = _make_bars([100.0] * 5)  # need lookback+2 = 7
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "HOLD"
        assert signal.confidence == 0.0
        assert "Insufficient" in signal.reason

    def test_signal_returns_correct_type(self, strategy):
        bars = _make_bars([100.0] * 15)
        signal = strategy.generate_signal(bars, "SPY")
        assert isinstance(signal, Signal)
        assert signal.symbol == "SPY"
        assert signal.strategy_name == "MomentumVolume"
        assert signal.direction in ("BUY", "SELL", "HOLD")
        assert 0.0 <= signal.confidence <= 1.0
        assert len(signal.timestamp) > 0

    def test_buy_signal_on_breakout(self, strategy):
        # lookback=5: bars 1-5 set the high, bar 7 breaks above with high volume
        # Stable prices for bars 0-6, then breakout on bar 7
        prices = [100, 100, 101, 100, 99, 100, 100, 110]
        # Volume spike on last bar (>1.5x average)
        volumes = [1000, 1000, 1000, 1000, 1000, 1000, 1000, 5000]
        bars = _make_bars(prices, volumes)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "BUY"
        assert signal.confidence >= 0.5
        assert "broke above" in signal.reason

    def test_sell_signal_on_breakdown(self, strategy):
        # Price stable then breaks below with volume
        prices = [100, 100, 99, 100, 101, 100, 100, 88]
        volumes = [1000, 1000, 1000, 1000, 1000, 1000, 1000, 5000]
        bars = _make_bars(prices, volumes)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "SELL"
        assert signal.confidence >= 0.5
        assert "broke below" in signal.reason

    def test_hold_without_volume_confirmation(self, strategy):
        # Price breaks above high but volume is normal (not 1.5x)
        prices = [100, 100, 101, 100, 99, 100, 100, 110]
        volumes = [1000] * 8  # no volume spike
        bars = _make_bars(prices, volumes)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "HOLD"

    def test_hold_in_flat_market(self, strategy):
        prices = [100.0] * 15
        bars = _make_bars(prices)
        signal = strategy.generate_signal(bars, "FLAT")
        assert signal.direction == "HOLD"

    def test_confidence_capped_at_one(self, strategy):
        # Extreme breakout should still cap at 1.0
        prices = [100, 100, 100, 100, 100, 100, 100, 500]
        volumes = [1000, 1000, 1000, 1000, 1000, 1000, 1000, 50000]
        bars = _make_bars(prices, volumes)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.confidence <= 1.0

    def test_signal_symbol_matches_input(self, strategy):
        bars = _make_bars([100.0] * 15)
        signal = strategy.generate_signal(bars, "AAPL")
        assert signal.symbol == "AAPL"


# ---------------------------------------------------------------------------
# Backtesting
# ---------------------------------------------------------------------------

class TestBacktest:
    @pytest.fixture
    def backtest_bars(self):
        """Generate synthetic price data with breakouts for backtesting."""
        np.random.seed(42)
        n = 300
        prices = [100.0]
        volumes = []
        for i in range(1, n):
            trend = 0.05 * np.sin(i / 15)
            noise = np.random.normal(0, 1.0)
            prices.append(prices[-1] * (1 + trend / 100 + noise / 100))
            # Random volume spikes to trigger signals
        for i in range(n):
            base_vol = 10000
            if np.random.random() > 0.85:
                volumes.append(int(base_vol * 3))
            else:
                volumes.append(base_vol)
        return _make_bars(prices, volumes)

    def test_backtest_returns_correct_type(self, strategy, backtest_bars):
        result = strategy.backtest(backtest_bars, "TEST")
        assert isinstance(result, BacktestResult)

    def test_backtest_result_fields(self, strategy, backtest_bars):
        result = strategy.backtest(backtest_bars, "TEST")
        assert result.strategy_name == "MomentumVolume"
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
        assert result.max_drawdown_pct >= 0.0

    def test_backtest_to_dict(self, strategy, backtest_bars):
        result = strategy.backtest(backtest_bars, "TEST")
        d = result.to_dict()
        assert isinstance(d, dict)
        assert d["strategy_name"] == "MomentumVolume"
        assert "total_return_pct" in d
        assert "sharpe_ratio" in d
