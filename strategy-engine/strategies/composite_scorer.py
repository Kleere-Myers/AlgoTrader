"""Composite scoring framework — aggregates weighted signals from multiple strategies."""

from datetime import datetime, timezone

from strategies.base import Signal


# Default swing strategy weights — sum to 1.0
DEFAULT_WEIGHTS = {
    "MultiTimeframeTrend": 0.30,
    "RelativeStrength": 0.40,
    "RSIMeanReversion": 0.10,
    "MomentumVolume": 0.10,
    "NewsSentimentStrategy": 0.10,
}

# Minimum composite score to trigger BUY or SELL
DEFAULT_THRESHOLD = 0.20

# If any single swing strategy exceeds this confidence, bypass the composite threshold
HIGH_CONFIDENCE_BYPASS = 0.80


class CompositeScorer:
    """Aggregate weighted strategy signals into a single swing trading signal.

    Each strategy contributes a directional score:
        BUY  → +confidence
        SELL → -confidence
        HOLD → 0

    The composite score is the weighted sum. If it exceeds +threshold → BUY,
    below -threshold → SELL, otherwise → HOLD.
    """

    def __init__(
        self,
        weights: dict[str, float] | None = None,
        threshold: float = DEFAULT_THRESHOLD,
        high_confidence_bypass: float = HIGH_CONFIDENCE_BYPASS,
    ):
        self.weights = dict(weights or DEFAULT_WEIGHTS)
        self.threshold = threshold
        self.high_confidence_bypass = high_confidence_bypass
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

        # Check if any single swing strategy has high enough confidence to bypass
        bypass_signal = None
        swing_strategies = {"MultiTimeframeTrend", "RelativeStrength"}
        for name in swing_strategies:
            sig = signals.get(name)
            if sig and sig.direction != "HOLD" and sig.confidence >= self.high_confidence_bypass:
                if bypass_signal is None or sig.confidence > bypass_signal.confidence:
                    bypass_signal = sig

        if composite > self.threshold:
            direction = "BUY"
        elif composite < -self.threshold:
            direction = "SELL"
        elif bypass_signal is not None:
            direction = bypass_signal.direction
        else:
            direction = "HOLD"

        # Scale confidence to use the full 0–1 range. The raw composite is a
        # weighted average that structurally tops out around 0.6, so using it
        # directly as confidence makes it impossible to pass the execution
        # engine's min_composite_confidence gate. Instead, map the range
        # [threshold, 1.0] → [0.5, 1.0] so that a signal at the threshold
        # starts at 0.5 and stronger agreement pushes toward 1.0.
        raw = abs(composite)
        if raw > self.threshold:
            confidence = min(0.5 + 0.5 * (raw - self.threshold) / (1.0 - self.threshold), 1.0)
        else:
            confidence = min(raw / self.threshold * 0.5, 0.5) if self.threshold > 0 else raw

        bypass_note = ""
        if bypass_signal is not None and abs(composite) <= self.threshold:
            bypass_note = f" | BYPASS: {bypass_signal.strategy_name}={bypass_signal.direction}({bypass_signal.confidence:.2f})>={self.high_confidence_bypass}"
            confidence = max(confidence, bypass_signal.confidence)

        reason = f"Composite={composite:+.3f} [{', '.join(contributing) or 'no contributing signals'}]{bypass_note}"

        return Signal(
            symbol=symbol,
            direction=direction,
            confidence=confidence,
            reason=reason,
            strategy_name="CompositeSwing",
            timestamp=datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            trade_type="swing",
        )
