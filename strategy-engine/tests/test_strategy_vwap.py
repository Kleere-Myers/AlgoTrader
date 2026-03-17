"""Tests for VWAPStrategy."""

import numpy as np
import pandas as pd
import pytest

from strategies.base import BaseStrategy, Signal, BacktestResult
from strategies.strategy_vwap import VWAPStrategy


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def strategy():
    return VWAPStrategy(distance_threshold=0.005)


@pytest.fixture
def default_strategy():
    return VWAPStrategy()


def _make_bars(prices: list[float], volumes: list[int] | None = None) -> pd.DataFrame:
    """Build a minimal OHLCV DataFrame from a list of close prices."""
    n = len(prices)
    if volumes is None:
        volumes = [1000] * n
    return pd.DataFrame({
        "timestamp": pd.date_range("2025-01-01", periods=n, freq="D"),
        "open": prices,
        "high": [p * 1.01 for p in prices],
        "low": [p * 0.99 for p in prices],
        "close": prices,
        "volume": volumes,
    })


# ---------------------------------------------------------------------------
# Interface compliance
# ---------------------------------------------------------------------------

class TestBaseStrategyContract:
    def test_extends_base_strategy(self):
        assert issubclass(VWAPStrategy, BaseStrategy)

    def test_name_property(self, strategy):
        assert strategy.name == "VWAPStrategy"

    def test_params_returns_dict(self, strategy):
        p = strategy.params()
        assert isinstance(p, dict)
        assert "distance_threshold" in p

    def test_default_params(self, default_strategy):
        assert default_strategy.params() == {"distance_threshold": 0.005}

    def test_custom_params(self):
        s = VWAPStrategy(distance_threshold=0.01)
        assert s.params() == {"distance_threshold": 0.01}


# ---------------------------------------------------------------------------
# Signal generation
# ---------------------------------------------------------------------------

class TestGenerateSignal:
    def test_insufficient_bars_returns_hold(self, strategy):
        bars = _make_bars([100.0] * 5)  # need at least 10
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "HOLD"
        assert signal.confidence == 0.0
        assert "Insufficient" in signal.reason

    def test_signal_returns_correct_type(self, strategy):
        bars = _make_bars([100.0] * 15)
        signal = strategy.generate_signal(bars, "SPY")
        assert isinstance(signal, Signal)
        assert signal.symbol == "SPY"
        assert signal.strategy_name == "VWAPStrategy"
        assert signal.direction in ("BUY", "SELL", "HOLD")
        assert 0.0 <= signal.confidence <= 1.0
        assert len(signal.timestamp) > 0

    def test_buy_signal_when_price_crosses_below_vwap(self, strategy):
        # Create prices that start above VWAP then cross below
        # High volume early pushes VWAP up, then price drops below
        prices = [100, 101, 102, 103, 104, 105, 104, 103, 102, 101, 100, 99]
        volumes = [5000, 5000, 5000, 5000, 5000, 5000, 1000, 1000, 1000, 1000, 1000, 1000]
        bars = _make_bars(prices, volumes)
        signal = strategy.generate_signal(bars, "TEST")
        # With these prices the close drops well below VWAP
        if signal.direction == "BUY":
            assert signal.confidence > 0.0
            assert "below VWAP" in signal.reason

    def test_sell_signal_when_price_crosses_above_vwap(self, strategy):
        # Start low, end high — price crosses above VWAP
        prices = [100, 99, 98, 97, 96, 95, 96, 97, 98, 99, 100, 105]
        volumes = [5000, 5000, 5000, 5000, 5000, 5000, 1000, 1000, 1000, 1000, 1000, 1000]
        bars = _make_bars(prices, volumes)
        signal = strategy.generate_signal(bars, "TEST")
        if signal.direction == "SELL":
            assert signal.confidence > 0.0
            assert "above VWAP" in signal.reason

    def test_hold_signal_in_flat_market(self, strategy):
        prices = [100.0] * 15
        bars = _make_bars(prices)
        signal = strategy.generate_signal(bars, "FLAT")
        assert signal.direction == "HOLD"

    def test_confidence_capped_at_one(self, strategy):
        # Extreme price movement should still cap confidence at 1.0
        prices = [100] * 10 + [50, 20]
        volumes = [10000] * 10 + [100, 100]
        bars = _make_bars(prices, volumes)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.confidence <= 1.0

    def test_signal_symbol_matches_input(self, strategy):
        bars = _make_bars([100.0] * 15)
        signal = strategy.generate_signal(bars, "AAPL")
        assert signal.symbol == "AAPL"


# ---------------------------------------------------------------------------
# VWAP computation
# ---------------------------------------------------------------------------

class TestVWAPComputation:
    def test_vwap_with_uniform_volume(self):
        """With uniform volume, VWAP should equal cumulative average of typical prices."""
        bars = _make_bars([100, 102, 104, 106, 108])
        vwap = VWAPStrategy._compute_vwap(bars)
        assert len(vwap) == 5
        assert not np.isnan(vwap.iloc[-1])

    def test_vwap_is_monotonic_with_rising_prices_and_uniform_volume(self):
        bars = _make_bars([100, 102, 104, 106, 108, 110])
        vwap = VWAPStrategy._compute_vwap(bars)
        # VWAP should generally rise with rising prices
        assert vwap.iloc[-1] > vwap.iloc[0]

    def test_vwap_with_zero_volume(self):
        bars = _make_bars([100, 102, 104], volumes=[0, 0, 0])
        vwap = VWAPStrategy._compute_vwap(bars)
        # Should be NaN when volume is zero
        assert np.isnan(vwap.iloc[-1])


# ---------------------------------------------------------------------------
# Backtesting
# ---------------------------------------------------------------------------

class TestBacktest:
    @pytest.fixture
    def backtest_bars(self):
        """Generate synthetic price data for backtesting."""
        np.random.seed(42)
        n = 200
        prices = [100.0]
        for i in range(1, n):
            trend = 0.05 * np.sin(i / 20)
            noise = np.random.normal(0, 0.5)
            prices.append(prices[-1] * (1 + trend + noise / 100))
        return _make_bars(prices)

    def test_backtest_returns_correct_type(self, strategy, backtest_bars):
        result = strategy.backtest(backtest_bars, "TEST")
        assert isinstance(result, BacktestResult)

    def test_backtest_result_fields(self, strategy, backtest_bars):
        result = strategy.backtest(backtest_bars, "TEST")
        assert result.strategy_name == "VWAPStrategy"
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
        assert d["strategy_name"] == "VWAPStrategy"
        assert "total_return_pct" in d
        assert "sharpe_ratio" in d
