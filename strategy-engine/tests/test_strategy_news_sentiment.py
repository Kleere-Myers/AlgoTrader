"""Tests for NewsSentimentStrategy, sentiment scorer, and news fetcher."""

from unittest.mock import patch, MagicMock

import pandas as pd
import pytest

from strategies.base import BaseStrategy, Signal, BacktestResult
from strategies.strategy_news_sentiment import NewsSentimentStrategy


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def strategy():
    return NewsSentimentStrategy()


@pytest.fixture
def custom_strategy():
    return NewsSentimentStrategy(min_articles=2, bullish_threshold=0.2, bearish_threshold=-0.2)


def _make_bars(n: int = 20) -> pd.DataFrame:
    return pd.DataFrame({
        "timestamp": pd.date_range("2025-01-01", periods=n, freq="D"),
        "open": [100.0] * n,
        "high": [101.0] * n,
        "low": [99.0] * n,
        "close": [100.0] * n,
        "volume": [1000] * n,
    })


def _make_articles(sentiments: list[tuple[str, float]]) -> list[dict]:
    """Build scored article dicts with given (label, score) pairs."""
    articles = []
    for i, (label, score) in enumerate(sentiments):
        articles.append({
            "headline": f"Test headline {i}",
            "summary": f"Summary {i}",
            "source": "TestSource",
            "url": f"https://example.com/{i}",
            "published_at": "2025-01-01T00:00:00Z",
            "sentiment": label,
            "sentiment_score": score,
        })
    return articles


# ---------------------------------------------------------------------------
# BaseStrategy contract
# ---------------------------------------------------------------------------

class TestBaseStrategyContract:
    def test_extends_base_strategy(self):
        assert issubclass(NewsSentimentStrategy, BaseStrategy)

    def test_name_property(self, strategy):
        assert strategy.name == "NewsSentimentStrategy"

    def test_params_returns_dict(self, strategy):
        p = strategy.params()
        assert isinstance(p, dict)
        assert "min_articles" in p
        assert "bullish_threshold" in p
        assert "bearish_threshold" in p

    def test_default_params(self, strategy):
        assert strategy.params() == {
            "min_articles": 5,
            "bullish_threshold": 0.5,
            "bearish_threshold": -0.5,
        }

    def test_custom_params(self, custom_strategy):
        assert custom_strategy.params() == {
            "min_articles": 2,
            "bullish_threshold": 0.2,
            "bearish_threshold": -0.2,
        }


# ---------------------------------------------------------------------------
# Sentiment scorer
# ---------------------------------------------------------------------------

class TestSentimentScorer:
    @patch("strategies.sentiment._get_pipeline")
    def test_score_headline_positive(self, mock_get):
        mock_pipe = MagicMock(return_value=[{"label": "positive", "score": 0.95}])
        mock_get.return_value = mock_pipe

        from strategies.sentiment import score_headline
        label, score = score_headline("Stock surges on strong earnings")
        assert label == "positive"
        assert score == 0.95

    @patch("strategies.sentiment._get_pipeline")
    def test_score_headline_negative(self, mock_get):
        mock_pipe = MagicMock(return_value=[{"label": "negative", "score": 0.88}])
        mock_get.return_value = mock_pipe

        from strategies.sentiment import score_headline
        label, score = score_headline("Company faces massive lawsuit")
        assert label == "negative"
        assert score == -0.88

    @patch("strategies.sentiment._get_pipeline")
    def test_score_headline_neutral(self, mock_get):
        mock_pipe = MagicMock(return_value=[{"label": "neutral", "score": 0.7}])
        mock_get.return_value = mock_pipe

        from strategies.sentiment import score_headline
        label, score = score_headline("Company schedules meeting")
        assert label == "neutral"
        assert score == 0.0

    @patch("strategies.sentiment._get_pipeline")
    def test_score_articles_batch(self, mock_get):
        mock_pipe = MagicMock(return_value=[
            {"label": "positive", "score": 0.9},
            {"label": "negative", "score": 0.8},
        ])
        mock_get.return_value = mock_pipe

        from strategies.sentiment import score_articles
        articles = [
            {"headline": "Good news"},
            {"headline": "Bad news"},
        ]
        scored = score_articles(articles)
        assert len(scored) == 2
        assert scored[0]["sentiment"] == "positive"
        assert scored[0]["sentiment_score"] == 0.9
        assert scored[1]["sentiment"] == "negative"
        assert scored[1]["sentiment_score"] == -0.8

    @patch("strategies.sentiment._get_pipeline")
    def test_score_articles_empty(self, mock_get):
        from strategies.sentiment import score_articles
        result = score_articles([])
        assert result == []
        mock_get.assert_not_called()


# ---------------------------------------------------------------------------
# Signal generation
# ---------------------------------------------------------------------------

