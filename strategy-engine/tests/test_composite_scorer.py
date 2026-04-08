"""Tests for the CompositeScorer."""

from strategies.base import Signal
from strategies.composite_scorer import CompositeScorer


def _sig(direction: str, confidence: float, strategy_name: str) -> Signal:
    return Signal(
        symbol="AAPL",
        direction=direction,
        confidence=confidence,
        reason="test",
        strategy_name=strategy_name,
        timestamp="2026-03-17T00:00:00Z",
        trade_type="swing",
    )


def test_all_buy_signals_produce_buy():
    scorer = CompositeScorer(
        weights={"A": 0.5, "B": 0.5},
        threshold=0.3,
    )
    signals = {
        "A": _sig("BUY", 0.8, "A"),
        "B": _sig("BUY", 0.7, "B"),
    }
    result = scorer.score("AAPL", signals)
    assert result.direction == "BUY"
    assert result.confidence > 0.3
    assert result.trade_type == "swing"
    assert result.strategy_name == "CompositeSwing"


def test_all_sell_signals_produce_sell():
    scorer = CompositeScorer(
        weights={"A": 0.5, "B": 0.5},
        threshold=0.3,
    )
    signals = {
        "A": _sig("SELL", 0.8, "A"),
        "B": _sig("SELL", 0.7, "B"),
    }
    result = scorer.score("AAPL", signals)
    assert result.direction == "SELL"
    assert result.confidence > 0.3


def test_mixed_signals_below_threshold_produce_hold():
    scorer = CompositeScorer(
        weights={"A": 0.5, "B": 0.5},
        threshold=0.3,
    )
    signals = {
        "A": _sig("BUY", 0.5, "A"),
        "B": _sig("SELL", 0.4, "B"),
    }
    result = scorer.score("AAPL", signals)
    # Composite = 0.5*0.5 - 0.4*0.5 = 0.05, below threshold
    assert result.direction == "HOLD"


def test_missing_strategies_handled():
    scorer = CompositeScorer(
        weights={"A": 0.5, "B": 0.3, "C": 0.2},
        threshold=0.3,
    )
    # Only provide A — B and C missing
    signals = {
        "A": _sig("BUY", 0.9, "A"),
    }
    result = scorer.score("AAPL", signals)
    # Composite = 0.9 * (0.5/1.0) = 0.45 > 0.3
    assert result.direction == "BUY"


def test_all_hold_signals_produce_hold():
    scorer = CompositeScorer(
        weights={"A": 0.5, "B": 0.5},
        threshold=0.3,
    )
    signals = {
        "A": _sig("HOLD", 0.0, "A"),
        "B": _sig("HOLD", 0.0, "B"),
    }
    result = scorer.score("AAPL", signals)
    assert result.direction == "HOLD"
    assert result.confidence == 0.0


def test_weight_normalization():
    # Weights don't sum to 1 — should be normalized
    scorer = CompositeScorer(
        weights={"A": 2.0, "B": 3.0},
        threshold=0.3,
    )
    assert abs(sum(scorer.weights.values()) - 1.0) < 0.001


def test_reason_contains_contributing_signals():
    scorer = CompositeScorer(weights={"A": 1.0}, threshold=0.1)
    signals = {"A": _sig("BUY", 0.8, "A")}
    result = scorer.score("AAPL", signals)
    assert "A=BUY" in result.reason
    assert "Composite=" in result.reason


def test_confidence_capped_at_one():
    scorer = CompositeScorer(weights={"A": 1.0}, threshold=0.1)
    signals = {"A": _sig("BUY", 1.0, "A")}
    result = scorer.score("AAPL", signals)
    assert result.confidence <= 1.0


def test_empty_signals_produce_hold():
    scorer = CompositeScorer()
    result = scorer.score("AAPL", {})
    assert result.direction == "HOLD"
    assert result.confidence == 0.0


def test_high_confidence_bypass_triggers_buy():
    """A single swing strategy with confidence >= 0.80 should bypass the composite threshold."""
    scorer = CompositeScorer(
        weights={"RelativeStrength": 0.40, "MultiTimeframeTrend": 0.30, "Other": 0.30},
        threshold=0.20,
        high_confidence_bypass=0.80,
    )
    signals = {
        "RelativeStrength": _sig("BUY", 0.85, "RelativeStrength"),
        # Others are HOLD — composite = 0.85 * 0.40 = 0.34 > 0.20, but even if
        # it were below threshold, bypass would kick in
    }
    result = scorer.score("AAPL", signals)
    assert result.direction == "BUY"


def test_high_confidence_bypass_when_below_threshold():
    """Bypass should promote HOLD to BUY when a swing strategy has high confidence
    but composite is below threshold due to opposing signals."""
    scorer = CompositeScorer(
        weights={"RelativeStrength": 0.40, "Other": 0.60},
        threshold=0.20,
        high_confidence_bypass=0.80,
    )
    signals = {
        "RelativeStrength": _sig("BUY", 0.90, "RelativeStrength"),
        "Other": _sig("SELL", 0.50, "Other"),
    }
    # Composite = 0.90*0.40 - 0.50*0.60 = 0.36 - 0.30 = 0.06, below threshold
    result = scorer.score("AAPL", signals)
    assert result.direction == "BUY"
    assert "BYPASS" in result.reason


def test_no_bypass_for_non_swing_strategies():
    """Only MultiTimeframeTrend and RelativeStrength can trigger bypass."""
    scorer = CompositeScorer(
        weights={"RSIMeanReversion": 0.50, "MomentumVolume": 0.50},
        threshold=0.30,
        high_confidence_bypass=0.80,
    )
    signals = {
        "RSIMeanReversion": _sig("BUY", 0.95, "RSIMeanReversion"),
        "MomentumVolume": _sig("SELL", 0.90, "MomentumVolume"),
    }
    # Composite = 0.95*0.5 - 0.90*0.5 = 0.025, below threshold
    # No bypass because neither is a swing strategy
    result = scorer.score("AAPL", signals)
    assert result.direction == "HOLD"


def test_no_bypass_below_confidence_threshold():
    """Swing strategy with confidence below 0.80 should not trigger bypass."""
    scorer = CompositeScorer(
        weights={"RelativeStrength": 0.40, "Other": 0.60},
        threshold=0.30,
        high_confidence_bypass=0.80,
    )
    signals = {
        "RelativeStrength": _sig("BUY", 0.75, "RelativeStrength"),
        "Other": _sig("SELL", 0.60, "Other"),
    }
    # Composite = 0.75*0.4 - 0.60*0.6 = 0.30 - 0.36 = -0.06, below threshold
    # No bypass: RS confidence 0.75 < 0.80
    result = scorer.score("AAPL", signals)
    assert result.direction == "HOLD"
