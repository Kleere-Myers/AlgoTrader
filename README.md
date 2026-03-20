# AlgoTrader

A self-hosted automated trading system for day trading and swing trading US equities and ETFs. Three-service architecture built with Python, Rust, and TypeScript — developed end-to-end using agentic AI workflows with [Claude Code](https://claude.ai/claude-code).

## Architecture

| Service | Language | Framework | Port | Purpose |
|---|---|---|---|---|
| `strategy-engine/` | Python 3.12 | FastAPI | 9100 | Signal generation, market data, ML/NLP pipelines |
| `execution-engine/` | Rust | Axum | 9101 | Order routing, risk enforcement, WebSocket feeds |
| `dashboard/` | TypeScript | Next.js 14 | 9102 | Real-time monitoring dashboard |

Services communicate via REST APIs, WebSocket (market data), and SSE (live position/order updates). DuckDB serves as the shared analytics database.

```
┌─────────────────┐     Signals (REST)      ┌──────────────────┐
│ Strategy Engine  │ ──────────────────────► │ Execution Engine │
│  Python/FastAPI  │                         │    Rust/Axum     │
│                  │◄── Bar Data (DuckDB) ──►│                  │
└────────┬────────┘                         └────────┬─────────┘
         │                                           │
         │  Market Data APIs                         │  SSE Events
         │  (REST)                                   │  (positions, orders, P&L)
         │                                           │
         └──────────────┐           ┌────────────────┘
                        ▼           ▼
                   ┌─────────────────────┐
                   │     Dashboard       │
                   │   Next.js 14 / SSE  │
                   └─────────────────────┘
```

## Trading Strategies

### Day Trading (7 strategies)

| Strategy | Type | Description |
|---|---|---|
| MovingAverageCrossover | Technical | Fast/slow MA crossover signals |
| RSIMeanReversion | Technical | RSI oversold/overbought reversals |
| MomentumVolume | Technical | Price momentum confirmed by volume |
| VWAPStrategy | Technical | VWAP deviation mean reversion |
| OpeningRangeBreakout | Technical | First 30-min range breakout |
| MLSignalGenerator | ML | LightGBM classifier on technical features |
| NewsSentimentStrategy | NLP | FinBERT sentiment analysis on live news |

### Swing Trading (2 strategies)

| Strategy | Type | Description |
|---|---|---|
| MultiTimeframeTrend | Technical | Weekly EMA trend + daily RSI pullback entries |
| RelativeStrength | Technical | RS ranking vs SPY benchmark |

A **CompositeScorer** aggregates weighted signals from multiple strategies into a single conviction score for swing trade entries.

## Risk Management

Risk enforcement is implemented in Rust (`execution-engine/src/risk.rs`) and runs before every order submission:

- Maximum daily loss limits
- Per-position size constraints
- Trade frequency throttling
- Automatic EOD position flattening at 3:45 PM ET (day trades only; swing positions exempt)
- All orders must pass risk validation before reaching the Alpaca API

## Dashboard

Yahoo Finance-inspired dark theme with real-time data.

**Pages:** Overview, Watchlist, Positions, Orders, Strategies, Backtest, Risk, Logs, Guide, Quote (per-symbol)

**Overview** features a markets carousel (10 indices/commodities/crypto), sector performance bars, portfolio P&L chart with range tabs, top movers, and a news feed with sentiment badges.

**Quote pages** (`/quote/[symbol]`) provide interactive candlestick/line charts, key statistics (14 metrics), company profiles, and symbol-specific news with sentiment.

**Positions** update in real-time via SSE with live prices during extended hours (4 AM - 8 PM ET).

## Tech Stack

**AI/ML:** LightGBM (signal classification), FinBERT/ProsusAI (news sentiment), Claude Code + MCP (development)

**Backend:** Python 3.12, FastAPI, Rust, Axum, DuckDB

**Frontend:** Next.js 14, Tailwind CSS, Recharts, SSE

**Integrations:** Alpaca Markets (trading + news), Yahoo Finance, yfinance

## Prerequisites

- Python 3.12+
- Rust (stable)
- Node.js 22+
- [Alpaca Markets](https://alpaca.markets/) paper trading account

## Setup

1. **Clone and configure environment:**

```bash
git clone https://github.com/Kleere-Myers/AlgoTrader.git
cd AlgoTrader
cp .env.example .env
# Edit .env with your Alpaca API keys
```

2. **Initialize the database:**

```bash
python scripts/init_db.py
```

3. **Start the strategy engine:**

```bash
cd strategy-engine
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
uvicorn main:app --host 0.0.0.0 --port 9100
```

4. **Start the execution engine:**

```bash
cd execution-engine
cargo run --release
```

5. **Start the dashboard:**

```bash
cd dashboard
npm install
npm run dev
```

The dashboard will be available at `http://localhost:9102`.

## Environment Variables

```
ALPACA_API_KEY=           # Alpaca paper trading API key
ALPACA_SECRET_KEY=        # Alpaca paper trading secret key
ALPACA_MODE=paper         # paper or live
STRATEGY_ENGINE_URL=http://localhost:9100
EXECUTION_ENGINE_URL=http://localhost:9101
DUCKDB_PATH=../data/algotrader.duckdb
NEXT_PUBLIC_EXECUTION_URL=http://localhost:9101
NEXT_PUBLIC_STRATEGY_URL=http://localhost:9100
```

## Tests

```bash
# Strategy engine (160 tests)
cd strategy-engine && python -m pytest

# Execution engine (34 tests)
cd execution-engine && cargo test

# Dashboard (10 route tests)
cd dashboard && npm test
```

**Total: 204 tests** across all three services.

## Instrument Universe

**Core:** SPY, QQQ, AAPL, MSFT, NVDA, GOOGL
**AI Energy:** CEG, GEV, VST, NEE, BE, CCJ, OKLO, LEU, EVRG, PEG, FE, ED

Regular session: 9:30 AM - 4:00 PM ET. Day positions auto-closed by 3:45 PM ET. Swing positions are exempt from EOD flatten.

## Built With Claude Code

This project was built using agentic AI development with [Claude Code](https://claude.ai/claude-code). The multi-agent architecture is defined in `CLAUDE.md` and `AGENT_*.md` files, which coordinate autonomous AI workflows across all three services using shared contracts, structured prompts, and governance rules via MCP (Model Context Protocol).

## Disclaimer

This is a personal project for educational and experimental purposes. It is configured for **paper trading only** by default. Use at your own risk. Not financial advice.

## License

All rights reserved. Not licensed for distribution.
