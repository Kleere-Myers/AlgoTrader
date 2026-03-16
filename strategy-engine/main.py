"""FastAPI app entrypoint — Strategy Engine."""

import os
from pathlib import Path
from typing import Any

import duckdb
import pandas as pd
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

from strategies.strategy_moving_average import MovingAverageCrossover
from strategies.strategy_rsi import RSIMeanReversion

app = FastAPI(title="AlgoTrader Strategy Engine", version="0.1.0")

DB_PATH = os.environ.get(
    "DUCKDB_PATH",
    str(Path(__file__).resolve().parent.parent / "data" / "algotrader.duckdb"),
)

# Strategy registry — add new strategies here
STRATEGIES = {
    "MovingAverageCrossover": MovingAverageCrossover(),
    "RSIMeanReversion": RSIMeanReversion(),
}

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
