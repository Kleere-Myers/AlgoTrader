"""FastAPI app entrypoint — Strategy Engine."""

import logging
import os
from contextlib import asynccontextmanager
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import duckdb
import pandas as pd
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

from strategies.strategy_moving_average import MovingAverageCrossover
from strategies.strategy_rsi import RSIMeanReversion
from strategies.strategy_momentum_volume import MomentumVolume
from strategies.strategy_ml_signal import MLSignalGenerator
from strategies.strategy_vwap import VWAPStrategy
from strategies.strategy_orb import OpeningRangeBreakout
from strategies.base import Signal
from strategies.strategy_news_sentiment import NewsSentimentStrategy
from strategies.strategy_multi_timeframe import MultiTimeframeTrendAlignment
from strategies.strategy_relative_strength import RelativeStrengthRanking
from strategies.composite_scorer import CompositeScorer

logger = logging.getLogger(__name__)

DB_PATH = os.environ.get(
    "DUCKDB_PATH",
    str(Path(__file__).resolve().parent.parent / "data" / "algotrader.duckdb"),
)

DEFAULT_SYMBOLS = ["SPY", "QQQ", "AAPL", "MSFT", "NVDA", "GOOGL"]
SYMBOLS = [s.strip().upper() for s in os.environ.get("SYMBOLS", ",".join(DEFAULT_SYMBOLS)).split(",") if s.strip()]

# Strategy registry — add new strategies here
_ml_strategy = MLSignalGenerator()

STRATEGIES = {
    "MovingAverageCrossover": MovingAverageCrossover(),
    "RSIMeanReversion": RSIMeanReversion(),
    "MomentumVolume": MomentumVolume(),
    "MLSignalGenerator": _ml_strategy,
    "VWAPStrategy": VWAPStrategy(),
    "OpeningRangeBreakout": OpeningRangeBreakout(),
    "NewsSentimentStrategy": NewsSentimentStrategy(),
}

# Swing trading strategies (not included in 5-min intraday signal loop)
SWING_STRATEGIES = {
    "MultiTimeframeTrend": MultiTimeframeTrendAlignment(),
    "RelativeStrength": RelativeStrengthRanking(),
}

_composite_scorer = CompositeScorer()


