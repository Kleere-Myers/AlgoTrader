# Agent Context: Strategy Engine
# AlgoTrader Personal — strategy-engine/ service

## Your Role
You are the Strategy Engine agent. You own everything inside `strategy-engine/`.
You do not modify files in `execution-engine/` or `dashboard/` unless explicitly
asked, and you flag any change that touches a shared contract before making it.

---

## Your Service at a Glance

- **Language:** Python 3.12
- **Framework:** FastAPI (async)
- **Port:** 8000
- **Database:** DuckDB via `duckdb` Python package
- **Scheduler:** APScheduler for market-hours jobs
- **Broker SDK:** alpaca-py (for market data fetching only — never submit orders)

---

## Key Dependencies

```
fastapi
uvicorn[standard]
alpaca-py
pandas
numpy
pandas-ta
vectorbt
scikit-learn
lightgbm
duckdb
apscheduler
python-dotenv
pytest
httpx          # for async test client
```

---

## BaseStrategy Interface — All Strategies Must Extend This

```python
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Literal
import pandas as pd

@dataclass
class Signal:
    symbol: str
    direction: Literal["BUY", "SELL", "HOLD"]
    confidence: float          # 0.0 to 1.0
    reason: str
    strategy_name: str
    timestamp: str             # ISO 8601 UTC

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

    def backtest(self, bars: pd.DataFrame) -> BacktestResult:
        """
        Default backtesting via vectorbt.
        Override for custom logic.
        Slippage: 0.05% per fill. Commission: $0.
        """
        ...
```

**Never skip the BaseStrategy contract.** One file per strategy in `strategies/`.
File naming: `strategy_moving_average.py`, `strategy_rsi.py`, etc.

---

## Implemented Strategies

### 1. MovingAverageCrossover
- BUY: fast SMA crosses above slow SMA
- SELL: fast SMA crosses below slow SMA
- Default params: `fast_period=10, slow_period=30, bar_size=5min`

### 2. RSIMeanReversion
- BUY: RSI drops below oversold threshold
- SELL: RSI rises above overbought threshold
- Default params: `rsi_period=14, oversold=30, overbought=70`

### 3. MomentumVolume
- BUY: price breaks above N-bar high AND volume > multiplier × 20-bar avg volume
- SELL: price breaks below N-bar low with same volume condition
- Default params: `lookback=20, volume_multiplier=1.5`

### 4. MLSignalGenerator
- Model: LightGBM classifier
- Output: BUY / SELL / HOLD with confidence score
- Retrain: weekly on latest 6 months of 5-min bars
- Min confidence to emit non-HOLD signal: 0.65
- Trained model artifacts stored in `models/` as `.pkl` files

#### ML Feature Set
```python
features = [
    'rsi_14', 'macd', 'macd_signal', 'macd_hist',
    'bb_pct_b',           # Bollinger Band %B
    'atr_14',
    'volume_ratio',       # current volume / 20-bar avg volume
    'roc_5', 'roc_10', 'roc_20',   # rate of change
    'rolling_return_5', 'rolling_return_20',
    'rolling_vol_20',     # rolling 20-bar price volatility
    'day_of_week',        # 0-4
    'hour_of_day',        # 9-15
]
# Label: 1 if price +0.3% in next 30min, -1 if -0.3%, 0 otherwise
```

---

## FastAPI Endpoints You Own

| Method | Path | Description |
|---|---|---|
| POST | /signal | Accept OHLCV bars payload, return signals from all enabled strategies |
| GET | /strategies | List all strategies with enabled status and params |
| PATCH | /strategies/{id} | Update strategy params or toggle enabled |
| POST | /backtest/{strategy}/{symbol} | Run backtest, store result, return summary |
| GET | /backtest/{strategy}/{symbol} | Get last stored backtest result |
| GET | /health | Returns `{"status": "ok"}` |

### POST /signal — Request/Response

```json
// Request body
{
  "symbol": "AAPL",
  "bars": [
    {"timestamp": "2026-03-16T14:25:00Z", "open": 172.1, "high": 172.8,
     "low": 171.9, "close": 172.5, "volume": 48200},
    ...
  ]
}

// Response body — one signal per enabled strategy
{
  "signals": [
    {
      "symbol": "AAPL",
      "direction": "BUY",
      "confidence": 0.72,
      "reason": "RSI(14) = 28.4, crossed below oversold threshold 30",
      "strategy_name": "RSIMeanReversion",
      "timestamp": "2026-03-16T14:30:00Z"
    }
  ]
}
```

---

## Database Access

```python
import duckdb
import os

def get_db():
    return duckdb.connect(os.getenv("DUCKDB_PATH", "../data/algotrader.duckdb"))
```

Tables you read and write:
- `ohlcv_bars` — read for backtesting and feature generation
- `signals` — write every signal emitted (including HOLDs for audit trail)
- `strategy_config` — read/write strategy params and enabled state
- `backtest_results` — write backtest summaries

Tables you read only:
- `positions` — read to avoid signaling on already-held symbols (optional logic)

Tables you never touch:
- `orders`, `daily_pnl` — owned by execution-engine

---

## Scheduling

Market data ingestion and ML retraining are scheduled jobs:

```python
# Run at 9:25 AM ET on trading days — fetch today's pre-market bars
# Run weekly Sunday 6 PM ET — retrain MLSignalGenerator
# Run every 5 minutes 9:30-4:00 ET — check for stale data and backfill
```

Use APScheduler with the AsyncIOScheduler. Register jobs in `main.py` on startup.
Use `pandas_market_calendars` or a simple NYSE holiday list to skip non-trading days.

---

## Testing Requirements

- Every strategy must have a `tests/test_strategy_[name].py` file
- Tests must cover: signal generation on known data, edge cases (flat market,
  insufficient bars), params validation
- Use `pytest` with `pytest-asyncio` for async endpoint tests
- Mock Alpaca API calls in tests — never hit real Alpaca in test suite
- Run tests: `pytest strategy-engine/tests/ -v`

---

## What to Flag Before Doing

- Any change to the Signal dataclass fields or types
- Any change to the POST /signal request or response schema
- Any new dependency that requires a C extension (can complicate deployment)
- Any change to DuckDB table schemas in `scripts/init_db.py`
