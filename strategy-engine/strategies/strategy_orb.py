"""OpeningRangeBreakout strategy — BUY/SELL on breakout from N-bar opening range."""

import numpy as np
import pandas as pd
import vectorbt as vbt

from strategies.base import BacktestResult, BaseStrategy, Signal


class OpeningRangeBreakout(BaseStrategy):
    def __init__(self, opening_bars: int = 6, volume_multiplier: float = 1.2):
        self.opening_bars = opening_bars
        self.volume_multiplier = volume_multiplier

    @property
    def name(self) -> str:
        return "OpeningRangeBreakout"

    def params(self) -> dict:
        return {"opening_bars": self.opening_bars, "volume_multiplier": self.volume_multiplier}

    def generate_signal(self, bars: pd.DataFrame, symbol: str) -> Signal:
        min_bars = self.opening_bars + 2
        if len(bars) < min_bars:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason=f"Insufficient bars ({len(bars)}) for opening range of {self.opening_bars} bars",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        # Opening range = high/low of first N bars (or last N bars as lookback window)
        opening_range = bars.iloc[-min_bars:-2]  # N bars before the current and previous bar
        range_high = opening_range["high"].max()
        range_low = opening_range["low"].min()

        if np.isnan(range_high) or np.isnan(range_low) or range_high == range_low:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason="Opening range is zero or NaN",
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        curr_close = bars["close"].iloc[-1]
        prev_close = bars["close"].iloc[-2]
        volume = bars["volume"].astype(float)
        avg_volume = volume.iloc[-min_bars:-1].mean()
        curr_volume = volume.iloc[-1]

        volume_confirmed = curr_volume > self.volume_multiplier * avg_volume if avg_volume > 0 else False

        range_size = range_high - range_low

        # BUY: close breaks above range high with volume confirmation
        if curr_close > range_high and prev_close <= range_high and volume_confirmed:
            breakout_magnitude = (curr_close - range_high) / range_size
            confidence = min(0.5 + breakout_magnitude * 2, 1.0)
            return Signal(
                symbol=symbol,
                direction="BUY",
                confidence=round(confidence, 4),
                reason=(
                    f"Price {curr_close:.2f} broke above opening range high {range_high:.2f} "
                    f"(range {range_low:.2f}-{range_high:.2f}, vol ratio {curr_volume/avg_volume:.2f}x)"
                ),
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        # SELL: close breaks below range low with volume confirmation
        if curr_close < range_low and prev_close >= range_low and volume_confirmed:
            breakdown_magnitude = (range_low - curr_close) / range_size
            confidence = min(0.5 + breakdown_magnitude * 2, 1.0)
            return Signal(
                symbol=symbol,
                direction="SELL",
                confidence=round(confidence, 4),
                reason=(
                    f"Price {curr_close:.2f} broke below opening range low {range_low:.2f} "
                    f"(range {range_low:.2f}-{range_high:.2f}, vol ratio {curr_volume/avg_volume:.2f}x)"
                ),
                strategy_name=self.name,
                timestamp=self._now_iso(),
            )

        return Signal(
            symbol=symbol,
            direction="HOLD",
            confidence=0.0,
            reason=(
                f"No breakout — close={curr_close:.2f}, range {range_low:.2f}-{range_high:.2f}"
            ),
            strategy_name=self.name,
            timestamp=self._now_iso(),
        )

    def backtest(self, bars: pd.DataFrame, symbol: str) -> BacktestResult:
        close = bars["close"]
        high = bars["high"]
        low = bars["low"]
        volume = bars["volume"].astype(float)

        # Rolling opening range: high/low of previous N bars
        rolling_range_high = high.shift(1).rolling(self.opening_bars).max()
        rolling_range_low = low.shift(1).rolling(self.opening_bars).min()
        avg_volume = volume.shift(1).rolling(self.opening_bars).mean()
        volume_confirmed = volume > self.volume_multiplier * avg_volume

        entries = (close > rolling_range_high) & (close.shift(1) <= rolling_range_high.shift(1)) & volume_confirmed
        exits = (close < rolling_range_low) & (close.shift(1) >= rolling_range_low.shift(1)) & volume_confirmed

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
