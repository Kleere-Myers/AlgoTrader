"""VWAPStrategy — BUY when price crosses below VWAP, SELL when above."""

import numpy as np
import pandas as pd
import vectorbt as vbt

from strategies.base import BacktestResult, BaseStrategy, Signal


class VWAPStrategy(BaseStrategy):
    def __init__(self, distance_threshold: float = 0.005):
        self.distance_threshold = distance_threshold

    @property
    def name(self) -> str:
        return "VWAPStrategy"

    def params(self) -> dict:
        return {"distance_threshold": self.distance_threshold}

    @staticmethod
    def _compute_vwap(bars: pd.DataFrame) -> pd.Series:
        """VWAP = cumsum(typical_price * volume) / cumsum(volume)."""
        typical_price = (bars["high"] + bars["low"] + bars["close"]) / 3.0
        vol = bars["volume"].astype(float)
        cumulative_tp_vol = (typical_price * vol).cumsum()
        cumulative_vol = vol.cumsum()
        vwap = cumulative_tp_vol / cumulative_vol.replace(0, np.nan)
        return vwap

    def generate_signal(self, bars: pd.DataFrame, symbol: str) -> Signal:
        min_bars = 10
        if len(bars) < min_bars:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason=f"Insufficient bars ({len(bars)}) for VWAP calculation",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        vwap = self._compute_vwap(bars)
        curr_close = bars["close"].iloc[-1]
        prev_close = bars["close"].iloc[-2]
        curr_vwap = vwap.iloc[-1]
        prev_vwap = vwap.iloc[-2]

        if np.isnan(curr_vwap) or np.isnan(prev_vwap):
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason="VWAP is NaN — insufficient volume data",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        distance = (curr_close - curr_vwap) / curr_vwap

        # BUY: price crosses below VWAP (cheap vs average) beyond threshold
        if curr_close < curr_vwap and prev_close >= prev_vwap and abs(distance) >= self.distance_threshold:
            confidence = min(0.5 + abs(distance) * 20, 1.0)
            return Signal(
                symbol=symbol,
                direction="BUY",
                confidence=round(confidence, 4),
                reason=f"Price {curr_close:.2f} crossed below VWAP {curr_vwap:.2f} (distance {distance:.4f})",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        # SELL: price crosses above VWAP (expensive vs average) beyond threshold
        if curr_close > curr_vwap and prev_close <= prev_vwap and distance >= self.distance_threshold:
            confidence = min(0.5 + distance * 20, 1.0)
            return Signal(
                symbol=symbol,
                direction="SELL",
                confidence=round(confidence, 4),
                reason=f"Price {curr_close:.2f} crossed above VWAP {curr_vwap:.2f} (distance {distance:.4f})",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        return Signal(
            symbol=symbol,
            direction="HOLD",
            confidence=0.0,
            reason=f"No VWAP crossover — close={curr_close:.2f}, VWAP={curr_vwap:.2f}, distance={distance:.4f}",
            strategy_name=self.name,
            timestamp=self._now_iso(),
        )

    def backtest(self, bars: pd.DataFrame, symbol: str) -> BacktestResult:
        close = bars["close"]
        vwap = self._compute_vwap(bars)

        # Entry when price crosses below VWAP; exit when price crosses above VWAP
        entries = (close < vwap) & (close.shift(1) >= vwap.shift(1))
        exits = (close > vwap) & (close.shift(1) <= vwap.shift(1))

        entries = entries.fillna(False)
        exits = exits.fillna(False)

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

        def _safe(val, default=0.0):
            v = float(val)
            return default if np.isnan(v) or np.isinf(v) else v

        return BacktestResult(
            strategy_name=self.name,
            symbol=symbol,
            total_return_pct=round(_safe(stats.get("Total Return [%]", 0.0)), 4),
            sharpe_ratio=round(_safe(stats.get("Sharpe Ratio", 0.0)), 4),
            max_drawdown_pct=round(_safe(stats.get("Max Drawdown [%]", 0.0)), 4),
            win_rate=round(win_rate, 4),
            total_trades=total_trades,
            avg_trade_duration_mins=round(avg_duration_mins, 2),
            profit_factor=round(profit_factor, 4) if profit_factor != float("inf") else 999.0,
            period_start=period_start,
            period_end=period_end,
        )
