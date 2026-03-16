"""MLSignalGenerator strategy — LightGBM-based signal generation."""

import logging
import pickle
from pathlib import Path

import numpy as np
import pandas as pd

from strategies.base import BacktestResult, BaseStrategy, Signal
from ml.features import FEATURE_COLUMNS, compute_features

logger = logging.getLogger(__name__)

MODELS_DIR = Path(__file__).resolve().parent.parent / "models"
MODEL_PATH = MODELS_DIR / "lgbm_signal_model.pkl"

MIN_CONFIDENCE = 0.65


class MLSignalGenerator(BaseStrategy):
    def __init__(self, min_confidence: float = MIN_CONFIDENCE):
        self.min_confidence = min_confidence
        self._model = None
        self._load_model()

    def _load_model(self):
        """Load trained LightGBM model from disk. Gracefully handle missing model."""
        if MODEL_PATH.exists():
            with open(MODEL_PATH, "rb") as f:
                self._model = pickle.load(f)
            logger.info("Loaded LightGBM model from %s", MODEL_PATH)
        else:
            logger.warning("No trained model found at %s — MLSignalGenerator will emit HOLD", MODEL_PATH)

    def reload_model(self):
        """Reload model from disk (called after retraining)."""
        self._load_model()

    @property
    def name(self) -> str:
        return "MLSignalGenerator"

    def params(self) -> dict:
        return {
            "min_confidence": self.min_confidence,
            "model_loaded": self._model is not None,
        }

    def generate_signal(self, bars: pd.DataFrame, symbol: str) -> Signal:
        if self._model is None:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason="No trained model available",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        # Need enough bars for feature computation (at least 30 for rolling windows)
        if len(bars) < 30:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason=f"Insufficient bars ({len(bars)}) for feature computation",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        featured = compute_features(bars)
        last_row = featured[FEATURE_COLUMNS].iloc[-1:]

        if last_row.isna().any(axis=1).iloc[0]:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason="Feature values contain NaN",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        # Predict: model outputs probabilities for classes [SELL(-1), HOLD(0), BUY(+1)]
        proba = self._model.predict(last_row.values)[0]
        pred_class = int(np.argmax(proba))
        confidence = float(proba[pred_class])

        # Map class index to direction: 0 -> SELL, 1 -> HOLD, 2 -> BUY
        class_map = {0: "SELL", 1: "HOLD", 2: "BUY"}
        direction = class_map[pred_class]

        # If confidence below threshold, emit HOLD
        if direction != "HOLD" and confidence < self.min_confidence:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=round(confidence, 4),
                reason=f"ML predicted {direction} with confidence {confidence:.4f} < min {self.min_confidence}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        return Signal(
            symbol=symbol,
            direction=direction,
            confidence=round(confidence, 4),
            reason=f"ML model predicted {direction} (proba: SELL={proba[0]:.3f}, HOLD={proba[1]:.3f}, BUY={proba[2]:.3f})",
            strategy_name=self.name,
            timestamp=self._now_iso(),
        )

    def backtest(self, bars: pd.DataFrame, symbol: str) -> BacktestResult:
        if self._model is None:
            return BacktestResult(
                strategy_name=self.name,
                symbol=symbol,
                total_return_pct=0.0,
                sharpe_ratio=0.0,
                max_drawdown_pct=0.0,
                win_rate=0.0,
                total_trades=0,
                avg_trade_duration_mins=0.0,
                profit_factor=0.0,
                period_start=str(bars["timestamp"].iloc[0]),
                period_end=str(bars["timestamp"].iloc[-1]),
            )

        import vectorbt as vbt

        featured = compute_features(bars)
        close = bars["close"]

        # Generate predictions for each row
        feature_data = featured[FEATURE_COLUMNS]
        valid_mask = ~feature_data.isna().any(axis=1)
        predictions = pd.Series(1, index=bars.index)  # default HOLD class
        confidences = pd.Series(0.0, index=bars.index)

        if valid_mask.any():
            valid_features = feature_data[valid_mask].values
            proba = self._model.predict(valid_features)
            pred_classes = np.argmax(proba, axis=1)
            pred_confs = np.max(proba, axis=1)
            predictions[valid_mask] = pred_classes
            confidences[valid_mask] = pred_confs

        # Entries: BUY (class 2) with sufficient confidence
        entries = (predictions == 2) & (confidences >= self.min_confidence)
        # Exits: SELL (class 0) with sufficient confidence
        exits = (predictions == 0) & (confidences >= self.min_confidence)

        pf = vbt.Portfolio.from_signals(
            close,
            entries=entries,
            exits=exits,
            init_cash=10_000,
            fees=0.0,
            slippage=0.0005,
            freq="1D",
        )

        stats = pf.stats()
        trades = pf.trades.records_readable if pf.trades.count() > 0 else pd.DataFrame()
        total_trades = int(pf.trades.count())

        def _safe(val, default=0.0):
            v = float(val) if not isinstance(val, float) else val
            return default if np.isnan(v) or np.isinf(v) else v

        if total_trades > 0 and not trades.empty:
            win_rate = _safe(float(stats.get("Win Rate [%]", 0.0)) / 100.0)
            dur = stats.get("Avg Winning Trade Duration", pd.Timedelta(0))
            avg_duration_days = _safe(dur.total_seconds() / 86400) if isinstance(dur, pd.Timedelta) else 0.0
            avg_duration_mins = avg_duration_days * 24 * 60
            winning_pnl = trades.loc[trades["PnL"] > 0, "PnL"].sum() if "PnL" in trades.columns else 0.0
            losing_pnl = abs(trades.loc[trades["PnL"] < 0, "PnL"].sum()) if "PnL" in trades.columns else 0.0
            profit_factor = float(winning_pnl / losing_pnl) if losing_pnl > 0 else float("inf")
        else:
            win_rate = 0.0
            avg_duration_mins = 0.0
            profit_factor = 0.0

        return BacktestResult(
            strategy_name=self.name,
            symbol=symbol,
            total_return_pct=round(_safe(stats.get("Total Return [%]", 0.0)), 4),
            sharpe_ratio=round(_safe(stats.get("Sharpe Ratio", 0.0)), 4),
            max_drawdown_pct=round(_safe(stats.get("Max Drawdown [%]", 0.0)), 4),
            win_rate=round(win_rate, 4),
            total_trades=total_trades,
            avg_trade_duration_mins=round(avg_duration_mins, 2),
            profit_factor=round(profit_factor, 4) if profit_factor != float("inf") else 999.0,
            period_start=str(bars["timestamp"].iloc[0]),
            period_end=str(bars["timestamp"].iloc[-1]),
        )
