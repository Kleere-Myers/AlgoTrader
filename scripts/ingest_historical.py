"""
Ingest 2 years of daily OHLCV bars for all tracked instruments via yfinance into SQLite.

Usage:
  python scripts/ingest_historical.py [--db-path PATH]

Instruments: SPY, QQQ, AAPL, MSFT, NVDA, GOOGL
Bar size: 1d (daily)
Period: 2 years from today
"""

import argparse
import os
import sqlite3
import sys
from datetime import datetime, timedelta
from pathlib import Path

import yfinance as yf

DEFAULT_SYMBOLS = ["SPY", "QQQ", "AAPL", "MSFT", "NVDA", "GOOGL"]
SYMBOLS = [s.strip().upper() for s in os.environ.get("SYMBOLS", ",".join(DEFAULT_SYMBOLS)).split(",") if s.strip()]
BAR_SIZE = "1d"

DEFAULT_DB_PATH = os.environ.get(
    "DB_PATH",
    os.environ.get("DUCKDB_PATH",
                    str(Path(__file__).resolve().parent.parent / "data" / "algotrader.sqlite")),
)


def ingest(db_path: str) -> None:
    path = Path(db_path)
    if not path.exists():
        print(f"ERROR: database not found at {path}", file=sys.stderr)
        print("Run scripts/init_db.py first.", file=sys.stderr)
        sys.exit(1)

    end = datetime.now()
    start = end - timedelta(days=2 * 365)

    con = sqlite3.connect(str(path))
    try:
        total_rows = 0
        for symbol in SYMBOLS:
            print(f"Downloading {symbol}...", end=" ", flush=True)
            ticker = yf.Ticker(symbol)
            df = ticker.history(start=start.strftime("%Y-%m-%d"), end=end.strftime("%Y-%m-%d"), interval="1d")

            if df.empty:
                print("no data returned — skipping")
                continue

            # yfinance returns a DatetimeIndex with tz; normalize to naive UTC timestamps
            df.index = df.index.tz_localize(None) if df.index.tz is None else df.index.tz_convert("UTC").tz_localize(None)

            rows = []
            for ts, row in df.iterrows():
                rows.append((
                    symbol,
                    ts.isoformat(),
                    float(row["Open"]),
                    float(row["High"]),
                    float(row["Low"]),
                    float(row["Close"]),
                    int(row["Volume"]),
                    BAR_SIZE,
                ))

            # Upsert: delete existing rows for this symbol+bar_size then insert
            con.execute(
                "DELETE FROM ohlcv_bars WHERE symbol = ? AND bar_size = ?",
                [symbol, BAR_SIZE],
            )
            con.executemany(
                """INSERT INTO ohlcv_bars (symbol, timestamp, open, high, low, close, volume, bar_size)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
                rows,
            )
            con.commit()
            total_rows += len(rows)
            print(f"{len(rows)} bars")

        # Summary
        count = con.execute("SELECT COUNT(*) FROM ohlcv_bars WHERE bar_size = ?", [BAR_SIZE]).fetchone()[0]
        date_range = con.execute(
            "SELECT MIN(timestamp), MAX(timestamp) FROM ohlcv_bars WHERE bar_size = ?", [BAR_SIZE]
        ).fetchone()
        print(f"\nDone. {count} total daily bars in ohlcv_bars")
        print(f"Date range: {date_range[0]} — {date_range[1]}")
    finally:
        con.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Ingest historical OHLCV bars into SQLite")
    parser.add_argument("--db-path", default=DEFAULT_DB_PATH, help="Path to SQLite file")
    args = parser.parse_args()
    ingest(args.db_path)