def _retrain_ml_model():
    """Weekly retraining job for MLSignalGenerator."""
    from ml.train import train_model
    try:
        result = train_model()
        _ml_strategy.reload_model()
        logger.info("ML model retrained: %s", result)
    except Exception:
        logger.exception("ML retraining failed")


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Startup/shutdown lifecycle — register APScheduler jobs."""
    from apscheduler.schedulers.asyncio import AsyncIOScheduler
    from apscheduler.triggers.cron import CronTrigger

    scheduler = AsyncIOScheduler()
    # Retrain every Sunday at 6 PM ET (23:00 UTC in EST, 22:00 UTC in EDT)
    scheduler.add_job(
        _retrain_ml_model,
        CronTrigger(day_of_week="sun", hour=23, minute=0, timezone="US/Eastern"),
        id="ml_retrain_weekly",
        replace_existing=True,
    )
    scheduler.start()
    logger.info("APScheduler started — ML retraining scheduled Sunday 6 PM ET")
    yield
    scheduler.shutdown()


from fastapi.middleware.cors import CORSMiddleware

app = FastAPI(title="AlgoTrader Strategy Engine", version="0.1.0", lifespan=lifespan)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000"],
    allow_methods=["*"],
    allow_headers=["*"],
)

# In-memory cache of last backtest results
_backtest_cache: dict[str, dict] = {}


def _cache_key(strategy: str, symbol: str) -> str:
    return f"{strategy}:{symbol}"


def _get_db(read_only: bool = True):
    return duckdb.connect(DB_PATH, read_only=read_only)


# --- POST /signal models ---

class BarData(BaseModel):
    timestamp: str
    open: float
    high: float
    low: float
    close: float
    volume: int


class SignalRequest(BaseModel):
    symbol: str
    bars: list[BarData]


# --- Endpoints ---

@app.get("/health")
def health():
    return {"status": "ok"}


class SymbolRequest(BaseModel):
    symbol: str


@app.get("/symbols")
def get_symbols():
    return {"symbols": SYMBOLS}


@app.post("/symbols", status_code=201)
def add_symbol(req: SymbolRequest):
    symbol = req.symbol.strip().upper()
    if not symbol:
        raise HTTPException(status_code=400, detail="Symbol cannot be empty")
    if len(symbol) > 10:
        raise HTTPException(status_code=400, detail="Symbol too long")
    if symbol in SYMBOLS:
        raise HTTPException(status_code=409, detail=f"Symbol '{symbol}' already exists")
    SYMBOLS.append(symbol)
    logger.info("Symbol added: %s — active list: %s", symbol, SYMBOLS)
    return {"symbols": SYMBOLS}


@app.delete("/symbols/{symbol}")
def remove_symbol(symbol: str):
    symbol = symbol.strip().upper()
    if symbol not in SYMBOLS:
        raise HTTPException(status_code=404, detail=f"Symbol '{symbol}' not found")
    if len(SYMBOLS) <= 1:
        raise HTTPException(status_code=400, detail="Cannot remove the last symbol")
    SYMBOLS.remove(symbol)
    logger.info("Symbol removed: %s — active list: %s", symbol, SYMBOLS)
    return {"symbols": SYMBOLS}


@app.get("/bars/{symbol}")
def get_bars(symbol: str, limit: int = 100):
    """Return recent OHLCV bars for a symbol from DuckDB."""
    symbol = symbol.upper()
    con = _get_db()
    try:
        bars = con.execute(
            "SELECT timestamp, open, high, low, close, volume "
            "FROM ohlcv_bars WHERE symbol = ? ORDER BY timestamp DESC LIMIT ?",
            [symbol, limit],
        ).fetchall()
    finally:
        con.close()

    return [
        {
            "timestamp": str(r[0]),
            "open": r[1],
            "high": r[2],
            "low": r[3],
            "close": r[4],
            "volume": int(r[5]),
        }
        for r in reversed(bars)
    ]


@app.get("/strategies")
def list_strategies():
    day_strats = [
        {"name": s.name, "enabled": True, "params": s.params(), "trade_type": "day"}
        for s in STRATEGIES.values()
    ]
    swing_strats = [
        {"name": s.name, "enabled": True, "params": s.params(), "trade_type": "swing"}
        for s in SWING_STRATEGIES.values()
    ]
    return day_strats + swing_strats


@app.post("/signal")
def generate_signals(req: SignalRequest):
    if not req.bars:
        raise HTTPException(status_code=400, detail="No bars provided")

    df = pd.DataFrame([b.model_dump() for b in req.bars])
    df["timestamp"] = pd.to_datetime(df["timestamp"])
    df = df.sort_values("timestamp").reset_index(drop=True)

    signals = []
    for strat in STRATEGIES.values():
        try:
            sig = strat.generate_signal(df, req.symbol.upper())
        except Exception as exc:
            logger.warning("Strategy %s failed for %s: %s", strat.name, req.symbol, exc)
            sig = Signal(
                symbol=req.symbol.upper(),
                direction="HOLD",
                confidence=0.0,
                reason=f"Strategy error: {exc}",
                strategy_name=strat.name,
                timestamp=datetime.now(timezone.utc).isoformat(),
            )
        signals.append(sig.to_dict())

    # Write signals to DuckDB for audit trail
    con = _get_db(read_only=False)
    try:
        for sig in signals:
            con.execute(
                "INSERT INTO signals (strategy_name, symbol, timestamp, direction, confidence, reason, trade_type) "
                "VALUES (?, ?, ?, ?, ?, ?, ?)",
                [sig["strategy_name"], sig["symbol"], sig["timestamp"],
                 sig["direction"], sig["confidence"], sig["reason"],
                 sig.get("trade_type", "day")],
            )
    finally:
        con.close()

    return {"signals": signals}


class SwingSignalRequest(BaseModel):
    symbol: str
    bars_daily: list[BarData]


@app.post("/signal/swing")
def generate_swing_signal(req: SwingSignalRequest):
    """Generate a composite swing trading signal from daily bars."""
    if not req.bars_daily:
        raise HTTPException(status_code=400, detail="No daily bars provided")

    df_daily = pd.DataFrame([b.model_dump() for b in req.bars_daily])
    df_daily["timestamp"] = pd.to_datetime(df_daily["timestamp"])
    df_daily = df_daily.sort_values("timestamp").reset_index(drop=True)

    symbol = req.symbol.upper()

    # Run swing-specific strategies on daily bars
    individual_signals: dict[str, Signal] = {}
    for strat_name, strat in SWING_STRATEGIES.items():
        try:
            sig = strat.generate_signal(df_daily, symbol)
        except Exception as exc:
            logger.warning("Swing strategy %s failed for %s: %s", strat_name, symbol, exc)
            sig = Signal(
                symbol=symbol, direction="HOLD", confidence=0.0,
                reason=f"Strategy error: {exc}", strategy_name=strat_name,
                timestamp=datetime.now(timezone.utc).isoformat(), trade_type="swing",
            )
        individual_signals[strat_name] = sig

    # Also run compatible day strategies on daily bars for composite scoring
    for strat_name in ["RSIMeanReversion", "MomentumVolume", "NewsSentimentStrategy"]:
        strat = STRATEGIES.get(strat_name)
        if strat is None:
            continue
        try:
            sig = strat.generate_signal(df_daily, symbol)
        except Exception as exc:
            logger.warning("Strategy %s failed for swing composite: %s", strat_name, exc)
            sig = Signal(
                symbol=symbol, direction="HOLD", confidence=0.0,
                reason=f"Strategy error: {exc}", strategy_name=strat_name,
                timestamp=datetime.now(timezone.utc).isoformat(), trade_type="swing",
            )
        individual_signals[strat_name] = sig

    # Composite scoring
    composite = _composite_scorer.score(symbol, individual_signals)

    # Write all signals to DuckDB for audit trail
    all_signals = [composite.to_dict()] + [s.to_dict() for s in individual_signals.values()]
    con = _get_db(read_only=False)
    try:
        for sig in all_signals:
            con.execute(
                "INSERT INTO signals (strategy_name, symbol, timestamp, direction, confidence, reason, trade_type) "
                "VALUES (?, ?, ?, ?, ?, ?, ?)",
                [sig["strategy_name"], sig["symbol"], sig["timestamp"],
                 sig["direction"], sig["confidence"], sig["reason"],
                 sig.get("trade_type", "swing")],
            )
    finally:
        con.close()

    return {
        "composite": composite.to_dict(),
        "individual": {name: sig.to_dict() for name, sig in individual_signals.items()},
    }


@app.post("/backtest/{strategy}/{symbol}")
def run_backtest(strategy: str, symbol: str):
    if strategy not in STRATEGIES:
        raise HTTPException(status_code=404, detail=f"Strategy '{strategy}' not found")

    symbol = symbol.upper()
    con = _get_db()
    try:
        bars = con.execute(
            "SELECT symbol, timestamp, open, high, low, close, volume "
            "FROM ohlcv_bars WHERE symbol = ? AND bar_size = '1d' ORDER BY timestamp",
            [symbol],
        ).fetchdf()
    finally:
        con.close()

    if bars.empty:
        raise HTTPException(status_code=404, detail=f"No OHLCV data for symbol '{symbol}'")

    strat = STRATEGIES[strategy]
    result = strat.backtest(bars, symbol)
    _backtest_cache[_cache_key(strategy, symbol)] = result.to_dict()
    return result.to_dict()


@app.get("/backtest/{strategy}/{symbol}")
def get_backtest(strategy: str, symbol: str):
    symbol = symbol.upper()
    key = _cache_key(strategy, symbol)
    if key not in _backtest_cache:
        raise HTTPException(status_code=404, detail="No backtest result cached — run POST first")
    return _backtest_cache[key]


@app.get("/strategies/{strategy_id}/performance")
def get_strategy_performance(strategy_id: str):
    if strategy_id not in STRATEGIES:
        raise HTTPException(status_code=404, detail=f"Strategy '{strategy_id}' not found")

    con = _get_db()
    try:
        row = con.execute(
            "SELECT "
            "  count(*) AS total_signals, "
            "  count(*) FILTER (WHERE direction = 'BUY') AS buy_signals, "
            "  count(*) FILTER (WHERE direction = 'SELL') AS sell_signals, "
            "  count(*) FILTER (WHERE direction = 'HOLD') AS hold_signals, "
            "  coalesce(avg(confidence), 0) AS avg_confidence "
            "FROM signals "
            "WHERE strategy_name = ? AND timestamp >= current_date - INTERVAL '30 days'",
            [strategy_id],
        ).fetchone()

        by_symbol_rows = con.execute(
            "SELECT symbol, count(*) AS cnt "
            "FROM signals "
            "WHERE strategy_name = ? AND timestamp >= current_date - INTERVAL '30 days' "
            "GROUP BY symbol ORDER BY symbol",
            [strategy_id],
        ).fetchall()
    finally:
        con.close()

    return {
        "strategy_name": strategy_id,
        "total_signals": int(row[0]),
        "buy_signals": int(row[1]),
        "sell_signals": int(row[2]),
        "hold_signals": int(row[3]),
        "avg_confidence": round(float(row[4]), 4),
        "signals_by_symbol": {sym: int(cnt) for sym, cnt in by_symbol_rows},
    }


# ---------------------------------------------------------------------------
# Company info + News endpoints
# ---------------------------------------------------------------------------

import time as _time

_company_cache: dict[str, tuple[float, dict]] = {}
_COMPANY_CACHE_TTL = 900  # 15 minutes


@app.get("/company/{symbol}")
def get_company_info(symbol: str):
    """Return company details from yfinance with 15-minute TTL cache."""
    symbol = symbol.upper()
    now = _time.time()

    if symbol in _company_cache:
        cached_at, info = _company_cache[symbol]
        if now - cached_at < _COMPANY_CACHE_TTL:
            return info

    try:
        import yfinance as yf
        ticker = yf.Ticker(symbol)
        raw = ticker.info or {}
    except Exception as exc:
        raise HTTPException(status_code=502, detail=f"yfinance lookup failed: {exc}")

    if not raw or raw.get("symbol") is None:
        raise HTTPException(status_code=404, detail=f"Symbol '{symbol}' not found")

    prev_close = raw.get("previousClose") or raw.get("regularMarketPreviousClose")
    current = raw.get("currentPrice") or raw.get("regularMarketPrice")
    change_pct = None
    if current and prev_close and prev_close > 0:
        change_pct = round((current - prev_close) / prev_close * 100, 2)

    result = {
        "symbol": symbol,
        "name": raw.get("shortName") or raw.get("longName") or symbol,
        "sector": raw.get("sector"),
        "industry": raw.get("industry"),
        "market_cap": raw.get("marketCap"),
        "summary": raw.get("longBusinessSummary"),
        "current_price": current,
        "previous_close": prev_close,
        "change_pct": change_pct,
        "fifty_two_week_high": raw.get("fiftyTwoWeekHigh"),
        "fifty_two_week_low": raw.get("fiftyTwoWeekLow"),
        "average_volume": raw.get("averageVolume"),
    }

    _company_cache[symbol] = (now, result)
    return result


@app.get("/news/{symbol}")
def get_news(symbol: str):
    """Return recent news with FinBERT sentiment scores."""
    symbol = symbol.upper()

    from strategies.news_fetcher import fetch_news as _fetch_news
    from strategies.sentiment import score_articles as _score_articles

    articles = _fetch_news(symbol)
    scored = _score_articles(articles)

    return {"symbol": symbol, "articles": scored}
