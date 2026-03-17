"""NewsSentimentStrategy — uses FinBERT to analyze news headlines for trading signals."""

import pandas as pd

from strategies.base import BacktestResult, BaseStrategy, Signal
from strategies.news_fetcher import fetch_news
from strategies.sentiment import score_articles


class NewsSentimentStrategy(BaseStrategy):
    def __init__(
        self,
        min_articles: int = 3,
        bullish_threshold: float = 0.3,
        bearish_threshold: float = -0.3,
    ):
        self.min_articles = min_articles
        self.bullish_threshold = bullish_threshold
        self.bearish_threshold = bearish_threshold

    @property
    def name(self) -> str:
        return "NewsSentimentStrategy"

    def params(self) -> dict:
        return {
            "min_articles": self.min_articles,
            "bullish_threshold": self.bullish_threshold,
            "bearish_threshold": self.bearish_threshold,
        }

    def generate_signal(self, bars: pd.DataFrame, symbol: str) -> Signal:
        articles = fetch_news(symbol)
        scored = score_articles(articles)

        if len(scored) < self.min_articles:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason=f"Insufficient news ({len(scored)} articles, need {self.min_articles})",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        avg_score = sum(a["sentiment_score"] for a in scored) / len(scored)

        if avg_score > self.bullish_threshold:
            return Signal(
                symbol=symbol,
                direction="BUY",
                confidence=round(min(abs(avg_score), 1.0), 4),
                reason=f"Bullish news sentiment: avg={avg_score:.3f} across {len(scored)} articles",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        if avg_score < self.bearish_threshold:
            return Signal(
                symbol=symbol,
                direction="SELL",
                confidence=round(min(abs(avg_score), 1.0), 4),
                reason=f"Bearish news sentiment: avg={avg_score:.3f} across {len(scored)} articles",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        return Signal(
            symbol=symbol,
            direction="HOLD",
            confidence=0.0,
            reason=f"Neutral news sentiment: avg={avg_score:.3f} across {len(scored)} articles",
            strategy_name=self.name,
            timestamp=self._now_iso(),
        )

    def backtest(self, bars: pd.DataFrame, symbol: str) -> BacktestResult:
        """No historical news data to backtest against — return zeroed result."""
        period_start = str(bars["timestamp"].iloc[0]) if not bars.empty else ""
        period_end = str(bars["timestamp"].iloc[-1]) if not bars.empty else ""

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
            period_start=period_start,
            period_end=period_end,
        )
