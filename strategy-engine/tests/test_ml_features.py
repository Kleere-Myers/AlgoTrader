"""Tests for ML feature pipeline."""

import numpy as np
import pandas as pd
import pytest

from ml.features import (
    FEATURE_COLUMNS,
    compute_features,
    compute_labels,
    compute_rsi,
    compute_macd,
    compute_bollinger_pct_b,
    compute_atr,
)


def _make_bars(n: int = 100) -> pd.DataFrame:
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
        "volume": [np.random.randint(500, 5000) for _ in range(n)],
    })


class TestComputeFeatures:
    def test_all_feature_columns_present(self):
        bars = _make_bars(100)
        result = compute_features(bars)
        for col in FEATURE_COLUMNS:
            assert col in result.columns, f"Missing feature: {col}"

    def test_output_same_length(self):
        bars = _make_bars(100)
        result = compute_features(bars)
        assert len(result) == len(bars)

    def test_original_columns_preserved(self):
        bars = _make_bars(100)
        result = compute_features(bars)
        for col in ["timestamp", "open", "high", "low", "close", "volume"]:
            assert col in result.columns

    def test_day_of_week_range(self):
        bars = _make_bars(100)
        result = compute_features(bars)
        valid = result["day_of_week"].dropna()
        assert valid.min() >= 0
        assert valid.max() <= 6

    def test_hour_of_day_range(self):
        bars = _make_bars(100)
        result = compute_features(bars)
        valid = result["hour_of_day"].dropna()
        assert valid.min() >= 0
        assert valid.max() <= 23

    def test_rsi_bounded(self):
        bars = _make_bars(100)
        result = compute_features(bars)
        valid_rsi = result["rsi_14"].dropna()
        assert valid_rsi.min() >= 0.0
        assert valid_rsi.max() <= 100.0

    def test_volume_ratio_positive(self):
        bars = _make_bars(100)
        result = compute_features(bars)
        valid = result["volume_ratio"].dropna()
        assert (valid > 0).all()


class TestComputeRSI:
    def test_rsi_values_bounded(self):
        # Use oscillating prices so RSI has both gains and losses
        close = pd.Series([100 + 5 * np.sin(i / 3) for i in range(50)])
        rsi = compute_rsi(close, 14)
        valid = rsi.dropna()
        assert len(valid) > 0
        assert valid.min() >= 0.0
        assert valid.max() <= 100.0

    def test_rsi_nan_for_insufficient_data(self):
        close = pd.Series([100, 101, 102])
        rsi = compute_rsi(close, 14)
        assert rsi.isna().all()


class TestComputeMACD:
    def test_macd_returns_three_series(self):
        close = pd.Series([100 + np.sin(i / 5) for i in range(100)])
        macd, signal, hist = compute_macd(close)
        assert len(macd) == 100
        assert len(signal) == 100
        assert len(hist) == 100

    def test_histogram_is_macd_minus_signal(self):
        close = pd.Series([100 + np.sin(i / 5) for i in range(100)])
        macd, signal, hist = compute_macd(close)
        np.testing.assert_allclose(hist, macd - signal, atol=1e-10)


class TestComputeLabels:
    def test_label_values(self):
        close = pd.Series([100.0] * 20)
        labels = compute_labels(close, threshold=0.003, forward_bars=6)
        # Flat prices -> all 0 (except trailing NaN)
        assert (labels.iloc[:-6] == 0).all()

    def test_positive_label(self):
        # Price jumps 1% after 6 bars
        prices = [100.0] * 6 + [101.0] * 6
        close = pd.Series(prices)
        labels = compute_labels(close, threshold=0.003, forward_bars=6)
        assert labels.iloc[0] == 1  # 100 -> 101 = +1%

    def test_negative_label(self):
        prices = [100.0] * 6 + [99.0] * 6
        close = pd.Series(prices)
        labels = compute_labels(close, threshold=0.003, forward_bars=6)
        assert labels.iloc[0] == -1

    def test_trailing_nan(self):
        close = pd.Series([100.0] * 20)
        labels = compute_labels(close, threshold=0.003, forward_bars=6)
        assert labels.iloc[-6:].isna().all()
