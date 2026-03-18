"""Composite scoring framework — aggregates weighted signals from multiple strategies."""

from datetime import datetime, timezone

from strategies.base import Signal


# Default swing strategy weights — sum to 1.0
DEFAULT_WEIGHTS = {
    "MultiTimeframeTrend": 0.35,
    "RelativeStrength": 0.25,
    "RSIMeanReversion": 0.15,
    "MomentumVolume": 0.15,
    "NewsSentimentStrategy": 0.10,
}

# Minimum composite score to trigger BUY or SELL
DEFAULT_THRESHOLD = 0.30


class CompositeScorer:
    """Aggregate weighted strategy signals into a single swing trading signal.

    Each strategy contributes a directional score:
        BUY  → +confidence
        SELL → -confidence
        HOLD → 0

    The composite score is the weighted sum. If it exceeds +threshold → BUY,
    below -threshold → SELL, otherwise → HOLD.
    """

    def __init__(self, weights: dict[str, float] | None = None, threshold: float = DEFAULT_THRESHOLD):
        self.weights = dict(weights or DEFAULT_WEIGHTS)
        self.threshold = threshold
        # Normalize weights to sum to 1.0
        total = sum(self.weights.values())
        if total > 0:
            self.weights = {k: v / total for k, v in self.weights.items()}

    def score(self, symbol: str, signals: dict[str, Signal]) -> Signal:
        """Combine individual strategy signals into a single composite signal.

        Args:
            symbol: The ticker symbol.
            signals: Map of strategy_name → Signal from individual strategies.

        Returns:
            A composite Signal with trade_type="swing".
        """
        composite = 0.0
        contributing = []

        for strategy_name, weight in self.weights.items():
            sig = signals.get(strategy_name)
            if sig is None:
                continue

            if sig.direction == "BUY":
                directional = sig.confidence
            elif sig.direction == "SELL":
                directional = -sig.confidence
            else:
                directional = 0.0

            weighted = directional * weight
            composite += weighted

            if directional != 0.0:
                contributing.append(
                    f"{strategy_name}={sig.direction}({sig.confidence:.2f})*{weight:.2f}"
                )

        if composite > self.threshold:
            direction = "BUY"
        elif composite < -self.threshold:
            direction = "SELL"
        else:
            direction = "HOLD"

        confidence = min(abs(composite), 1.0)
        reason = f"Composite={composite:+.3f} [{', '.join(contributing) or 'no contributing signals'}]"

        return Signal(
            symbol=symbol,
            direction=direction,
            confidence=confidence,
            reason=reason,
            strategy_name="CompositeSwing",
            timestamp=datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            trade_type="swing",
        )
