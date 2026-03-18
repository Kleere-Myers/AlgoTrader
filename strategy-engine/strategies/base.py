"""BaseStrategy interface, Signal, and BacktestResult — all strategies extend this."""

from abc import ABC, abstractmethod
from dataclasses import dataclass, asdict
from datetime import datetime, timezone
from typing import Literal

import pandas as pd


@dataclass
class Signal:
    symbol: str
    direction: Literal["BUY", "SELL", "HOLD"]
    confidence: float  # 0.0 to 1.0
    reason: str
    strategy_name: str
    timestamp: str  # ISO 8601 UTC
    trade_type: Literal["day", "swing"] = "day"

    def to_dict(self) -> dict:
        return asdict(self)


@dataclass
class BacktestResult:
    strategy_name: str
    symbol: str
    total_return_pct: float
    sharpe_ratio: float
    max_drawdown_pct: float
    win_rate: float
    total_trades: int
    avg_trade_duration_mins: float
    profit_factor: float
    period_start: str
    period_end: str

    def to_dict(self) -> dict:
        return asdict(self)


class BaseStrategy(ABC):
    @property
    @abstractmethod
    def name(self) -> str:
        """Unique strategy identifier used in DB and API responses."""
        ...

    @abstractmethod
    def generate_signal(self, bars: pd.DataFrame, symbol: str) -> Signal:
        """
        Accept OHLCV DataFrame with columns:
        [timestamp, open, high, low, close, volume]
        sorted ascending by timestamp.
        Return a Signal dataclass.
        """
        ...

    @abstractmethod
    def params(self) -> dict:
        """Return current tunable parameters as a dict."""
        ...

    @abstractmethod
    def backtest(self, bars: pd.DataFrame, symbol: str) -> BacktestResult:
        """
        Run backtest on historical OHLCV bars.
        Slippage: 0.05% per fill. Commission: $0.
        """
        ...

    def _now_iso(self) -> str:
        return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
