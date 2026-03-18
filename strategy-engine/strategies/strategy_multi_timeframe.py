"""Multi-Timeframe Trend Alignment — swing trading strategy.

Confirms weekly trend direction via EMA slope, then enters on daily RSI pullbacks
with price confirmation above daily EMA.
"""

import numpy as np
import pandas as pd

from strategies.base import BaseStrategy, BacktestResult, Signal


class MultiTimeframeTrendAlignment(BaseStrategy):

    def __init__(
        self,
        weekly_ema_period: int = 20,
        daily_rsi_period: int = 14,
        pullback_rsi_low: float = 40.0,
        pullback_rsi_high: float = 55.0,
        daily_ema_period: int = 10,
        weekly_slope_weeks: int = 3,
    ):
        self.weekly_ema_period = weekly_ema_period
        self.daily_rsi_period = daily_rsi_period
        self.pullback_rsi_low = pullback_rsi_low
        self.pullback_rsi_high = pullback_rsi_high
        self.daily_ema_period = daily_ema_period
        self.weekly_slope_weeks = weekly_slope_weeks

    @property
    def name(self) -> str:
        return "MultiTimeframeTrend"

    def params(self) -> dict:
        return {
            "weekly_ema_period": self.weekly_ema_period,
            "daily_rsi_period": self.daily_rsi_period,
            "pullback_rsi_low": self.pullback_rsi_low,
            "pullback_rsi_high": self.pullback_rsi_high,
            "daily_ema_period": self.daily_ema_period,
            "weekly_slope_weeks": self.weekly_slope_weeks,
        }

    def _compute_rsi(self, series: pd.Series, period: int) -> pd.Series:
        delta = series.diff()
        gain = delta.where(delta > 0, 0.0)
        loss = -delta.where(delta < 0, 0.0)
        avg_gain = gain.ewm(com=period - 1, min_periods=period).mean()
        avg_loss = loss.ewm(com=period - 1, min_periods=period).mean()
        rs = avg_gain / avg_loss.replace(0, np.nan)
        return 100.0 - (100.0 / (1.0 + rs))

    def _resample_weekly(self, daily: pd.DataFrame) -> pd.DataFrame:
        """Resample daily OHLCV to weekly bars (week ending Friday)."""
        df = daily.set_index("timestamp")
        weekly = df.resample("W-FRI").agg({
            "open": "first",
            "high": "max",
            "low": "min",
            "close": "last",
            "volume": "sum",
        }).dropna()
        weekly = weekly.reset_index()
        return weekly

    def generate_signal(self, bars: pd.DataFrame, symbol: str) -> Signal:
        min_daily = self.weekly_ema_period * 5 + 20  # ~120 daily bars
        if len(bars) < min_daily:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason=f"Insufficient bars ({len(bars)}) for weekly EMA, need {min_daily}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
                trade_type="swing",
            )

        # Weekly timeframe analysis
        weekly = self._resample_weekly(bars)
        if len(weekly) < self.weekly_ema_period + self.weekly_slope_weeks:
            return Signal(
                symbol=symbol,
                direction="HOLD",
                confidence=0.0,
                reason=f"Insufficient weekly bars ({len(weekly)})",
                strategy_name=self.name,
                timestamp=self._now_iso(),
                trade_type="swing",
            )

        weekly["ema"] = weekly["close"].ewm(span=self.weekly_ema_period, adjust=False).mean()
        weekly["ema_slope"] = weekly["ema"].diff()

        # Check if weekly EMA slope has been positive for N consecutive weeks
        recent_slopes = weekly["ema_slope"].iloc[-self.weekly_slope_weeks:]
        weekly_bullish = (recent_slopes > 0).all()
        weekly_bearish = (recent_slopes < 0).all()

        # Weekly trend strength: average slope normalized by price
        weekly_price = weekly["close"].iloc[-1]
        avg_slope = recent_slopes.mean()
        weekly_strength = min(abs(avg_slope / weekly_price) * 100, 1.0) if weekly_price > 0 else 0.0

        # Daily timeframe analysis
        daily_rsi = self._compute_rsi(bars["close"], self.daily_rsi_period)
        current_rsi = daily_rsi.iloc[-1]
        daily_ema = bars["close"].ewm(span=self.daily_ema_period, adjust=False).mean()
        price_above_ema = bars["close"].iloc[-1] > daily_ema.iloc[-1]

        # Signal logic
        rsi_in_pullback = self.pullback_rsi_low <= current_rsi <= self.pullback_rsi_high
        rsi_in_oversold = current_rsi < self.pullback_rsi_low

        if weekly_bullish and rsi_in_pullback and price_above_ema:
            # Classic pullback entry in uptrend
            confidence = 0.5 + weekly_strength * 0.3 + (1.0 - current_rsi / 100) * 0.2
            confidence = min(confidence, 1.0)
            return Signal(
                symbol=symbol,
                direction="BUY",
                confidence=round(confidence, 4),
                reason=f"Weekly uptrend ({self.weekly_slope_weeks}w), RSI pullback={current_rsi:.1f}, price>EMA{self.daily_ema_period}",
                strategy_name=self.name,
                timestamp=self._now_iso(),
                trade_type="swing",
            )

        if weekly_bearish and current_rsi > 60 and not price_above_ema:
            # Bearish trend with RSI bouncing — potential short/exit
            confidence = 0.4 + weekly_strength * 0.3
            confidence = min(confidence, 1.0)
            return Signal(
                symbol=symbol,
                direction="SELL",
                confidence=round(confidence, 4),
                reason=f"Weekly downtrend ({self.weekly_slope_weeks}w), RSI={current_rsi:.1f} overbought in downtrend",
                strategy_name=self.name,
                timestamp=self._now_iso(),
                trade_type="swing",
            )

        # No alignment
        trend = "bullish" if weekly_bullish else ("bearish" if weekly_bearish else "neutral")
        return Signal(
            symbol=symbol,
            direction="HOLD",
            confidence=0.0,
            reason=f"No alignment: weekly={trend}, RSI={current_rsi:.1f}, price{'>' if price_above_ema else '<'}EMA",
            strategy_name=self.name,
            timestamp=self._now_iso(),
            trade_type="swing",
        )

    def backtest(self, bars: pd.DataFrame, symbol: str) -> BacktestResult:
        """Simple backtest: generate signals on rolling windows of daily bars."""
        if len(bars) < self.weekly_ema_period * 5 + 20:
            return BacktestResult(
                strategy_name=self.name, symbol=symbol,
                total_return_pct=0.0, sharpe_ratio=0.0, max_drawdown_pct=0.0,
                win_rate=0.0, total_trades=0, avg_trade_duration_mins=0.0,
                profit_factor=0.0, period_start=str(bars["timestamp"].iloc[0]),
                period_end=str(bars["timestamp"].iloc[-1]),
            )

        trades = []
        position = None
        min_window = self.weekly_ema_period * 5 + 20

        for i in range(min_window, len(bars)):
            window = bars.iloc[:i + 1].copy()
            sig = self.generate_signal(window, symbol)

            if sig.direction == "BUY" and position is None:
                position = {"entry": bars["close"].iloc[i], "entry_idx": i}
            elif sig.direction == "SELL" and position is not None:
                exit_price = bars["close"].iloc[i] * (1 - 0.0005)  # slippage
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
