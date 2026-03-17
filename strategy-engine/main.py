"""FastAPI app entrypoint — Strategy Engine."""

import logging
import os
from contextlib import asynccontextmanager
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

logger = logging.getLogger(__name__)

DB_PATH = os.environ.get(
    "DUCKDB_PATH",
    str(Path(__file__).resolve().parent.parent / "data" / "algotrader.duckdb"),
)

# Strategy registry — add new strategies here
_ml_strategy = MLSignalGenerator()

STRATEGIES = {
    "MovingAverageCrossover": MovingAverageCrossover(),
    "RSIMeanReversion": RSIMeanReversion(),
    "MomentumVolume": MomentumVolume(),
    "MLSignalGenerator": _ml_strategy,
}


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
    return [
        {
            "name": s.name,
            "enabled": True,
            "params": s.params(),
        }
        for s in STRATEGIES.values()
    ]


@app.post("/signal")
def generate_signals(req: SignalRequest):
    if not req.bars:
        raise HTTPException(status_code=400, detail="No bars provided")

    df = pd.DataFrame([b.model_dump() for b in req.bars])
    df["timestamp"] = pd.to_datetime(df["timestamp"])
    df = df.sort_values("timestamp").reset_index(drop=True)

    signals = []
    for strat in STRATEGIES.values():
        sig = strat.generate_signal(df, req.symbol.upper())
        signals.append(sig.to_dict())

    # Write signals to DuckDB for audit trail
    con = _get_db(read_only=False)
    try:
        for sig in signals:
            con.execute(
                "INSERT INTO signals (strategy_name, symbol, timestamp, direction, confidence, reason) "
                "VALUES (?, ?, ?, ?, ?, ?)",
                [sig["strategy_name"], sig["symbol"], sig["timestamp"],
                 sig["direction"], sig["confidence"], sig["reason"]],
            )
    finally:
        con.close()

    return {"signals": signals}


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
