"""
Run MovingAverageCrossover backtest against all 6 instruments in DuckDB.

Usage:
  python scripts/run_backtest.py [--db-path PATH]
"""

import argparse
import os
import sys
from pathlib import Path

# Add strategy-engine to path so we can import strategies
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "strategy-engine"))

import duckdb
import pandas as pd

from strategies.strategy_moving_average import MovingAverageCrossover

SYMBOLS = ["SPY", "QQQ", "AAPL", "MSFT", "NVDA", "GOOGL"]

DEFAULT_DB_PATH = os.environ.get(
    "DUCKDB_PATH",
    str(Path(__file__).resolve().parent.parent / "data" / "algotrader.duckdb"),
)


def main(db_path: str) -> None:
    con = duckdb.connect(db_path, read_only=True)
    strategy = MovingAverageCrossover(fast_period=10, slow_period=30)

    print(f"Strategy: {strategy.name}")
    print(f"Params:   {strategy.params()}")
    print(f"{'Symbol':<8} {'Return%':>9} {'Sharpe':>8} {'MaxDD%':>8} {'WinRate':>8} {'Trades':>7} {'PF':>8}")
    print("-" * 62)

    results = []
    for symbol in SYMBOLS:
        bars = con.execute(
            "SELECT symbol, timestamp, open, high, low, close, volume "
            "FROM ohlcv_bars WHERE symbol = ? AND bar_size = '1d' ORDER BY timestamp",
            [symbol],
        ).fetchdf()

        if bars.empty:
            print(f"{symbol:<8} — no data")
            continue

        result = strategy.backtest(bars, symbol)
        results.append(result)

        pf = "inf" if result.profit_factor >= 999 else f"{result.profit_factor:.2f}"
        print(
            f"{symbol:<8} {result.total_return_pct:>8.2f}% "
            f"{result.sharpe_ratio:>8.2f} {result.max_drawdown_pct:>7.2f}% "
            f"{result.win_rate:>7.1%} {result.total_trades:>7} {pf:>8}"
        )

    con.close()

    if results:
        avg_return = sum(r.total_return_pct for r in results) / len(results)
        avg_sharpe = sum(r.sharpe_ratio for r in results) / len(results)
        total_trades = sum(r.total_trades for r in results)
        print("-" * 62)
        print(f"{'AVG':<8} {avg_return:>8.2f}% {avg_sharpe:>8.2f} {'':>8} {'':>8} {total_trades:>7}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Run MovingAverageCrossover backtest")
    parser.add_argument("--db-path", default=DEFAULT_DB_PATH, help="Path to DuckDB file")
    args = parser.parse_args()
    main(args.db_path)
