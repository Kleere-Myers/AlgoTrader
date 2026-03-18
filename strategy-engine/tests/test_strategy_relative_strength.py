"""Tests for RelativeStrengthRanking strategy."""

from unittest.mock import patch, MagicMock

import numpy as np
import pandas as pd
import pytest

from strategies.strategy_relative_strength import RelativeStrengthRanking


def _make_bars(n: int, trend_pct: float = 0.1, symbol: str = "AAPL") -> pd.DataFrame:
    """Generate bars with a given total return over the period."""
    dates = pd.bdate_range(end="2026-03-17", periods=n)
    start = 100.0
    end = start * (1 + trend_pct)
    closes = np.linspace(start, end, n)
    return pd.DataFrame({
        "timestamp": dates,
        "open": closes - 0.2,
        "high": closes + 0.5,
        "low": closes - 0.5,
        "close": closes,
        "volume": np.random.randint(1_000_000, 5_000_000, n),
    })


def _mock_benchmark(n: int = 30, trend_pct: float = 0.05):
    """Create benchmark bars DataFrame."""
    dates = pd.bdate_range(end="2026-03-17", periods=n)
    closes = np.linspace(400, 400 * (1 + trend_pct), n)
    return pd.DataFrame({"timestamp": dates, "close": closes})


class TestRelativeStrengthRanking:

    def test_insufficient_bars_returns_hold(self):
        strat = RelativeStrengthRanking(lookback=20)
        bars = _make_bars(10)
        sig = strat.generate_signal(bars, "AAPL")
        assert sig.direction == "HOLD"
        assert "Insufficient" in sig.reason
        assert sig.trade_type == "swing"

    def test_name(self):
        assert RelativeStrengthRanking().name == "RelativeStrength"

    def test_params(self):
        strat = RelativeStrengthRanking(lookback=30, benchmark="QQQ")
        p = strat.params()
        assert p["lookback"] == 30
        assert p["benchmark"] == "QQQ"

    @patch.object(RelativeStrengthRanking, "_fetch_benchmark_bars")
    @patch.object(RelativeStrengthRanking, "_fetch_all_symbol_returns")
    def test_strong_symbol_gets_buy(self, mock_all_returns, mock_bench):
        strat = RelativeStrengthRanking(lookback=20)

        # Symbol outperforms benchmark
        bars = _make_bars(30, trend_pct=0.20)  # +20%
        mock_bench.return_value = _mock_benchmark(30, trend_pct=0.05)  # SPY +5%

        # Ranked top quartile
        mock_all_returns.return_value = {
            "AAPL": 0.20,
            "MSFT": 0.10,
            "GOOGL": 0.05,
            "NVDA": 0.02,
            "SPY": 0.05,
        }

        sig = strat.generate_signal(bars, "AAPL")
        assert sig.direction == "BUY"
        assert sig.confidence > 0.0
        assert "top" in sig.reason.lower() or "percentile" in sig.reason.lower()

    @patch.object(RelativeStrengthRanking, "_fetch_benchmark_bars")
    @patch.object(RelativeStrengthRanking, "_fetch_all_symbol_returns")
    def test_weak_symbol_gets_sell(self, mock_all_returns, mock_bench):
        strat = RelativeStrengthRanking(lookback=20)

        bars = _make_bars(30, trend_pct=-0.10)  # -10%
        mock_bench.return_value = _mock_benchmark(30, trend_pct=0.05)

        mock_all_returns.return_value = {
            "AAPL": -0.10,  # worst
            "MSFT": 0.10,
            "GOOGL": 0.15,
            "NVDA": 0.20,
            "SPY": 0.05,
        }

        sig = strat.generate_signal(bars, "AAPL")
        assert sig.direction == "SELL"

    @patch.object(RelativeStrengthRanking, "_fetch_benchmark_bars")
    @patch.object(RelativeStrengthRanking, "_fetch_all_symbol_returns")
    def test_middle_rank_gets_hold(self, mock_all_returns, mock_bench):
        strat = RelativeStrengthRanking(lookback=20)

        bars = _make_bars(30, trend_pct=0.05)
        mock_bench.return_value = _mock_benchmark(30, trend_pct=0.05)

        mock_all_returns.return_value = {
            "AAPL": 0.05,  # middle
            "MSFT": 0.10,
            "GOOGL": 0.15,
            "NVDA": -0.05,
            "QQQ": 0.02,
            "SPY": 0.05,
        }

        sig = strat.generate_signal(bars, "AAPL")
        assert sig.direction == "HOLD"

    @patch.object(RelativeStrengthRanking, "_fetch_benchmark_bars")
    def test_missing_benchmark_returns_hold(self, mock_bench):
        strat = RelativeStrengthRanking()
        mock_bench.return_value = None
        bars = _make_bars(30)
        sig = strat.generate_signal(bars, "AAPL")
        assert sig.direction == "HOLD"
        assert "benchmark" in sig.reason.lower()

    @patch.object(RelativeStrengthRanking, "_fetch_benchmark_bars")
    @patch.object(RelativeStrengthRanking, "_fetch_all_symbol_returns")
    def test_few_peers_uses_simple_ratio(self, mock_all_returns, mock_bench):
        strat = RelativeStrengthRanking(lookback=20)
        bars = _make_bars(30, trend_pct=0.30)  # +30%
        mock_bench.return_value = _mock_benchmark(30, trend_pct=0.05)
        mock_all_returns.return_value = {"AAPL": 0.30, "SPY": 0.05}  # only 2 symbols

        sig = strat.generate_signal(bars, "AAPL")
        # With < 4 peers, falls back to simple RS ratio
        assert sig.direction == "BUY"  # RS ratio = 6.0 > 1.2

    def test_signal_always_swing_type(self):
        strat = RelativeStrengthRanking()
        bars = _make_bars(10)  # insufficient
        sig = strat.generate_signal(bars, "AAPL")
        assert sig.trade_type == "swing"

    def test_confidence_bounded(self):
        strat = RelativeStrengthRanking()
        bars = _make_bars(10)
        sig = strat.generate_signal(bars, "AAPL")
        assert 0.0 <= sig.confidence <= 1.0

    @patch.object(RelativeStrengthRanking, "_fetch_benchmark_bars")
    def test_backtest_insufficient_data(self, mock_bench):
        strat = RelativeStrengthRanking(lookback=20)
        mock_bench.return_value = None
        bars = _make_bars(10)
        result = strat.backtest(bars, "AAPL")
        assert result.total_trades == 0
