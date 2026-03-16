"""MovingAverageCrossover strategy — BUY on fast SMA > slow SMA, SELL on reverse."""

import numpy as np
import pandas as pd
import vectorbt as vbt

from strategies.base import BacktestResult, BaseStrategy, Signal


class MovingAverageCrossover(BaseStrategy):
    def __init__(self, fast_period: int = 10, slow_period: int = 30):
        self.fast_period = fast_period
        self.slow_period = slow_period

    @property
    def name(self) -> str:
        return "MovingAverageCrossover"

    def params(self) -> dict:
        return {"fast_period": self.fast_period, "slow_period": self.slow_period}

    def generate_signal(self, bars: pd.DataFrame, symbol: str) -> Signal:
        if len(bars) < self.slow_period + 1:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason=f"Insufficient bars ({len(bars)}) for slow_period={self.slow_period}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        close = bars["close"]
        fast_sma = close.rolling(self.fast_period).mean()
        slow_sma = close.rolling(self.slow_period).mean()

        curr_fast = fast_sma.iloc[-1]
        curr_slow = slow_sma.iloc[-1]
        prev_fast = fast_sma.iloc[-2]
        prev_slow = slow_sma.iloc[-2]

        # Crossover: fast was below slow, now above
        if prev_fast <= prev_slow and curr_fast > curr_slow:
            spread = (curr_fast - curr_slow) / curr_slow
            confidence = min(0.5 + spread * 10, 1.0)
            return Signal(
                symbol=symbol,
                direction="BUY",
                confidence=round(confidence, 4),
                reason=f"Fast SMA({self.fast_period})={curr_fast:.2f} crossed above Slow SMA({self.slow_period})={curr_slow:.2f}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        # Crossunder: fast was above slow, now below
        if prev_fast >= prev_slow and curr_fast < curr_slow:
            spread = (curr_slow - curr_fast) / curr_slow
            confidence = min(0.5 + spread * 10, 1.0)
            return Signal(
                symbol=symbol,
                direction="SELL",
                confidence=round(confidence, 4),
                reason=f"Fast SMA({self.fast_period})={curr_fast:.2f} crossed below Slow SMA({self.slow_period})={curr_slow:.2f}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        # No crossover
        return Signal(
            symbol=symbol,
            direction="HOLD",
            confidence=0.0,
            reason=f"No crossover — Fast SMA({self.fast_period})={curr_fast:.2f}, Slow SMA({self.slow_period})={curr_slow:.2f}",
            strategy_name=self.name,
            timestamp=self._now_iso(),
        )

    def backtest(self, bars: pd.DataFrame, symbol: str) -> BacktestResult:
        close = bars["close"]

        fast_sma = close.rolling(self.fast_period).mean()
        slow_sma = close.rolling(self.slow_period).mean()

        # Entry when fast crosses above slow; exit when fast crosses below slow
        entries = (fast_sma > slow_sma) & (fast_sma.shift(1) <= slow_sma.shift(1))
        exits = (fast_sma < slow_sma) & (fast_sma.shift(1) >= slow_sma.shift(1))

        pf = vbt.Portfolio.from_signals(
            close,
            entries=entries,
            exits=exits,
            init_cash=10_000,
            fees=0.0,       # commission-free (Alpaca)
            slippage=0.0005, # 0.05% per fill
            freq="1D",
        )

        stats = pf.stats()
        trades = pf.trades.records_readable if pf.trades.count() > 0 else pd.DataFrame()

        total_trades = int(pf.trades.count())

        if total_trades > 0 and not trades.empty:
            win_rate = float(stats.get("Win Rate [%]", 0.0)) / 100.0
            # Average trade duration in minutes (daily bars → convert days to minutes)
            avg_duration_days = float(stats.get("Avg Winning Trade Duration", pd.Timedelta(0)).total_seconds() / 86400) if total_trades > 0 else 0.0
            avg_duration_mins = avg_duration_days * 24 * 60

            winning_pnl = trades.loc[trades["PnL"] > 0, "PnL"].sum() if "PnL" in trades.columns else 0.0
            losing_pnl = abs(trades.loc[trades["PnL"] < 0, "PnL"].sum()) if "PnL" in trades.columns else 0.0
            profit_factor = float(winning_pnl / losing_pnl) if losing_pnl > 0 else float("inf")
        else:
            win_rate = 0.0
            avg_duration_mins = 0.0
            profit_factor = 0.0

        period_start = str(bars["timestamp"].iloc[0])
        period_end = str(bars["timestamp"].iloc[-1])

        return BacktestResult(
            strategy_name=self.name,
            symbol=symbol,
            total_return_pct=round(float(stats.get("Total Return [%]", 0.0)), 4),
            sharpe_ratio=round(float(stats.get("Sharpe Ratio", 0.0)), 4),
            max_drawdown_pct=round(float(stats.get("Max Drawdown [%]", 0.0)), 4),
            win_rate=round(win_rate, 4),
            total_trades=total_trades,
            avg_trade_duration_mins=round(avg_duration_mins, 2),
            profit_factor=round(profit_factor, 4) if profit_factor != float("inf") else 999.0,
            period_start=period_start,
            period_end=period_end,
        )
