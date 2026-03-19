# AlgoTrader Personal

A self-hosted automated trading system for day trading US equities and ETFs. Built with Python, Rust, and Next.js. Trades through [Alpaca Markets](https://alpaca.markets/) paper or live accounts.

**For personal use only. Not licensed for distribution.**

## Architecture

Three independently running services communicate over localhost HTTP:

| Service | Stack | Port | Responsibility |
|---|---|---|---|
| **Strategy Engine** | Python 3.12 + FastAPI | 8000 | Signal generation, backtesting, ML models |
| **Execution Engine** | Rust + Axum | 8080 | Order routing, risk enforcement, WebSocket feeds, position tracking |
| **Dashboard** | Next.js 14 + Tailwind | 3000 | Real-time monitoring, strategy config, P&L visualization |

```
Alpaca WebSocket ──> Execution Engine ──> Strategy Engine
                          │                     │
                          │ <── signals ─────────┘
                          │
                          ├──> Risk Checks ──> Alpaca Orders
                          ├──> DuckDB (shared)
                          └──> SSE Stream ──> Dashboard
```

## Strategies

Four trading strategies run on every incoming bar, all extending `BaseStrategy`:

| Strategy | Logic | Default Params |
|---|---|---|
| **MovingAverageCrossover** | BUY on fast SMA crossing above slow SMA | fast=10, slow=30 |
| **RSIMeanReversion** | BUY when RSI crosses below oversold threshold | period=14, oversold=30, overbought=70 |
| **MomentumVolume** | BUY on price breakout above 20-bar high with volume confirmation | lookback=20, vol_mult=1.5 |
| **MLSignalGenerator** | LightGBM classifier trained on technical features | min_confidence=0.65, retrain=weekly |

## Risk Management

Enforced in Rust before any order reaches Alpaca. Non-negotiable.

| Rule | Default | On Breach |
|---|---|---|
| Max daily loss | 2% of equity | Halt all trading |
| Max position size | 10% of equity per symbol | Reject signal |
| Max open positions | 4 | Reject signal |
| Min signal confidence | 0.60 | Reject signal |
| Order throttle | 5 min per symbol | Throttle |
| EOD flatten | 3:45 PM ET | Market-sell all positions |

## Instrument Universe

SPY, QQQ, AAPL, MSFT, NVDA, GOOGL — regular session only (9:30 AM - 4:00 PM ET).

## Quick Start

### Prerequisites

- Python 3.12+
- Rust (stable toolchain)
- Node.js 18+
- [Alpaca Markets](https://alpaca.markets/) account (paper trading)

### Setup

```bash
# Clone and enter project
git clone https://github.com/Kleere-Myers/AlgoTrader.git
cd AlgoTrader

# Create .env from template
cp .env.example .env
# Edit .env with your Alpaca API keys

# Python setup
python3 -m venv .venv
.venv/bin/pip install -r strategy-engine/requirements.txt

# Initialize database and ingest historical data
.venv/bin/python scripts/init_db.py
.venv/bin/python scripts/ingest_historical.py

# Rust build
cd execution-engine && cargo build && cd ..

# Dashboard setup
cd dashboard && npm install && cd ..
```

### Verify Alpaca Connection

```bash
cd execution-engine && cargo run -- --check-auth
```

### Start All Services

```bash
# Terminal 1 — Strategy Engine
cd strategy-engine && ../.venv/bin/uvicorn main:app --port 8000

# Terminal 2 — Execution Engine
cd execution-engine && RUST_LOG=info cargo run

# Terminal 3 — Dashboard
cd dashboard && npm run dev
```

Open http://localhost:9102 to view the dashboard.

### Run Tests

```bash
# Rust (26 tests — risk rules, EOD scheduler, risk config API)
cd execution-engine && cargo test

# Python (72 tests — 4 strategies, ML features, performance endpoint)
cd strategy-engine && ../.venv/bin/python -m pytest tests/ -v

# Dashboard (build check — all 7 routes)
cd dashboard && npx next build
```

## Project Structure

```
algotrader/
  strategy-engine/           # Python FastAPI service
    strategies/              # One file per strategy plugin
    ml/                      # ML feature pipeline and training
    models/                  # Trained model artifacts (.pkl, gitignored)
    tests/                   # pytest test suite
    main.py                  # FastAPI app entrypoint
  execution-engine/          # Rust Axum service
    src/
      main.rs                # Axum router, WebSocket ingestion, signal processing
      alpaca.rs              # Alpaca REST + WebSocket client
      risk.rs                # Risk rule enforcement (8 checks)
      orders.rs              # Order submission to Alpaca
      positions.rs           # In-memory position tracker
      db.rs                  # DuckDB access layer
      sse.rs                 # Server-Sent Events broadcaster
      scheduler.rs           # EOD auto-flatten logic
    Cargo.toml
  dashboard/                 # Next.js 14 app
    app/                     # App Router pages (7 routes)
    components/              # StrategyCard, CandlestickChart, etc.
    hooks/                   # useSseEvents SSE hook
    lib/                     # Typed API client
    types/                   # Shared TypeScript types
  data/                      # DuckDB database file (gitignored)
  scripts/                   # DB init, data ingestion, backtest runner
  .env                       # API keys (gitignored)
```

## Database

DuckDB with 6 tables: `ohlcv_bars`, `signals`, `orders`, `positions`, `daily_pnl`, `strategy_config`. Schema defined in `scripts/init_db.py`.

## Dashboard Pages

| Route | Page | Description |
|---|---|---|
| `/` | Overview | Equity stats, candlestick chart with signal overlays, open positions, SSE event feed |
| `/positions` | Positions | Open positions with real-time P&L (green/red) via SSE |
| `/orders` | Orders | Order history with status badges, strategy attribution |
| `/strategies` | Strategies | Enable/disable, edit params, trigger backtests |
| `/backtest` | Backtest | Equity curves (Recharts), metrics table, strategy/symbol filters |
| `/risk` | Risk Settings | Editable thresholds, emergency halt button |
| `/logs` | Logs | Color-coded SSE event stream with auto-scroll |

## Important Notes

- `ALPACA_MODE=paper` is the default. **Never switch to `live` without explicit intention.**
- Risk rules in `risk.rs` are the last line of defense. Never make them more permissive without review.
- API keys come from environment variables only. Never hardcode.
- The ML model is currently trained on daily bars (59.4% accuracy baseline). It will improve as intraday data accumulates from the live WebSocket feed.
