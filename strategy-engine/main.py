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
    allow_origins=["http://localhost:9102"],
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


_history_cache: dict[str, tuple[float, list]] = {}
_HISTORY_CACHE_TTL = 300  # 5 minutes

_HISTORY_RANGES = {
    "1d":  {"period": "1d",  "interval": "5m"},
    "5d":  {"period": "5d",  "interval": "15m"},
    "1m":  {"period": "1mo", "interval": "1d"},
    "6m":  {"period": "6mo", "interval": "1d"},
    "1y":  {"period": "1y",  "interval": "1d"},
    "5y":  {"period": "5y",  "interval": "1wk"},
}


@app.get("/bars/{symbol}/history")
def get_historical_bars(symbol: str, range: str = "1d"):
    """Return historical OHLCV bars from yfinance for charting."""
    symbol = symbol.upper()
    if range not in _HISTORY_RANGES:
        raise HTTPException(
            status_code=400,
            detail=f"Invalid range '{range}'. Must be one of: {', '.join(_HISTORY_RANGES)}",
        )

    now = _time.time()
    cache_key = f"{symbol}:{range}"
    if cache_key in _history_cache:
        cached_at, data = _history_cache[cache_key]
        if now - cached_at < _HISTORY_CACHE_TTL:
            return data

    try:
        import yfinance as yf
        params = _HISTORY_RANGES[range]
        df = yf.Ticker(symbol).history(period=params["period"], interval=params["interval"])
    except Exception as exc:
        raise HTTPException(status_code=502, detail=f"yfinance history failed: {exc}")

    bars = [
        {
            "timestamp": str(idx),
            "open": round(float(row["Open"]), 4),
            "high": round(float(row["High"]), 4),
            "low": round(float(row["Low"]), 4),
            "close": round(float(row["Close"]), 4),
            "volume": int(row["Volume"]),
        }
        for idx, row in df.iterrows()
    ]

    _history_cache[cache_key] = (now, bars)
    return bars


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
        # Extended quote fields
        "trailing_pe": raw.get("trailingPE"),
        "forward_pe": raw.get("forwardPE"),
        "eps": raw.get("trailingEps"),
        "beta": raw.get("beta"),
        "dividend_rate": raw.get("dividendRate"),
        "dividend_yield": raw.get("dividendYield"),
        "payout_ratio": raw.get("payoutRatio"),
        "open": raw.get("open") or raw.get("regularMarketOpen"),
        "day_high": raw.get("dayHigh") or raw.get("regularMarketDayHigh"),
        "day_low": raw.get("dayLow") or raw.get("regularMarketDayLow"),
        "volume": raw.get("volume") or raw.get("regularMarketVolume"),
        "bid": raw.get("bid"),
        "ask": raw.get("ask"),
        "bid_size": raw.get("bidSize"),
        "ask_size": raw.get("askSize"),
        "target_mean_price": raw.get("targetMeanPrice"),
        "exchange": raw.get("exchange"),
        "currency": raw.get("currency"),
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


# ---------------------------------------------------------------------------
# Market data endpoints (for dashboard Overview)
# ---------------------------------------------------------------------------

_market_cache: dict[str, tuple[float, Any]] = {}
_MARKET_CACHE_TTL = 60  # 1 minute


def _cached_market(key: str, ttl: int = _MARKET_CACHE_TTL):
    """Check market cache. Returns (hit, data)."""
    now = _time.time()
    if key in _market_cache:
        cached_at, data = _market_cache[key]
        if now - cached_at < ttl:
            return True, data
    return False, None


