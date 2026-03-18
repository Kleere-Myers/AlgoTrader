"""Relative Strength Ranking — swing trading strategy.

Scores symbols against SPY benchmark on a rolling return basis.
Only generates BUY signals for symbols in the top quartile of relative strength.
"""

import logging
import os
from pathlib import Path

import numpy as np
import pandas as pd

from strategies.base import BaseStrategy, BacktestResult, Signal

logger = logging.getLogger(__name__)

DB_PATH = os.environ.get(
    "DUCKDB_PATH",
    str(Path(__file__).resolve().parent.parent.parent / "data" / "algotrader.duckdb"),
)


class RelativeStrengthRanking(BaseStrategy):

    def __init__(
        self,
        lookback: int = 20,
        benchmark: str = "SPY",
        top_quartile: float = 0.25,
    ):
        self.lookback = lookback
        self.benchmark = benchmark
        self.top_quartile = top_quartile

    @property
    def name(self) -> str:
        return "RelativeStrength"

    def params(self) -> dict:
        return {
            "lookback": self.lookback,
            "benchmark": self.benchmark,
            "top_quartile": self.top_quartile,
        }

    def _fetch_benchmark_bars(self) -> pd.DataFrame | None:
        """Fetch benchmark (SPY) daily bars from DuckDB."""
        try:
            import duckdb
            con = duckdb.connect(DB_PATH, read_only=True)
            try:
                df = con.execute(
                    "SELECT timestamp, close FROM ohlcv_bars "
                    "WHERE symbol = ? AND bar_size = '1d' ORDER BY timestamp",
                    [self.benchmark],
                ).fetchdf()
            finally:
                con.close()
            if df.empty:
                return None
            df["timestamp"] = pd.to_datetime(df["timestamp"])
            return df
        except Exception as e:
            logger.warning("Failed to fetch benchmark bars: %s", e)
            return None

    def _fetch_all_symbol_returns(self) -> dict[str, float] | None:
        """Fetch rolling returns for all symbols to compute rankings."""
        try:
            import duckdb
            con = duckdb.connect(DB_PATH, read_only=True)
            try:
                df = con.execute(
                    "SELECT symbol, timestamp, close FROM ohlcv_bars "
                    "WHERE bar_size = '1d' ORDER BY symbol, timestamp",
                ).fetchdf()
            finally:
                con.close()
            if df.empty:
                return None

            returns = {}
            for sym, group in df.groupby("symbol"):
                if len(group) < self.lookback:
                    continue
                closes = group["close"].values
                ret = (closes[-1] - closes[-self.lookback]) / closes[-self.lookback]
                returns[sym] = ret
            return returns
        except Exception as e:
            logger.warning("Failed to fetch symbol returns: %s", e)
            return None

    def generate_signal(self, bars: pd.DataFrame, symbol: str) -> Signal:
        if len(bars) < self.lookback:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason=f"Insufficient bars ({len(bars)}) for lookback={self.lookback}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
                trade_type="swing",
            )

        # Get benchmark data
        benchmark_bars = self._fetch_benchmark_bars()
        if benchmark_bars is None or len(benchmark_bars) < self.lookback:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason=f"Insufficient benchmark ({self.benchmark}) data",
                strategy_name=self.name,
                timestamp=self._now_iso(),
                trade_type="swing",
            )

        # Compute symbol return over lookback period
        symbol_closes = bars["close"].values
        symbol_return = (symbol_closes[-1] - symbol_closes[-self.lookback]) / symbol_closes[-self.lookback]

        # Compute benchmark return
        bench_closes = benchmark_bars["close"].values
        bench_return = (bench_closes[-1] - bench_closes[-self.lookback]) / bench_closes[-self.lookback]

        # Relative strength ratio
        if bench_return == 0:
            rs_ratio = 1.0
        else:
            rs_ratio = symbol_return / bench_return if bench_return != 0 else 0.0

        # Get all symbol returns for ranking
        all_returns = self._fetch_all_symbol_returns()
        if all_returns is None or len(all_returns) < 4:
            # Not enough symbols to rank — use simple RS ratio
            if rs_ratio > 1.2:
                return Signal(
                    symbol=symbol, direction="BUY",
                    confidence=min(0.5 + (rs_ratio - 1.0) * 0.5, 1.0),
                    reason=f"RS ratio={rs_ratio:.2f} vs {self.benchmark} (insufficient peers for ranking)",
                    strategy_name=self.name, timestamp=self._now_iso(), trade_type="swing",
                )
            return Signal(
                symbol=symbol, direction="HOLD", confidence=0.0,
                reason=f"RS ratio={rs_ratio:.2f} vs {self.benchmark} (insufficient peers)",
                strategy_name=self.name, timestamp=self._now_iso(), trade_type="swing",
            )

        # Compute percentile rank
        all_rets = sorted(all_returns.values())
        symbol_ret = all_returns.get(symbol, symbol_return)
        rank = sum(1 for r in all_rets if r <= symbol_ret) / len(all_rets)

        if rank >= (1.0 - self.top_quartile):
            # Top quartile — strong relative strength
            confidence = 0.4 + rank * 0.5  # Scale: 0.4 to 0.9
            confidence = min(confidence, 1.0)
            return Signal(
                symbol=symbol,
                direction="BUY",
                confidence=round(confidence, 4),
                reason=f"RS percentile={rank:.0%} (top {self.top_quartile:.0%}), ratio={rs_ratio:.2f} vs {self.benchmark}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
                trade_type="swing",
            )

        if rank <= self.top_quartile:
            # Bottom quartile — weak relative strength
            confidence = 0.3 + (1.0 - rank) * 0.3
            confidence = min(confidence, 1.0)
            return Signal(
                symbol=symbol,
                direction="SELL",
                confidence=round(confidence, 4),
                reason=f"RS percentile={rank:.0%} (bottom {self.top_quartile:.0%}), ratio={rs_ratio:.2f} vs {self.benchmark}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
                trade_type="swing",
            )

        # Middle of the pack
        return Signal(
            symbol=symbol,
            direction="HOLD",
            confidence=0.0,
            reason=f"RS percentile={rank:.0%} (mid-range), ratio={rs_ratio:.2f} vs {self.benchmark}",
            strategy_name=self.name,
            timestamp=self._now_iso(),
            trade_type="swing",
        )

    def backtest(self, bars: pd.DataFrame, symbol: str) -> BacktestResult:
        """Backtest using relative strength against benchmark."""
        if len(bars) < self.lookback + 10:
            return BacktestResult(
                strategy_name=self.name, symbol=symbol,
                total_return_pct=0.0, sharpe_ratio=0.0, max_drawdown_pct=0.0,
                win_rate=0.0, total_trades=0, avg_trade_duration_mins=0.0,
                profit_factor=0.0, period_start=str(bars["timestamp"].iloc[0]),
                period_end=str(bars["timestamp"].iloc[-1]),
            )

        benchmark_bars = self._fetch_benchmark_bars()
        if benchmark_bars is None or len(benchmark_bars) < self.lookback:
            return BacktestResult(
                strategy_name=self.name, symbol=symbol,
                total_return_pct=0.0, sharpe_ratio=0.0, max_drawdown_pct=0.0,
                win_rate=0.0, total_trades=0, avg_trade_duration_mins=0.0,
                profit_factor=0.0, period_start=str(bars["timestamp"].iloc[0]),
                period_end=str(bars["timestamp"].iloc[-1]),
            )

        trades = []
        position = None

        for i in range(self.lookback, len(bars)):
            window = bars.iloc[:i + 1].copy()
            sig = self.generate_signal(window, symbol)

            if sig.direction == "BUY" and position is None:
                position = {"entry": bars["close"].iloc[i], "entry_idx": i}
            elif sig.direction != "BUY" and position is not None:
                exit_price = bars["close"].iloc[i] * (1 - 0.0005)
                entry_price = position["entry"] * (1 + 0.0005)
                pnl_pct = (exit_price - entry_price) / entry_price
                trades.append(pnl_pct)
                position = None

        if not trades:
            return BacktestResult(
                strategy_name=self.name, symbol=symbol,
                total_return_pct=0.0, sharpe_ratio=0.0, max_drawdown_pct=0.0,
                win_rate=0.0, total_trades=0, avg_trade_duration_mins=0.0,
                profit_factor=0.0, period_start=str(bars["timestamp"].iloc[0]),
                period_end=str(bars["timestamp"].iloc[-1]),
            )

        wins = [t for t in trades if t > 0]
        losses = [t for t in trades if t <= 0]
        total_return = sum(trades) * 100
        win_rate = len(wins) / len(trades) if trades else 0
        profit_factor = (sum(wins) / abs(sum(losses))) if losses and sum(losses) != 0 else 0.0
        sharpe = (np.mean(trades) / np.std(trades) * np.sqrt(252)) if np.std(trades) > 0 else 0.0

        return BacktestResult(
            strategy_name=self.name, symbol=symbol,
            total_return_pct=round(total_return, 2),
            sharpe_ratio=round(float(sharpe), 2),
            max_drawdown_pct=round(min(trades) * 100, 2) if trades else 0.0,
            win_rate=round(win_rate, 4),
            total_trades=len(trades),
            avg_trade_duration_mins=0.0,
            profit_factor=round(profit_factor, 2),
            period_start=str(bars["timestamp"].iloc[0]),
            period_end=str(bars["timestamp"].iloc[-1]),
        )
