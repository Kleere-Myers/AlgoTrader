"""MomentumVolume strategy — BUY on breakout above N-bar high with volume confirmation."""

import numpy as np
import pandas as pd
import vectorbt as vbt

from strategies.base import BacktestResult, BaseStrategy, Signal


class MomentumVolume(BaseStrategy):
    def __init__(self, lookback: int = 20, volume_multiplier: float = 1.5):
        self.lookback = lookback
        self.volume_multiplier = volume_multiplier

    @property
    def name(self) -> str:
        return "MomentumVolume"

    def params(self) -> dict:
        return {"lookback": self.lookback, "volume_multiplier": self.volume_multiplier}

    def generate_signal(self, bars: pd.DataFrame, symbol: str) -> Signal:
        min_bars = self.lookback + 2
        if len(bars) < min_bars:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason=f"Insufficient bars ({len(bars)}) for lookback={self.lookback}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        close = bars["close"]
        high = bars["high"]
        low = bars["low"]
        volume = bars["volume"].astype(float)

        # N-bar high/low (excluding current bar)
        rolling_high = high.shift(1).rolling(self.lookback).max()
        rolling_low = low.shift(1).rolling(self.lookback).min()

        # Volume condition: current volume > multiplier × 20-bar average
        avg_volume = volume.shift(1).rolling(self.lookback).mean()
        curr_volume = volume.iloc[-1]
        volume_confirmed = curr_volume > self.volume_multiplier * avg_volume.iloc[-1]

        curr_close = close.iloc[-1]
        prev_close = close.iloc[-2]
        curr_high_level = rolling_high.iloc[-1]
        curr_low_level = rolling_low.iloc[-1]

        if any(np.isnan(v) for v in [curr_high_level, curr_low_level, avg_volume.iloc[-1]]):
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason="Indicator values are NaN — insufficient data",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        # BUY: price breaks above N-bar high with volume confirmation
        if curr_close > curr_high_level and prev_close <= rolling_high.iloc[-2] and volume_confirmed:
            breakout_pct = (curr_close - curr_high_level) / curr_high_level
            vol_ratio = curr_volume / avg_volume.iloc[-1]
            confidence = min(0.5 + breakout_pct * 5 + (vol_ratio - self.volume_multiplier) * 0.1, 1.0)
            return Signal(
                symbol=symbol,
                direction="BUY",
                confidence=round(max(confidence, 0.5), 4),
                reason=(
                    f"Price {curr_close:.2f} broke above {self.lookback}-bar high {curr_high_level:.2f} "
                    f"with volume ratio {vol_ratio:.2f}x"
                ),
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        # SELL: price breaks below N-bar low with volume confirmation
        if curr_close < curr_low_level and prev_close >= rolling_low.iloc[-2] and volume_confirmed:
            breakdown_pct = (curr_low_level - curr_close) / curr_low_level
            vol_ratio = curr_volume / avg_volume.iloc[-1]
            confidence = min(0.5 + breakdown_pct * 5 + (vol_ratio - self.volume_multiplier) * 0.1, 1.0)
            return Signal(
                symbol=symbol,
                direction="SELL",
                confidence=round(max(confidence, 0.5), 4),
                reason=(
                    f"Price {curr_close:.2f} broke below {self.lookback}-bar low {curr_low_level:.2f} "
                    f"with volume ratio {vol_ratio:.2f}x"
                ),
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        return Signal(
            symbol=symbol,
            direction="HOLD",
            confidence=0.0,
            reason=f"No breakout — close={curr_close:.2f}, {self.lookback}-bar high={curr_high_level:.2f}, low={curr_low_level:.2f}",
            strategy_name=self.name,
            timestamp=self._now_iso(),
        )

    def backtest(self, bars: pd.DataFrame, symbol: str) -> BacktestResult:
        close = bars["close"]
        high = bars["high"]
        low = bars["low"]
        volume = bars["volume"].astype(float)

        rolling_high = high.shift(1).rolling(self.lookback).max()
        rolling_low = low.shift(1).rolling(self.lookback).min()
        avg_volume = volume.shift(1).rolling(self.lookback).mean()
        volume_confirmed = volume > self.volume_multiplier * avg_volume

        entries = (close > rolling_high) & (close.shift(1) <= rolling_high.shift(1)) & volume_confirmed
        exits = (close < rolling_low) & (close.shift(1) >= rolling_low.shift(1)) & volume_confirmed

        # Fill NaN with False for boolean indexing
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