@app.get("/market/indices")
def get_market_indices():
    """Return market summary data matching Yahoo Finance layout — indices, VIX, bonds, commodities, crypto."""
    hit, data = _cached_market("indices")
    if hit:
        return data

    import yfinance as yf

    markets = [
        ("^GSPC", "S&P 500"),
        ("^DJI", "Dow 30"),
        ("^IXIC", "Nasdaq"),
        ("^RUT", "Russell 2000"),
        ("^VIX", "VIX"),
        ("^TNX", "10-Yr Bond"),
        ("GC=F", "Gold"),
        ("CL=F", "Crude Oil"),
        ("BTC-USD", "Bitcoin"),
        ("EURUSD=X", "EUR/USD"),
    ]
    result = []
    for symbol, name in markets:
        try:
            ticker = yf.Ticker(symbol)
            hist = ticker.history(period="1d", interval="5m")
            info = ticker.info or {}

            prev_close = info.get("previousClose") or info.get("regularMarketPreviousClose") or 0
            current = info.get("currentPrice") or info.get("regularMarketPrice") or 0
            if not current and not hist.empty:
                current = float(hist["Close"].iloc[-1])

            change_abs = current - prev_close if prev_close else 0
            change_pct = (change_abs / prev_close * 100) if prev_close else 0

            # Downsample intraday to ~30 points for compact sparklines
            intraday = []
            if not hist.empty:
                step = max(1, len(hist) // 30)
                for i in range(0, len(hist), step):
                    row = hist.iloc[i]
                    intraday.append({"value": round(float(row["Close"]), 4)})

            result.append({
                "symbol": symbol,
                "name": name,
                "current_price": round(current, 4) if "USD" in symbol or symbol == "^TNX" else round(current, 2),
                "previous_close": round(prev_close, 4) if "USD" in symbol or symbol == "^TNX" else round(prev_close, 2),
                "change_abs": round(change_abs, 2),
                "change_pct": round(change_pct, 2),
                "intraday_prices": intraday,
            })
        except Exception as exc:
            logger.warning("Failed to fetch market data for %s: %s", symbol, exc)
            result.append({
                "symbol": symbol, "name": name,
                "current_price": 0, "previous_close": 0,
                "change_abs": 0, "change_pct": 0, "intraday_prices": [],
            })

    _market_cache["indices"] = (_time.time(), result)
    return result


@app.get("/market/sectors")
def get_market_sectors():
    """Return sector ETF daily performance."""
    hit, data = _cached_market("sectors", ttl=300)
    if hit:
        return data

    import yfinance as yf

    sector_map = [
        ("XLK", "Technology"), ("XLF", "Financials"), ("XLE", "Energy"),
        ("XLV", "Healthcare"), ("XLY", "Consumer Disc"), ("XLP", "Consumer Staples"),
        ("XLI", "Industrials"), ("XLB", "Materials"), ("XLRE", "Real Estate"),
        ("XLU", "Utilities"), ("XLC", "Communication"),
    ]
    symbols = [s for s, _ in sector_map]
    result = []

    try:
        tickers = yf.Tickers(" ".join(symbols))
        for symbol, sector in sector_map:
            try:
                info = tickers.tickers[symbol].info or {}
                prev = info.get("previousClose") or info.get("regularMarketPreviousClose") or 0
                curr = info.get("currentPrice") or info.get("regularMarketPrice") or 0
                change_pct = ((curr - prev) / prev * 100) if prev else 0
                result.append({"sector": sector, "symbol": symbol, "change_pct": round(change_pct, 2)})
            except Exception:
                result.append({"sector": sector, "symbol": symbol, "change_pct": 0.0})
    except Exception as exc:
        logger.warning("Failed to fetch sector data: %s", exc)

    result.sort(key=lambda x: x["change_pct"], reverse=True)
    _market_cache["sectors"] = (_time.time(), result)
    return result


@app.get("/market/movers")
def get_market_movers():
    """Return top gainers and losers from tracked symbols."""
    hit, data = _cached_market("movers")
    if hit:
        return data

    movers = []
    for symbol in SYMBOLS:
        now = _time.time()
        if symbol in _company_cache:
            cached_at, info = _company_cache[symbol]
            if now - cached_at < _COMPANY_CACHE_TTL:
                movers.append({
                    "symbol": symbol,
                    "name": info.get("name", symbol),
                    "current_price": info.get("current_price"),
                    "change_pct": info.get("change_pct", 0) or 0,
                })
                continue

        try:
            import yfinance as yf
            ticker = yf.Ticker(symbol)
            raw = ticker.info or {}
            prev = raw.get("previousClose") or raw.get("regularMarketPreviousClose") or 0
            curr = raw.get("currentPrice") or raw.get("regularMarketPrice") or 0
            pct = ((curr - prev) / prev * 100) if prev else 0
            movers.append({
                "symbol": symbol,
                "name": raw.get("shortName") or symbol,
                "current_price": round(curr, 2) if curr else None,
                "change_pct": round(pct, 2),
            })
        except Exception:
            movers.append({"symbol": symbol, "name": symbol, "current_price": None, "change_pct": 0})

    sorted_movers = sorted(movers, key=lambda x: x["change_pct"], reverse=True)
    gainers = [m for m in sorted_movers if m["change_pct"] > 0]
    losers = list(reversed([m for m in sorted_movers if m["change_pct"] <= 0]))

    result = {"gainers": gainers, "losers": losers}
    _market_cache["movers"] = (_time.time(), result)
    return result


@app.get("/portfolio/pnl-history")
def get_pnl_history(range: str = "1d"):
    """Return portfolio P&L time series and summary for the given range.

    Uses Alpaca's portfolio history API for real equity curve data.
    """
    import httpx

    alpaca_key = os.environ.get("ALPACA_API_KEY", "")
    alpaca_secret = os.environ.get("ALPACA_SECRET_KEY", "")
    alpaca_mode = os.environ.get("ALPACA_MODE", "paper")
    base_url = (
        "https://api.alpaca.markets"
        if alpaca_mode == "live"
        else "https://paper-api.alpaca.markets"
    )
    headers = {
        "APCA-API-KEY-ID": alpaca_key,
        "APCA-API-SECRET-KEY": alpaca_secret,
    }

    exec_url = os.environ.get("EXECUTION_ENGINE_URL", "http://localhost:9101")

    # Fetch account + positions from execution engine
    try:
        acct = httpx.get(f"{exec_url}/account", timeout=5).json()
    except Exception:
        acct = {}
    try:
        positions = httpx.get(f"{exec_url}/positions", timeout=5).json()
    except Exception:
        positions = []

    equity = acct.get("equity", 0)
    buying_power = acct.get("buying_power", 0)
    cash = acct.get("cash", 0)
    day_positions = sum(1 for p in positions if p.get("trade_type", "day") == "day")
    swing_positions = sum(1 for p in positions if p.get("trade_type") == "swing")

    # Map range to Alpaca portfolio history params
    range_params = {
        "1d":  {"period": "1D", "timeframe": "15Min", "intraday_reporting": "market_hours"},
        "1w":  {"period": "1W", "timeframe": "1H"},
        "1m":  {"period": "1M", "timeframe": "1D"},
        "3m":  {"period": "3M", "timeframe": "1D"},
        "ytd": {"period": "1A", "timeframe": "1D"},
    }
    params = range_params.get(range, range_params["1d"])

    # Fetch portfolio history from Alpaca
    timestamps = []
    equity_series = []
    pnl_series = []
    base_value = float(equity) or 100000

    try:
        resp = httpx.get(
            f"{base_url}/v2/account/portfolio/history",
            headers=headers,
            params=params,
            timeout=10,
        )
        if resp.status_code == 200:
            data = resp.json()
            raw_ts = data.get("timestamp", [])
            raw_eq = data.get("equity", [])
            raw_pnl = data.get("profit_loss", [])
            base_value = data.get("base_value", base_value)

            from datetime import datetime, timezone

            for i, ts in enumerate(raw_ts):
                if raw_eq[i] is None or raw_eq[i] == 0:
                    continue
                dt = datetime.fromtimestamp(ts, tz=timezone.utc)
                if range == "1d":
                    timestamps.append(dt.strftime("%-I:%M %p"))
                elif range == "1w":
                    timestamps.append(dt.strftime("%b %-d %-I%p"))
                else:
                    timestamps.append(dt.strftime("%b %-d"))
                equity_series.append(float(raw_eq[i]))
                pnl_series.append(float(raw_pnl[i]) if raw_pnl[i] is not None else 0)
    except Exception:
        pass

    # If empty, fall back to just current equity
    if not equity_series and equity:
        timestamps.append("Now")
        equity_series.append(float(equity))
        pnl_series.append(0)

    start_equity = equity_series[0] if equity_series else base_value
    current_equity = float(equity) if equity else (equity_series[-1] if equity_series else base_value)
    period_pnl = current_equity - start_equity
    period_pnl_pct = (period_pnl / start_equity * 100) if start_equity else 0

    # Realized P&L from Alpaca profit_loss sum
    realized = sum(pnl_series) if pnl_series else 0

    # Win rate placeholder
    win_rate = 0.0

    return {
        "timestamps": timestamps,
        "equity": equity_series,
        "pnl": pnl_series,
        "summary": {
            "total_equity": round(current_equity, 2),
            "period_pnl": round(period_pnl, 2),
            "period_pnl_pct": round(period_pnl_pct, 2),
            "realized_pnl": round(float(realized), 2),
            "buying_power": round(float(buying_power), 2) if buying_power else 0,
            "cash": round(float(cash), 2) if cash else 0,
            "day_positions": day_positions,
            "swing_positions": swing_positions,
            "win_rate": round(win_rate, 4),
        },
    }


@app.get("/news/feed")
def get_news_feed(limit: int = 20):
    """Return aggregated news feed across all tracked symbols with thumbnails."""
    from strategies.news_fetcher import fetch_news as _fetch_news
    from strategies.sentiment import score_articles as _score_articles

    all_articles = []
    for symbol in SYMBOLS[:6]:  # Limit to core symbols to avoid slow response
        articles = _fetch_news(symbol, limit=5)
        scored = _score_articles(articles)
        for a in scored:
            a["symbol"] = symbol
        all_articles.extend(scored)

    # Sort by published_at descending, deduplicate by headline
    seen = set()
    unique = []
    for a in all_articles:
        if a["headline"] not in seen:
            seen.add(a["headline"])
            unique.append(a)

    unique.sort(key=lambda x: x.get("published_at") or "", reverse=True)
    return {"articles": unique[:limit]}
