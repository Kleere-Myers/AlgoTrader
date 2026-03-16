"""ML feature pipeline — compute all features defined in AGENT_STRATEGY.md."""

import numpy as np
import pandas as pd


FEATURE_COLUMNS = [
    "rsi_14",
    "macd",
    "macd_signal",
    "macd_hist",
    "bb_pct_b",
    "atr_14",
    "volume_ratio",
    "roc_5",
    "roc_10",
    "roc_20",
    "rolling_return_5",
    "rolling_return_20",
    "rolling_vol_20",
    "day_of_week",
    "hour_of_day",
]

# Label: +1 if price up 0.3% in next 30 min, -1 if down 0.3%, 0 otherwise
LABEL_THRESHOLD = 0.003
FORWARD_BARS = 6  # 6 × 5min = 30 min


def compute_rsi(close: pd.Series, period: int = 14) -> pd.Series:
    delta = close.diff()
    gain = delta.where(delta > 0, 0.0)
    loss = (-delta).where(delta < 0, 0.0)
    avg_gain = gain.rolling(period).mean()
    avg_loss = loss.rolling(period).mean()
    rs = avg_gain / avg_loss.replace(0, np.nan)
    return 100.0 - (100.0 / (1.0 + rs))


def compute_macd(close: pd.Series, fast: int = 12, slow: int = 26, signal: int = 9):
    ema_fast = close.ewm(span=fast, adjust=False).mean()
    ema_slow = close.ewm(span=slow, adjust=False).mean()
    macd_line = ema_fast - ema_slow
    signal_line = macd_line.ewm(span=signal, adjust=False).mean()
    histogram = macd_line - signal_line
    return macd_line, signal_line, histogram


def compute_bollinger_pct_b(close: pd.Series, period: int = 20, std_dev: float = 2.0) -> pd.Series:
    sma = close.rolling(period).mean()
    std = close.rolling(period).std()
    upper = sma + std_dev * std
    lower = sma - std_dev * std
    bandwidth = upper - lower
    pct_b = (close - lower) / bandwidth.replace(0, np.nan)
    return pct_b


def compute_atr(high: pd.Series, low: pd.Series, close: pd.Series, period: int = 14) -> pd.Series:
    tr1 = high - low
    tr2 = (high - close.shift(1)).abs()
    tr3 = (low - close.shift(1)).abs()
    true_range = pd.concat([tr1, tr2, tr3], axis=1).max(axis=1)
    return true_range.rolling(period).mean()


def compute_features(df: pd.DataFrame) -> pd.DataFrame:
    """Compute all ML features from an OHLCV DataFrame.

    Expects columns: timestamp, open, high, low, close, volume.
    Returns a copy with feature columns added.
    """
    out = df.copy()
    close = out["close"]
    high = out["high"]
    low = out["low"]
    volume = out["volume"].astype(float)

    # RSI
    out["rsi_14"] = compute_rsi(close, 14)

    # MACD
    macd_line, signal_line, histogram = compute_macd(close)
    out["macd"] = macd_line
    out["macd_signal"] = signal_line
    out["macd_hist"] = histogram

    # Bollinger %B
    out["bb_pct_b"] = compute_bollinger_pct_b(close)

    # ATR
    out["atr_14"] = compute_atr(high, low, close, 14)

    # Volume ratio: current volume / 20-bar average volume
    out["volume_ratio"] = volume / volume.rolling(20).mean()

    # Rate of change
    out["roc_5"] = close.pct_change(5)
    out["roc_10"] = close.pct_change(10)
    out["roc_20"] = close.pct_change(20)

    # Rolling return
    out["rolling_return_5"] = close.pct_change(5)
    out["rolling_return_20"] = close.pct_change(20)

    # Rolling volatility (20-bar standard deviation of returns)
    out["rolling_vol_20"] = close.pct_change().rolling(20).std()

    # Time features
    ts = pd.to_datetime(out["timestamp"])
    out["day_of_week"] = ts.dt.dayofweek
    out["hour_of_day"] = ts.dt.hour

    return out


def compute_labels(close: pd.Series, threshold: float = LABEL_THRESHOLD,
                   forward_bars: int = FORWARD_BARS) -> pd.Series:
    """Compute target labels: +1 if price up threshold in forward_bars, -1 if down, 0 otherwise."""
    future_return = close.shift(-forward_bars) / close - 1.0
    labels = pd.Series(0, index=close.index, dtype=int)
    labels[future_return >= threshold] = 1
    labels[future_return <= -threshold] = -1
    # NaN out bars where we can't compute forward return
    labels.iloc[-forward_bars:] = np.nan
    return labels
