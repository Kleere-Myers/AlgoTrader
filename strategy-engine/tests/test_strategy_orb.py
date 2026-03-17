"""Tests for OpeningRangeBreakout strategy."""

import numpy as np
import pandas as pd
import pytest

from strategies.base import BaseStrategy, Signal, BacktestResult
from strategies.strategy_orb import OpeningRangeBreakout


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def strategy():
    return OpeningRangeBreakout(opening_bars=6, volume_multiplier=1.2)


@pytest.fixture
def default_strategy():
    return OpeningRangeBreakout()


def _make_bars(prices: list[float], volumes: list[int] | None = None,
               highs: list[float] | None = None, lows: list[float] | None = None) -> pd.DataFrame:
    """Build a minimal OHLCV DataFrame."""
    n = len(prices)
    if volumes is None:
        volumes = [1000] * n
    if highs is None:
        highs = [p * 1.01 for p in prices]
    if lows is None:
        lows = [p * 0.99 for p in prices]
    return pd.DataFrame({
        "timestamp": pd.date_range("2025-01-01", periods=n, freq="D"),
        "open": prices,
        "high": highs,
        "low": lows,
        "close": prices,
        "volume": volumes,
    })


# ---------------------------------------------------------------------------
# Interface compliance
# ---------------------------------------------------------------------------

class TestBaseStrategyContract:
    def test_extends_base_strategy(self):
        assert issubclass(OpeningRangeBreakout, BaseStrategy)

    def test_name_property(self, strategy):
        assert strategy.name == "OpeningRangeBreakout"

    def test_params_returns_dict(self, strategy):
        p = strategy.params()
        assert isinstance(p, dict)
        assert "opening_bars" in p
        assert "volume_multiplier" in p

    def test_default_params(self, default_strategy):
        assert default_strategy.params() == {"opening_bars": 6, "volume_multiplier": 1.2}

    def test_custom_params(self):
        s = OpeningRangeBreakout(opening_bars=10, volume_multiplier=1.5)
        assert s.params() == {"opening_bars": 10, "volume_multiplier": 1.5}


# ---------------------------------------------------------------------------
# Signal generation
# ---------------------------------------------------------------------------

class TestGenerateSignal:
    def test_insufficient_bars_returns_hold(self, strategy):
        bars = _make_bars([100.0] * 5)  # need opening_bars + 2 = 8
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "HOLD"
        assert signal.confidence == 0.0
        assert "Insufficient" in signal.reason

    def test_signal_returns_correct_type(self, strategy):
        bars = _make_bars([100.0] * 12)
        signal = strategy.generate_signal(bars, "SPY")
        assert isinstance(signal, Signal)
        assert signal.symbol == "SPY"
        assert signal.strategy_name == "OpeningRangeBreakout"
        assert signal.direction in ("BUY", "SELL", "HOLD")
        assert 0.0 <= signal.confidence <= 1.0
        assert len(signal.timestamp) > 0

    def test_buy_signal_on_breakout_above_range(self, strategy):
        # 6 bars for opening range, then prev bar within range, then breakout
        # Opening range bars (indices 0-5): highs around 102, lows around 98
        prices = [100, 100, 100, 100, 100, 100, 100, 108]
        highs =  [102, 102, 102, 102, 102, 102, 102, 110]
        lows =   [98,  98,  98,  98,  98,  98,  98,  98]
        # High volume on breakout bar
        volumes = [1000, 1000, 1000, 1000, 1000, 1000, 1000, 5000]
        bars = _make_bars(prices, volumes=volumes, highs=highs, lows=lows)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "BUY"
        assert signal.confidence >= 0.5
        assert "broke above" in signal.reason

    def test_sell_signal_on_breakdown_below_range(self, strategy):
        prices = [100, 100, 100, 100, 100, 100, 100, 90]
        highs =  [102, 102, 102, 102, 102, 102, 102, 102]
        lows =   [98,  98,  98,  98,  98,  98,  98,  88]
        volumes = [1000, 1000, 1000, 1000, 1000, 1000, 1000, 5000]
        bars = _make_bars(prices, volumes=volumes, highs=highs, lows=lows)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "SELL"
        assert signal.confidence >= 0.5
        assert "broke below" in signal.reason

    def test_hold_when_no_breakout(self, strategy):
        # Price stays within range
        prices = [100.0] * 12
        bars = _make_bars(prices)
        signal = strategy.generate_signal(bars, "FLAT")
        assert signal.direction == "HOLD"

    def test_hold_when_volume_insufficient(self, strategy):
        # Breakout without volume confirmation
        prices = [100, 100, 100, 100, 100, 100, 100, 108]
        highs =  [102, 102, 102, 102, 102, 102, 102, 110]
        lows =   [98,  98,  98,  98,  98,  98,  98,  98]
        # Low volume on breakout bar — below 1.2x average
        volumes = [1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000]
        bars = _make_bars(prices, volumes=volumes, highs=highs, lows=lows)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.direction == "HOLD"

    def test_confidence_capped_at_one(self, strategy):
        # Extreme breakout
        prices = [100, 100, 100, 100, 100, 100, 100, 200]
        highs =  [102, 102, 102, 102, 102, 102, 102, 210]
        lows =   [98,  98,  98,  98,  98,  98,  98,  98]
        volumes = [1000, 1000, 1000, 1000, 1000, 1000, 1000, 50000]
        bars = _make_bars(prices, volumes=volumes, highs=highs, lows=lows)
        signal = strategy.generate_signal(bars, "TEST")
        assert signal.confidence <= 1.0

    def test_signal_symbol_matches_input(self, strategy):
        bars = _make_bars([100.0] * 12)
        signal = strategy.generate_signal(bars, "AAPL")
        assert signal.symbol == "AAPL"


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
        assert result.strategy_name == "OpeningRangeBreakout"
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
        assert d["strategy_name"] == "OpeningRangeBreakout"
        assert "total_return_pct" in d
        assert "sharpe_ratio" in d
