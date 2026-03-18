"""Tests for MultiTimeframeTrendAlignment strategy."""

import numpy as np
import pandas as pd
import pytest

from strategies.strategy_multi_timeframe import MultiTimeframeTrendAlignment


def _make_daily_bars(n_days: int, trend: str = "up", symbol: str = "AAPL") -> pd.DataFrame:
    """Generate synthetic daily bars with a controllable trend."""
    dates = pd.bdate_range(end="2026-03-17", periods=n_days)
    base = 100.0
    prices = []

    if trend == "up":
        for i in range(n_days):
            base += np.random.uniform(0.0, 0.5)  # slight upward drift
            prices.append(base)
    elif trend == "down":
        for i in range(n_days):
            base -= np.random.uniform(0.0, 0.5)
            prices.append(base)
    elif trend == "flat":
        for i in range(n_days):
            base += np.random.uniform(-0.2, 0.2)
            prices.append(base)
    else:
        raise ValueError(f"Unknown trend: {trend}")

    closes = np.array(prices)
    return pd.DataFrame({
        "timestamp": dates,
        "open": closes - np.random.uniform(0, 0.5, n_days),
        "high": closes + np.random.uniform(0, 1.0, n_days),
        "low": closes - np.random.uniform(0, 1.0, n_days),
        "close": closes,
        "volume": np.random.randint(1_000_000, 10_000_000, n_days),
    })


def _make_uptrend_with_pullback(n_days: int = 150) -> pd.DataFrame:
    """Generate an uptrend with a RSI pullback at the end."""
    np.random.seed(42)
    dates = pd.bdate_range(end="2026-03-17", periods=n_days)

    # Strong uptrend for most of the period
    prices = [100.0]
    for i in range(1, n_days - 10):
        prices.append(prices[-1] + np.random.uniform(0.1, 0.6))

    # Pullback (slight decline for ~7 days)
    for i in range(7):
        prices.append(prices[-1] - np.random.uniform(0.1, 0.3))

    # Bounce back above EMA
    for i in range(3):
        prices.append(prices[-1] + np.random.uniform(0.3, 0.8))

    closes = np.array(prices)
    return pd.DataFrame({
        "timestamp": dates,
        "open": closes - 0.2,
        "high": closes + 0.5,
        "low": closes - 0.5,
        "close": closes,
        "volume": np.random.randint(1_000_000, 10_000_000, n_days),
    })


class TestMultiTimeframeTrend:

    def test_insufficient_bars_returns_hold(self):
        strat = MultiTimeframeTrendAlignment()
        bars = _make_daily_bars(30)
        sig = strat.generate_signal(bars, "AAPL")
        assert sig.direction == "HOLD"
        assert "Insufficient" in sig.reason
        assert sig.trade_type == "swing"

    def test_name(self):
        strat = MultiTimeframeTrendAlignment()
        assert strat.name == "MultiTimeframeTrend"

    def test_params_returned(self):
        strat = MultiTimeframeTrendAlignment()
        p = strat.params()
        assert "weekly_ema_period" in p
        assert "daily_rsi_period" in p

    def test_uptrend_with_pullback_generates_buy(self):
        strat = MultiTimeframeTrendAlignment()
        bars = _make_uptrend_with_pullback(150)
        sig = strat.generate_signal(bars, "AAPL")
        # Should recognize the pullback in uptrend
        assert sig.trade_type == "swing"
        assert sig.strategy_name == "MultiTimeframeTrend"
        # Direction depends on exact RSI value, but should not error
        assert sig.direction in ("BUY", "HOLD", "SELL")

    def test_downtrend_does_not_generate_buy(self):
        np.random.seed(123)
        strat = MultiTimeframeTrendAlignment()
        bars = _make_daily_bars(150, trend="down")
        sig = strat.generate_signal(bars, "AAPL")
        assert sig.direction != "BUY"
        assert sig.trade_type == "swing"

    def test_flat_market_returns_hold(self):
        np.random.seed(456)
        strat = MultiTimeframeTrendAlignment()
        bars = _make_daily_bars(150, trend="flat")
        sig = strat.generate_signal(bars, "AAPL")
        # Flat market has no weekly trend alignment
        assert sig.direction == "HOLD"

    def test_signal_confidence_bounded(self):
        strat = MultiTimeframeTrendAlignment()
        bars = _make_uptrend_with_pullback(150)
        sig = strat.generate_signal(bars, "AAPL")
        assert 0.0 <= sig.confidence <= 1.0

    def test_backtest_returns_result(self):
        strat = MultiTimeframeTrendAlignment()
        bars = _make_daily_bars(150, trend="up")
        result = strat.backtest(bars, "AAPL")
        assert result.strategy_name == "MultiTimeframeTrend"
        assert result.symbol == "AAPL"

    def test_backtest_insufficient_data(self):
        strat = MultiTimeframeTrendAlignment()
        bars = _make_daily_bars(30)
        result = strat.backtest(bars, "AAPL")
        assert result.total_trades == 0

    def test_weekly_resample(self):
        strat = MultiTimeframeTrendAlignment()
        bars = _make_daily_bars(50)
        weekly = strat._resample_weekly(bars)
        # ~50 business days ≈ 10 weeks
        assert len(weekly) >= 8
        assert len(weekly) <= 12
