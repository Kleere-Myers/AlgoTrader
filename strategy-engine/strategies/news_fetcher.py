"""Shared news fetcher — Alpaca News API (primary) + yfinance (fallback)."""

import logging
import os
import time
from typing import Any

logger = logging.getLogger(__name__)

# Module-level cache: {symbol: (timestamp, articles)}
_news_cache: dict[str, tuple[float, list[dict[str, Any]]]] = {}
_CACHE_TTL_SECONDS = 300  # 5 minutes


def fetch_news(symbol: str, limit: int = 10) -> list[dict[str, Any]]:
    """Fetch recent news articles for a symbol.

    Primary: Alpaca News API.  Fallback: yfinance.
    Results are cached per-symbol with a 5-minute TTL.
    """
    now = time.time()
    if symbol in _news_cache:
        cached_at, articles = _news_cache[symbol]
        if now - cached_at < _CACHE_TTL_SECONDS:
            return articles[:limit]

    articles = _fetch_alpaca(symbol, limit)
    if not articles:
        articles = _fetch_yfinance(symbol, limit)

    _news_cache[symbol] = (now, articles)
    return articles[:limit]


def _fetch_alpaca(symbol: str, limit: int) -> list[dict[str, Any]]:
    api_key = os.environ.get("ALPACA_API_KEY", "")
    secret_key = os.environ.get("ALPACA_SECRET_KEY", "")
    if not api_key or not secret_key:
        return []

    try:
        from alpaca.data.historical.news import NewsClient
        from alpaca.data.requests import NewsRequest

        client = NewsClient(api_key=api_key, secret_key=secret_key)
        request = NewsRequest(symbols=symbol, limit=limit, sort="desc")
        news = client.get_news(request)

        articles = []
        for item in news.news:
            articles.append({
                "headline": item.headline,
                "summary": getattr(item, "summary", None) or "",
                "source": getattr(item, "source", None) or "",
                "url": getattr(item, "url", None) or "",
                "published_at": str(item.created_at) if item.created_at else None,
            })
        return articles
    except Exception:
        logger.debug("Alpaca news fetch failed for %s, falling back to yfinance", symbol)
        return []


def _fetch_yfinance(symbol: str, limit: int) -> list[dict[str, Any]]:
    try:
        import yfinance as yf

        ticker = yf.Ticker(symbol)
        raw_news = ticker.news or []

        articles = []
        for item in raw_news[:limit]:
            articles.append({
                "headline": item.get("title", ""),
                "summary": item.get("summary", "") or "",
                "source": item.get("publisher", "") or "",
                "url": item.get("link", "") or "",
                "published_at": None,
            })
        return articles
    except Exception:
        logger.debug("yfinance news fetch failed for %s", symbol)
        return []
