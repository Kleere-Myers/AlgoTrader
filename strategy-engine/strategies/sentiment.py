"""FinBERT sentiment scorer — lazy-loads model on first call."""

import logging
from typing import Any

logger = logging.getLogger(__name__)

_pipeline = None


def _get_pipeline():
    """Lazy-load FinBERT pipeline (avoids startup cost if unused)."""
    global _pipeline
    if _pipeline is None:
        from transformers import pipeline as hf_pipeline

        logger.info("Loading FinBERT model (first call — may take a moment)...")
        _pipeline = hf_pipeline(
            "sentiment-analysis",
            model="ProsusAI/finbert",
            truncation=True,
        )
        logger.info("FinBERT model loaded.")
    return _pipeline


def score_headline(headline: str) -> tuple[str, float]:
    """Score a single headline.

    Returns (label, score) where label is "positive"/"negative"/"neutral"
    and score is a float in [-1.0, 1.0].
    """
    pipe = _get_pipeline()
    result = pipe(headline)[0]
    label = result["label"].lower()  # positive, negative, neutral
    raw_score = result["score"]

    if label == "positive":
        score = raw_score
    elif label == "negative":
        score = -raw_score
    else:
        score = 0.0

    return label, round(score, 4)


def score_articles(articles: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Add 'sentiment' and 'sentiment_score' fields to each article dict.

    Batch-scores headlines for efficiency.
    """
    if not articles:
        return articles

    pipe = _get_pipeline()
    headlines = [a.get("headline", "") or "" for a in articles]

    # Filter out empty headlines to avoid pipeline errors
    non_empty_indices = [i for i, h in enumerate(headlines) if h.strip()]

    results_map: dict[int, dict] = {}
    if non_empty_indices:
        non_empty_headlines = [headlines[i] for i in non_empty_indices]
        raw_results = pipe(non_empty_headlines)
        for idx, res in zip(non_empty_indices, raw_results):
            results_map[idx] = res

    scored = []
    for i, article in enumerate(articles):
        enriched = dict(article)
        if i in results_map:
            res = results_map[i]
            label = res["label"].lower()
            raw_score = res["score"]
            if label == "positive":
                enriched["sentiment_score"] = round(raw_score, 4)
            elif label == "negative":
                enriched["sentiment_score"] = round(-raw_score, 4)
            else:
                enriched["sentiment_score"] = 0.0
            enriched["sentiment"] = label
        else:
            enriched["sentiment"] = "neutral"
            enriched["sentiment_score"] = 0.0
        scored.append(enriched)

    return scored
