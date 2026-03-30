"""
Initialize the SQLite schema for AlgoTrader.

Creates all 7 tables:
  ohlcv_bars, signals, orders, positions, daily_pnl, strategy_config, watched_symbols

Usage:
  python scripts/init_db.py [--db-path PATH]

Default db path: data/algotrader.sqlite
"""

import argparse
import os
import sqlite3
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parent.parent

_env_path = os.environ.get("DB_PATH", os.environ.get("DUCKDB_PATH", ""))
if _env_path:
    p = Path(_env_path)
    DEFAULT_DB_PATH = str(
        p if p.is_absolute() else (PROJECT_ROOT / "strategy-engine" / p).resolve()
    )
else:
    DEFAULT_DB_PATH = str(PROJECT_ROOT / "data" / "algotrader.sqlite")

SCHEMA_SQL = """
-- Historical and live price bars
CREATE TABLE IF NOT EXISTS ohlcv_bars (
    symbol      TEXT NOT NULL,
    timestamp   TEXT NOT NULL,
    open        REAL NOT NULL,
    high        REAL NOT NULL,
    low         REAL NOT NULL,
    close       REAL NOT NULL,
    volume      INTEGER NOT NULL,
    bar_size    TEXT NOT NULL DEFAULT '5min',
    PRIMARY KEY (symbol, timestamp, bar_size)
);

-- Strategy signals log
CREATE TABLE IF NOT EXISTS signals (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    strategy_name   TEXT NOT NULL,
    symbol          TEXT NOT NULL,
    timestamp       TEXT NOT NULL,
    direction       TEXT NOT NULL CHECK (direction IN ('BUY', 'SELL', 'HOLD')),
    confidence      REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
    reason          TEXT,
    metadata        TEXT,
    trade_type      TEXT NOT NULL DEFAULT 'day' CHECK (trade_type IN ('day', 'swing'))
);

-- Order history
CREATE TABLE IF NOT EXISTS orders (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id        TEXT NOT NULL UNIQUE,
    alpaca_id       TEXT,
    symbol          TEXT NOT NULL,
    side            TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    qty             REAL NOT NULL,
    order_type      TEXT NOT NULL DEFAULT 'market',
    limit_price     REAL,
    filled_price    REAL,
    status          TEXT NOT NULL DEFAULT 'pending',
    strategy_name   TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    filled_at       TEXT,
    trade_type      TEXT NOT NULL DEFAULT 'day' CHECK (trade_type IN ('day', 'swing'))
);

-- Current open positions
CREATE TABLE IF NOT EXISTS positions (
    symbol          TEXT PRIMARY KEY,
    side            TEXT NOT NULL DEFAULT 'long' CHECK (side IN ('long', 'short')),
    qty             REAL NOT NULL,
    avg_entry_price REAL NOT NULL,
    current_price   REAL,
    unrealized_pnl  REAL,
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    trade_type      TEXT NOT NULL DEFAULT 'day' CHECK (trade_type IN ('day', 'swing')),
    stop_loss_price REAL,
    take_profit_price REAL
);

-- Daily P&L summary
CREATE TABLE IF NOT EXISTS daily_pnl (
    date            TEXT PRIMARY KEY,
    realized_pnl    REAL NOT NULL DEFAULT 0.0,
    unrealized_pnl  REAL NOT NULL DEFAULT 0.0,
    total_trades    INTEGER NOT NULL DEFAULT 0,
    win_rate        REAL,
    account_equity  REAL
);

-- Tracked symbols (persists across restarts)
CREATE TABLE IF NOT EXISTS watched_symbols (
    symbol      TEXT PRIMARY KEY,
    added_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
);

-- Active strategy settings
CREATE TABLE IF NOT EXISTS strategy_config (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    strategy_name   TEXT NOT NULL UNIQUE,
    params          TEXT,
    enabled         INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
);
"""


def init_db(db_path: str) -> None:
    """Create the SQLite file and initialize all tables."""
    path = Path(db_path)
    path.parent.mkdir(parents=True, exist_ok=True)

    con = sqlite3.connect(str(path))
    try:
        con.execute("PRAGMA journal_mode=WAL;")
        con.execute("PRAGMA busy_timeout=5000;")
        con.executescript(SCHEMA_SQL)

        # Verify all tables were created
        tables = con.execute(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
        ).fetchall()
        table_names = [t[0] for t in tables if not t[0].startswith("sqlite_")]

        expected = ["daily_pnl", "ohlcv_bars", "orders", "positions", "signals", "strategy_config", "watched_symbols"]
        for name in expected:
            if name not in table_names:
                print(f"ERROR: table '{name}' was not created", file=sys.stderr)
                sys.exit(1)

        print(f"SQLite initialized at {path}")
        print(f"Tables: {', '.join(table_names)}")
    finally:
        con.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Initialize AlgoTrader SQLite schema")
    parser.add_argument("--db-path", default=DEFAULT_DB_PATH, help="Path to SQLite file")
    args = parser.parse_args()
    init_db(args.db_path)