class TestGenerateSignal:
    @patch("strategies.strategy_news_sentiment.score_articles")
    @patch("strategies.strategy_news_sentiment.fetch_news")
    def test_buy_signal_on_bullish_news(self, mock_fetch, mock_score, strategy):
        mock_fetch.return_value = [{"headline": f"h{i}"} for i in range(5)]
        mock_score.return_value = _make_articles([
            ("positive", 0.8), ("positive", 0.7), ("positive", 0.6),
            ("neutral", 0.0), ("positive", 0.5),
        ])

        bars = _make_bars()
        signal = strategy.generate_signal(bars, "AAPL")
        assert isinstance(signal, Signal)
        assert signal.direction == "BUY"
        assert signal.confidence > 0
        assert signal.symbol == "AAPL"
        assert signal.strategy_name == "NewsSentimentStrategy"
        assert "Bullish" in signal.reason

    @patch("strategies.strategy_news_sentiment.score_articles")
    @patch("strategies.strategy_news_sentiment.fetch_news")
    def test_sell_signal_on_bearish_news(self, mock_fetch, mock_score, strategy):
        mock_fetch.return_value = [{"headline": f"h{i}"} for i in range(5)]
        mock_score.return_value = _make_articles([
            ("negative", -0.8), ("negative", -0.7), ("negative", -0.6),
            ("neutral", 0.0), ("negative", -0.5),
        ])

        bars = _make_bars()
        signal = strategy.generate_signal(bars, "MSFT")
        assert signal.direction == "SELL"
        assert signal.confidence > 0
        assert "Bearish" in signal.reason

    @patch("strategies.strategy_news_sentiment.score_articles")
    @patch("strategies.strategy_news_sentiment.fetch_news")
    def test_hold_signal_on_neutral_news(self, mock_fetch, mock_score, strategy):
        mock_fetch.return_value = [{"headline": f"h{i}"} for i in range(5)]
        mock_score.return_value = _make_articles([
            ("positive", 0.1), ("negative", -0.1), ("neutral", 0.0),
            ("positive", 0.05), ("negative", -0.05),
        ])

        bars = _make_bars()
        signal = strategy.generate_signal(bars, "SPY")
        assert signal.direction == "HOLD"
        assert "Neutral" in signal.reason

    @patch("strategies.strategy_news_sentiment.score_articles")
    @patch("strategies.strategy_news_sentiment.fetch_news")
    def test_confidence_capped_at_one(self, mock_fetch, mock_score, strategy):
        mock_fetch.return_value = [{"headline": f"h{i}"} for i in range(5)]
        mock_score.return_value = _make_articles([
            ("positive", 0.99), ("positive", 0.98), ("positive", 0.97),
            ("positive", 0.96), ("positive", 0.95),
        ])

        bars = _make_bars()
        signal = strategy.generate_signal(bars, "NVDA")
        assert signal.confidence <= 1.0


class TestInsufficientArticles:
    @patch("strategies.strategy_news_sentiment.score_articles")
    @patch("strategies.strategy_news_sentiment.fetch_news")
    def test_fewer_than_min_articles_returns_hold(self, mock_fetch, mock_score, strategy):
        mock_fetch.return_value = [{"headline": "h1"}]
        mock_score.return_value = _make_articles([("positive", 0.9)])

        bars = _make_bars()
        signal = strategy.generate_signal(bars, "AAPL")
        assert signal.direction == "HOLD"
        assert signal.confidence == 0.0
        assert "Insufficient" in signal.reason

    @patch("strategies.strategy_news_sentiment.score_articles")
    @patch("strategies.strategy_news_sentiment.fetch_news")
    def test_zero_articles_returns_hold(self, mock_fetch, mock_score, strategy):
        mock_fetch.return_value = []
        mock_score.return_value = []

        bars = _make_bars()
        signal = strategy.generate_signal(bars, "GOOGL")
        assert signal.direction == "HOLD"
        assert signal.confidence == 0.0


# ---------------------------------------------------------------------------
# Backtesting
# ---------------------------------------------------------------------------

class TestBacktest:
    def test_returns_valid_backtest_result(self, strategy):
        bars = _make_bars()
        result = strategy.backtest(bars, "SPY")
        assert isinstance(result, BacktestResult)

    def test_backtest_result_zeroed(self, strategy):
        bars = _make_bars()
        result = strategy.backtest(bars, "TEST")
        assert result.strategy_name == "NewsSentimentStrategy"
        assert result.symbol == "TEST"
        assert result.total_return_pct == 0.0
        assert result.sharpe_ratio == 0.0
        assert result.max_drawdown_pct == 0.0
        assert result.win_rate == 0.0
        assert result.total_trades == 0
        assert result.profit_factor == 0.0

    def test_backtest_to_dict(self, strategy):
        bars = _make_bars()
        result = strategy.backtest(bars, "SPY")
        d = result.to_dict()
        assert isinstance(d, dict)
        assert d["strategy_name"] == "NewsSentimentStrategy"
        assert "total_return_pct" in d

    def test_backtest_period_dates(self, strategy):
        bars = _make_bars()
        result = strategy.backtest(bars, "SPY")
        assert result.period_start != ""
        assert result.period_end != ""
