"""
Initialize the DuckDB schema for AlgoTrader.

Creates all 6 tables defined in PRD Section 2.3:
  ohlcv_bars, signals, orders, positions, daily_pnl, strategy_config

Usage:
  python scripts/init_db.py [--db-path PATH]

Default db path: data/algotrader.duckdb
"""

import argparse
import os
import sys
from pathlib import Path

import duckdb


DEFAULT_DB_PATH = os.environ.get(
    "DUCKDB_PATH",
    str(Path(__file__).resolve().parent.parent / "data" / "algotrader.duckdb"),
)

SCHEMA_SQL = """
-- Historical and live price bars
CREATE TABLE IF NOT EXISTS ohlcv_bars (
    symbol      VARCHAR NOT NULL,
    timestamp   TIMESTAMP NOT NULL,
    open        DOUBLE NOT NULL,
    high        DOUBLE NOT NULL,
    low         DOUBLE NOT NULL,
    close       DOUBLE NOT NULL,
    volume      BIGINT NOT NULL,
    bar_size    VARCHAR NOT NULL DEFAULT '5min',
    PRIMARY KEY (symbol, timestamp, bar_size)
);

-- Strategy signals log
CREATE TABLE IF NOT EXISTS signals (
    id              INTEGER PRIMARY KEY DEFAULT nextval('signals_id_seq'),
    strategy_name   VARCHAR NOT NULL,
    symbol          VARCHAR NOT NULL,
    timestamp       TIMESTAMP NOT NULL,
    direction       VARCHAR NOT NULL CHECK (direction IN ('BUY', 'SELL', 'HOLD')),
    confidence      DOUBLE NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
    reason          VARCHAR,
    metadata        JSON
);

-- Order history
CREATE TABLE IF NOT EXISTS orders (
    id              INTEGER PRIMARY KEY DEFAULT nextval('orders_id_seq'),
    order_id        VARCHAR NOT NULL UNIQUE,
    alpaca_id       VARCHAR,
    symbol          VARCHAR NOT NULL,
    side            VARCHAR NOT NULL CHECK (side IN ('buy', 'sell')),
    qty             DOUBLE NOT NULL,
    order_type      VARCHAR NOT NULL DEFAULT 'market',
    limit_price     DOUBLE,
    filled_price    DOUBLE,
    status          VARCHAR NOT NULL DEFAULT 'pending',
    strategy_name   VARCHAR,
    created_at      TIMESTAMP NOT NULL DEFAULT current_timestamp,
    filled_at       TIMESTAMP
);

-- Current open positions
CREATE TABLE IF NOT EXISTS positions (
    symbol          VARCHAR PRIMARY KEY,
    qty             DOUBLE NOT NULL,
    avg_entry_price DOUBLE NOT NULL,
    current_price   DOUBLE,
    unrealized_pnl  DOUBLE,
    updated_at      TIMESTAMP NOT NULL DEFAULT current_timestamp
);

-- Daily P&L summary
CREATE TABLE IF NOT EXISTS daily_pnl (
    date            DATE PRIMARY KEY,
    realized_pnl    DOUBLE NOT NULL DEFAULT 0.0,
    unrealized_pnl  DOUBLE NOT NULL DEFAULT 0.0,
    total_trades    INTEGER NOT NULL DEFAULT 0,
    win_rate        DOUBLE,
    account_equity  DOUBLE
);

-- Active strategy settings
CREATE TABLE IF NOT EXISTS strategy_config (
    id              INTEGER PRIMARY KEY DEFAULT nextval('strategy_config_id_seq'),
    strategy_name   VARCHAR NOT NULL UNIQUE,
    params          JSON,
    enabled         BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at      TIMESTAMP NOT NULL DEFAULT current_timestamp
);
"""


def init_db(db_path: str) -> None:
    """Create the DuckDB file and initialize all tables."""
    path = Path(db_path)
    path.parent.mkdir(parents=True, exist_ok=True)

    con = duckdb.connect(str(path))
    try:
        # Create sequences for auto-increment IDs
        con.execute("CREATE SEQUENCE IF NOT EXISTS signals_id_seq START 1;")
        con.execute("CREATE SEQUENCE IF NOT EXISTS orders_id_seq START 1;")
        con.execute("CREATE SEQUENCE IF NOT EXISTS strategy_config_id_seq START 1;")

        con.execute(SCHEMA_SQL)

        # Verify all tables were created
        tables = con.execute(
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'main' ORDER BY table_name"
        ).fetchall()
        table_names = [t[0] for t in tables]

        expected = ["daily_pnl", "ohlcv_bars", "orders", "positions", "signals", "strategy_config"]
        for name in expected:
            if name not in table_names:
                print(f"ERROR: table '{name}' was not created", file=sys.stderr)
                sys.exit(1)

        print(f"DuckDB initialized at {path}")
        print(f"Tables: {', '.join(table_names)}")
    finally:
        con.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Initialize AlgoTrader DuckDB schema")
    parser.add_argument("--db-path", default=DEFAULT_DB_PATH, help="Path to DuckDB file")
    args = parser.parse_args()
    init_db(args.db_path)
