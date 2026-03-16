"""RSIMeanReversion strategy — BUY when RSI oversold, SELL when RSI overbought."""

import numpy as np
import pandas as pd
import vectorbt as vbt

from strategies.base import BacktestResult, BaseStrategy, Signal


class RSIMeanReversion(BaseStrategy):
    def __init__(self, rsi_period: int = 14, oversold: float = 30.0, overbought: float = 70.0):
        self.rsi_period = rsi_period
        self.oversold = oversold
        self.overbought = overbought

    @property
    def name(self) -> str:
        return "RSIMeanReversion"

    def params(self) -> dict:
        return {
            "rsi_period": self.rsi_period,
            "oversold": self.oversold,
            "overbought": self.overbought,
        }

    @staticmethod
    def _compute_rsi(close: pd.Series, period: int) -> pd.Series:
        delta = close.diff()
        gain = delta.where(delta > 0, 0.0)
        loss = (-delta).where(delta < 0, 0.0)
        avg_gain = gain.rolling(period).mean()
        avg_loss = loss.rolling(period).mean()
        rs = avg_gain / avg_loss.replace(0, np.nan)
        rsi = 100.0 - (100.0 / (1.0 + rs))
        return rsi

    def generate_signal(self, bars: pd.DataFrame, symbol: str) -> Signal:
        min_bars = self.rsi_period + 2
        if len(bars) < min_bars:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason=f"Insufficient bars ({len(bars)}) for RSI period={self.rsi_period}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        close = bars["close"]
        rsi = self._compute_rsi(close, self.rsi_period)

        curr_rsi = rsi.iloc[-1]
        prev_rsi = rsi.iloc[-2]

        if np.isnan(curr_rsi):
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason="RSI is NaN — insufficient data",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        # BUY: RSI crossed below oversold threshold
        if curr_rsi < self.oversold and prev_rsi >= self.oversold:
            depth = (self.oversold - curr_rsi) / self.oversold
            confidence = min(0.6 + depth * 2, 1.0)
            return Signal(
                symbol=symbol,
                direction="BUY",
                confidence=round(confidence, 4),
                reason=f"RSI({self.rsi_period})={curr_rsi:.1f} crossed below oversold threshold {self.oversold}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        # SELL: RSI crossed above overbought threshold
        if curr_rsi > self.overbought and prev_rsi <= self.overbought:
            excess = (curr_rsi - self.overbought) / (100 - self.overbought)
            confidence = min(0.6 + excess * 2, 1.0)
            return Signal(
                symbol=symbol,
                direction="SELL",
                confidence=round(confidence, 4),
                reason=f"RSI({self.rsi_period})={curr_rsi:.1f} crossed above overbought threshold {self.overbought}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        return Signal(
            symbol=symbol,
            direction="HOLD",
            confidence=0.0,
            reason=f"No RSI crossover — RSI({self.rsi_period})={curr_rsi:.1f}",
            strategy_name=self.name,
            timestamp=self._now_iso(),
        )

    def backtest(self, bars: pd.DataFrame, symbol: str) -> BacktestResult:
        close = bars["close"]
        rsi = self._compute_rsi(close, self.rsi_period)

        entries = (rsi < self.oversold) & (rsi.shift(1) >= self.oversold)
        exits = (rsi > self.overbought) & (rsi.shift(1) <= self.overbought)

        pf = vbt.Portfolio.from_signals(
            close,
            entries=entries,
            exits=exits,
            init_cash=10_000,
            fees=0.0,
            slippage=0.0005,
            freq="1D",
        )

        stats = pf.stats()
        trades = pf.trades.records_readable if pf.trades.count() > 0 else pd.DataFrame()
        total_trades = int(pf.trades.count())

        if total_trades > 0 and not trades.empty:
            win_rate = float(stats.get("Win Rate [%]", 0.0)) / 100.0
            avg_duration_days = float(
                stats.get("Avg Winning Trade Duration", pd.Timedelta(0)).total_seconds() / 86400
            )
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
