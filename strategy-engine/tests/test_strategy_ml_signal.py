"""Tests for MLSignalGenerator strategy."""

import numpy as np
import pandas as pd
import pytest
from unittest.mock import patch, MagicMock

from strategies.base import BaseStrategy, Signal, BacktestResult
from strategies.strategy_ml_signal import MLSignalGenerator


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

def _make_bars(n: int = 50) -> pd.DataFrame:
    np.random.seed(42)
    prices = [100.0]
    for _ in range(1, n):
        prices.append(prices[-1] * (1 + np.random.normal(0, 0.01)))
    return pd.DataFrame({
        "timestamp": pd.date_range("2025-01-01 09:30", periods=n, freq="5min"),
        "open": prices,
        "high": [p * 1.005 for p in prices],
        "low": [p * 0.995 for p in prices],
        "close": prices,
        "volume": [1000] * n,
    })


class FakeModel:
    """Fake LightGBM model that returns controlled predictions."""
    def __init__(self, pred_class: int = 1, confidence: float = 0.8):
        self._pred_class = pred_class
        self._confidence = confidence

    def predict(self, X):
        n = len(X)
        proba = np.zeros((n, 3))
        for i in range(n):
            proba[i, self._pred_class] = self._confidence
            remaining = (1 - self._confidence) / 2
            for j in range(3):
                if j != self._pred_class:
                    proba[i, j] = remaining
        return proba


@pytest.fixture
def strategy_no_model():
    with patch.object(MLSignalGenerator, "_load_model"):
        s = MLSignalGenerator()
        s._model = None
        return s


@pytest.fixture
def strategy_buy():
    with patch.object(MLSignalGenerator, "_load_model"):
        s = MLSignalGenerator()
        s._model = FakeModel(pred_class=2, confidence=0.8)  # BUY
        return s


@pytest.fixture
def strategy_sell():
    with patch.object(MLSignalGenerator, "_load_model"):
        s = MLSignalGenerator()
        s._model = FakeModel(pred_class=0, confidence=0.75)  # SELL
        return s


@pytest.fixture
def strategy_low_conf():
    with patch.object(MLSignalGenerator, "_load_model"):
        s = MLSignalGenerator()
        s._model = FakeModel(pred_class=2, confidence=0.5)  # BUY but low confidence
        return s


# ---------------------------------------------------------------------------
# Interface compliance
# ---------------------------------------------------------------------------

class TestBaseStrategyContract:
    def test_extends_base_strategy(self):
        assert issubclass(MLSignalGenerator, BaseStrategy)

    def test_name_property(self, strategy_no_model):
        assert strategy_no_model.name == "MLSignalGenerator"

    def test_params_returns_dict(self, strategy_no_model):
        p = strategy_no_model.params()
        assert isinstance(p, dict)
        assert "min_confidence" in p
        assert "model_loaded" in p

    def test_params_no_model(self, strategy_no_model):
        assert strategy_no_model.params()["model_loaded"] is False

    def test_params_with_model(self, strategy_buy):
        assert strategy_buy.params()["model_loaded"] is True


# ---------------------------------------------------------------------------
# Signal generation
# ---------------------------------------------------------------------------

class TestGenerateSignal:
    def test_hold_when_no_model(self, strategy_no_model):
        bars = _make_bars(50)
        signal = strategy_no_model.generate_signal(bars, "SPY")
        assert signal.direction == "HOLD"
        assert "No trained model" in signal.reason

    def test_hold_insufficient_bars(self, strategy_buy):
        bars = _make_bars(10)
        signal = strategy_buy.generate_signal(bars, "SPY")
        assert signal.direction == "HOLD"
        assert "Insufficient" in signal.reason

    def test_buy_signal(self, strategy_buy):
        bars = _make_bars(50)
        signal = strategy_buy.generate_signal(bars, "AAPL")
        assert isinstance(signal, Signal)
        assert signal.direction == "BUY"
        assert signal.confidence >= 0.65
        assert signal.symbol == "AAPL"
        assert signal.strategy_name == "MLSignalGenerator"

    def test_sell_signal(self, strategy_sell):
        bars = _make_bars(50)
        signal = strategy_sell.generate_signal(bars, "MSFT")
        assert signal.direction == "SELL"
        assert signal.confidence >= 0.65

    def test_low_confidence_returns_hold(self, strategy_low_conf):
        bars = _make_bars(50)
        signal = strategy_low_conf.generate_signal(bars, "SPY")
        assert signal.direction == "HOLD"
        assert "confidence" in signal.reason.lower()

    def test_signal_type_and_fields(self, strategy_buy):
        bars = _make_bars(50)
        signal = strategy_buy.generate_signal(bars, "NVDA")
        assert isinstance(signal, Signal)
        assert signal.direction in ("BUY", "SELL", "HOLD")
        assert 0.0 <= signal.confidence <= 1.0
        assert len(signal.timestamp) > 0

    def test_confidence_capped_at_one(self, strategy_buy):
        bars = _make_bars(50)
        signal = strategy_buy.generate_signal(bars, "TEST")
        assert signal.confidence <= 1.0


# ---------------------------------------------------------------------------
# Backtest
# ---------------------------------------------------------------------------

class TestBacktest:
    def test_backtest_no_model_returns_empty(self, strategy_no_model):
        bars = _make_bars(50)
        result = strategy_no_model.backtest(bars, "TEST")
        assert isinstance(result, BacktestResult)
        assert result.total_trades == 0
        assert result.total_return_pct == 0.0

    def test_backtest_returns_correct_type(self, strategy_buy):
        bars = _make_bars(100)
        result = strategy_buy.backtest(bars, "TEST")
        assert isinstance(result, BacktestResult)
        assert result.strategy_name == "MLSignalGenerator"
        assert result.symbol == "TEST"

    def test_backtest_result_fields(self, strategy_buy):
        bars = _make_bars(100)
        result = strategy_buy.backtest(bars, "TEST")
        assert isinstance(result.total_return_pct, float)
        assert isinstance(result.sharpe_ratio, float)
        assert isinstance(result.win_rate, float)
        assert isinstance(result.total_trades, int)
        assert 0.0 <= result.win_rate <= 1.0
