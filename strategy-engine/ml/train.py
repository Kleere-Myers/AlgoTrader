"""Train LightGBM model on historical data from DuckDB."""

import os
import pickle
from pathlib import Path

import duckdb
import lightgbm as lgb
import numpy as np
import pandas as pd
from sklearn.model_selection import TimeSeriesSplit
from sklearn.metrics import classification_report

from ml.features import FEATURE_COLUMNS, compute_features, compute_labels

DEFAULT_SYMBOLS = ["SPY", "QQQ", "AAPL", "MSFT", "NVDA", "GOOGL"]
SYMBOLS = [s.strip().upper() for s in os.environ.get("SYMBOLS", ",".join(DEFAULT_SYMBOLS)).split(",") if s.strip()]
MODELS_DIR = Path(__file__).resolve().parent.parent / "models"
MODEL_PATH = MODELS_DIR / "lgbm_signal_model.pkl"
METADATA_PATH = MODELS_DIR / "lgbm_metadata.pkl"

DB_PATH = os.environ.get(
    "DUCKDB_PATH",
    str(Path(__file__).resolve().parent.parent.parent / "data" / "algotrader.duckdb"),
)


def load_training_data() -> pd.DataFrame:
    """Load OHLCV bars for all symbols from DuckDB and compute features + labels."""
    con = duckdb.connect(DB_PATH, read_only=True)
    try:
        placeholders = ", ".join(["?"] * len(SYMBOLS))
        all_bars = con.execute(
            "SELECT symbol, timestamp, open, high, low, close, volume, bar_size "
            f"FROM ohlcv_bars WHERE symbol IN ({placeholders}) "
            "ORDER BY symbol, timestamp",
            SYMBOLS,
        ).fetchdf()
    finally:
        con.close()

    if all_bars.empty:
        raise ValueError("No OHLCV data found in DuckDB for training")

    frames = []
    for symbol in SYMBOLS:
        sym_bars = all_bars[all_bars["symbol"] == symbol].copy()
        if sym_bars.empty:
            continue
        sym_bars = sym_bars.sort_values("timestamp").reset_index(drop=True)
        featured = compute_features(sym_bars)
        featured["label"] = compute_labels(featured["close"])
        featured["symbol"] = symbol
        frames.append(featured)

    combined = pd.concat(frames, ignore_index=True)
    return combined


def train_model(data: pd.DataFrame | None = None) -> dict:
    """Train LightGBM classifier and save to models/ directory.

    Returns dict with training metrics.
    """
    if data is None:
        data = load_training_data()

    # Drop rows with NaN in features or label
    subset = data[FEATURE_COLUMNS + ["label"]].dropna()
    if len(subset) < 100:
        raise ValueError(f"Too few training samples ({len(subset)}) after dropping NaN rows")

    X = subset[FEATURE_COLUMNS].values
    # Remap labels: -1 -> 0 (SELL), 0 -> 1 (HOLD), 1 -> 2 (BUY) for LightGBM
    y_raw = subset["label"].values.astype(int)
    y = y_raw + 1  # now 0, 1, 2

    # Time-series cross-validation
    tscv = TimeSeriesSplit(n_splits=3)
    best_model = None
    best_score = -1.0

    for train_idx, val_idx in tscv.split(X):
        X_train, X_val = X[train_idx], X[val_idx]
        y_train, y_val = y[train_idx], y[val_idx]

        train_set = lgb.Dataset(X_train, label=y_train)
        val_set = lgb.Dataset(X_val, label=y_val, reference=train_set)

        params = {
            "objective": "multiclass",
            "num_class": 3,
            "metric": "multi_logloss",
            "learning_rate": 0.05,
            "num_leaves": 31,
            "max_depth": 6,
            "min_child_samples": 20,
            "subsample": 0.8,
            "colsample_bytree": 0.8,
            "verbose": -1,
        }

        model = lgb.train(
            params,
            train_set,
            num_boost_round=300,
            valid_sets=[val_set],
            callbacks=[lgb.early_stopping(30), lgb.log_evaluation(0)],
        )

        preds = model.predict(X_val)
        pred_labels = np.argmax(preds, axis=1)  # 0, 1, 2
        accuracy = np.mean(pred_labels == y_val)

        if accuracy > best_score:
            best_score = accuracy
            best_model = model

    # Save model
    MODELS_DIR.mkdir(parents=True, exist_ok=True)
    with open(MODEL_PATH, "wb") as f:
        pickle.dump(best_model, f)

    # Save metadata
    metadata = {
        "feature_columns": FEATURE_COLUMNS,
        "symbols": SYMBOLS,
        "samples": len(subset),
        "accuracy": round(best_score, 4),
        "label_mapping": {0: -1, 1: 0, 2: 1},  # LightGBM class index -> label
    }
    with open(METADATA_PATH, "wb") as f:
        pickle.dump(metadata, f)

    # Final evaluation on full dataset
    full_preds = best_model.predict(X)
    full_pred_labels = np.argmax(full_preds, axis=1)
    report = classification_report(y, full_pred_labels, target_names=["SELL(0)", "HOLD(1)", "BUY(2)"], output_dict=True)

    return {
        "model_path": str(MODEL_PATH),
        "samples": len(subset),
        "best_cv_accuracy": round(best_score, 4),
        "full_report": report,
    }


if __name__ == "__main__":
    result = train_model()
    print(f"Model saved to {result['model_path']}")
    print(f"Training samples: {result['samples']}")
    print(f"Best CV accuracy: {result['best_cv_accuracy']}")
