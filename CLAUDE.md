# AlgoTrader Personal — Claude Code Project Context

## What This Project Is
A self-hosted personal automated trading system for day trading and swing trading
US equities and ETFs. Built for personal use only. Not licensed for distribution.
Full PRD is in `AlgoTrader_PRD.docx` at the project root — read it before making
architectural decisions.

---

## Multi-Agent Usage Policy

**DO NOT use multiple parallel agents until Phase 3.**

Phases 1 and 2 must be built with a single Claude Code session to establish
and validate the shared contracts before splitting responsibilities.

The shared contracts are not stable until ALL of the following are true:
- [x] BaseStrategy interface is implemented and tested
- [x] Signal struct is in use end-to-end (Python → Rust)
- [x] DuckDB schema is initialized and both services are reading/writing it
- [x] SSE event format is confirmed working in the dashboard
- [x] At least one strategy is paper trading successfully end-to-end

When all boxes above are checked, update CURRENT PHASE to Phase 3 and agents
may be split. Until then, use a single session and work across all three
service directories as needed.

---

## Three Services — Clear Boundaries

| Service | Language | Port | Owner Agent |
|---|---|---|---|
| `strategy-engine/` | Python 3.12 + FastAPI | 8000 | AGENT_STRATEGY.md |
| `execution-engine/` | Rust + Axum | 8080 | AGENT_EXECUTION.md |
| `dashboard/` | Next.js 14 | 3000 | AGENT_DASHBOARD.md |

**Each agent owns exactly one service directory. Never modify files outside your
service without explicitly flagging it first.**

---

## Shared Contracts — Never Change Without Coordinating All Three Agents

These are the interfaces that cross service boundaries. Changing any of these
unilaterally WILL break other services.

### 1. Signal Struct
Produced by: strategy-engine
Consumed by: execution-engine

```json
{
  "symbol": "AAPL",
  "direction": "BUY",
  "confidence": 0.75,
  "reason": "RSI crossed below 30",
  "strategy_name": "RSIMeanReversion",
  "timestamp": "2026-03-16T14:32:00Z",
  "trade_type": "day"
}
```
`direction` must be exactly: `BUY`, `SELL`, or `HOLD`
`confidence` must be a float between 0.0 and 1.0
`trade_type` must be `"day"` or `"swing"` (defaults to `"day"` if omitted)

### 2. SSE Event Format
Produced by: execution-engine
Consumed by: dashboard

```json
{
  "event_type": "PostionUpdate | ORDER_FILL | TRADING_HALTED | DAILY_PNL",
  "timestamp": "2026-03-16T14:32:00Z",
  "payload": { }
}
```

### 3. DuckDB Schema
Shared by: strategy-engine (read/write) and execution-engine (read/write)
Dashboard never writes to the database directly — only reads via service APIs.

Tables: `ohlcv_bars`, `signals`, `orders`, `positions`, `daily_pnl`, `strategy_config`
Schema definition: `scripts/init_db.py`
Database file: `data/algotrader.duckdb` (gitignored)

**DuckDB version MUST match across both services.** Currently pinned to **1.2.x**
(`duckdb = "1.1"` in Cargo.toml resolves to 1.2.2; `duckdb>=1.2,<1.3` in requirements.txt).
Mismatched versions cause silent storage-format incompatibility — bars won't persist.

**Rust DuckDB queries must CAST TIMESTAMP columns to VARCHAR** before reading into
String fields (e.g. `CAST(timestamp AS VARCHAR)`). The 1.2 Rust driver does not
auto-coerce TIMESTAMP → String like 0.10 did.

---

## Required Environment Variables

```
ALPACA_API_KEY=
ALPACA_SECRET_KEY=
ALPACA_MODE=paper              # NEVER change to live without explicit instruction
STRATEGY_ENGINE_URL=http://localhost:8000
EXECUTION_ENGINE_URL=http://localhost:8080
DUCKDB_PATH=../data/algotrader.duckdb
NEXT_PUBLIC_EXECUTION_URL=http://localhost:8080
NEXT_PUBLIC_STRATEGY_URL=http://localhost:8000
```

Stored in `.env` at project root. `.env` is gitignored. Never log API keys.

---

## Instrument Universe (v2)
**Core:** SPY, QQQ, AAPL, MSFT, NVDA, GOOGL
**AI Energy:** CEG, GEV, VST, NEE, BE, CCJ, OKLO, LEU, EVRG, PEG, FE, ED

Symbol list is managed at runtime via `GET/POST/DELETE /symbols` on the strategy engine.
Default list is set via `SYMBOLS` env var or `DEFAULT_SYMBOLS` in `strategy-engine/main.py`.

Regular session only: 9:30 AM — 4:00 PM ET
Day positions auto-closed by 3:45 PM ET (swing positions are exempt)

---

## Non-Negotiable Rules (applies to all agents)

1. `ALPACA_MODE=paper` is the default. Live mode requires explicit instruction.
2. Risk rules in `execution-engine/src/risk.rs` are never relaxed without explicit instruction.
3. API keys always come from environment variables. Never hardcode.
4. Dashboard never writes to DuckDB directly. All writes go through service APIs.
5. Every new strategy must extend `BaseStrategy`. No standalone strategy scripts.
6. All Rust order submission must pass risk validation BEFORE calling Alpaca API.
7. `.env` and `data/` are always in `.gitignore`.
8. DuckDB version must stay aligned between Python and Rust. Never upgrade one without the other.
9. Dashboard must follow the Yahoo Finance dark theme. Run `/styling` before any UI changes.

---

## Current Build Phase
Update this line as you progress:
**CURRENT PHASE: Phase 5 — Live Trading Transition**

## Test Baseline (Phase 5 current)
- Rust:   34/34 (includes 8 swing risk tests)
- Python: 160/160 (includes swing trading: 9 composite + 10 multi-timeframe + 11 relative strength)
- Next.js: 10/10 routes (includes /watchlist)
- Total:  204 tests

## Agent Context Files
- `AGENT_STRATEGY.md` — Python strategy engine agent prompt
- `AGENT_EXECUTION.md` — Rust execution engine agent prompt
- `AGENT_DASHBOARD.md` — Next.js dashboard agent prompt

To activate an agent, start your session with:
"Read CLAUDE.md and AGENT_[NAME].md — you are the [Name] agent."
Until that phrase is used, treat all agent files as reference documentation only.

## Strategies (9 total — 7 day + 2 swing)

### Day Trading Strategies
| Strategy | Type | File |
|---|---|---|
| MovingAverageCrossover | Technical | `strategy_moving_average.py` |
| RSIMeanReversion | Technical | `strategy_rsi.py` |
| MomentumVolume | Technical | `strategy_momentum_volume.py` |
| MLSignalGenerator | ML | `strategy_ml_signal.py` |
| VWAPStrategy | Technical | `strategy_vwap.py` |
| OpeningRangeBreakout | Technical | `strategy_orb.py` |
| NewsSentimentStrategy | NLP/FinBERT | `strategy_news_sentiment.py` |

### Swing Trading Strategies
| Strategy | Type | File |
|---|---|---|
| MultiTimeframeTrend | Weekly EMA + Daily RSI pullback | `strategy_multi_timeframe.py` |
| RelativeStrength | RS ranking vs SPY benchmark | `strategy_relative_strength.py` |

Swing signals are generated via `POST /signal/swing` using daily bars. A `CompositeScorer`
(`strategies/composite_scorer.py`) aggregates weighted signals from swing + compatible day
strategies into a single conviction score. Positions with `trade_type="swing"` are exempt
from EOD auto-flatten.

Shared utilities for the news strategy:
- `strategies/news_fetcher.py` — Alpaca News API (with thumbnail extraction) + yfinance fallback, 5-min TTL cache
- `strategies/sentiment.py` — FinBERT (`ProsusAI/finbert`) lazy-loaded sentiment scorer

FinBERT model (~500MB) auto-downloads on first use to `~/.cache/huggingface/`.

### Strategy Engine Market Data Endpoints
| Endpoint | Purpose | Cache |
|---|---|---|
| `GET /market/indices` | 10 markets (indices, VIX, bonds, commodities, crypto, FX) with sparkline data | 60s |
| `GET /market/sectors` | 11 sector ETF daily performance | 5min |
| `GET /market/movers` | Portfolio symbols ranked by daily change % | 60s |
| `GET /portfolio/pnl-history?range=` | P&L time series + summary (1d/1w/1m/3m/ytd) | none |
| `GET /news/feed?limit=` | Aggregated news across tracked symbols with thumbnails | none |

**Important:** The strategy engine must be started with `.env` sourced so Alpaca API
keys are available for news and market data endpoints.

## Dashboard (Next.js 14 — Yahoo Finance dark theme)

**Design system:** Yahoo Finance dark mode (`/styling` skill for full reference).
Top navbar layout, full-width, Helvetica Neue font, `#101518` body background.

### Routes (10 total)
`/` Overview, `/watchlist` Watchlist, `/positions`, `/orders`, `/strategies`,
`/backtest`, `/risk`, `/logs`, `/guide`

### Overview Page (`/`)
Yahoo Finance-inspired layout with:
- **Markets carousel** — horizontal scrolling cards for S&P 500, Dow 30, Nasdaq,
  Russell 2000, VIX, 10-Yr Bond, Gold, Crude Oil, Bitcoin, EUR/USD
  (data from `GET /market/indices` on strategy engine, 60s cache)
- **Sector performance** — horizontal bars showing daily change for 11 sector ETFs
  (`GET /market/sectors`, 5-min cache)
- **Portfolio P&L chart** — area chart with 1D/1W/1M/3M/YTD tabs + financial summary sidebar
  (`GET /portfolio/pnl-history?range=`)
- **Top Movers** — gainers/losers from tracked symbols (`GET /market/movers`)
- **News feed** — editorial cards with thumbnails, sentiment, symbol badges
  (`GET /news/feed`)

### Components (14 total)
Navbar, MarketIndexCard, SparklineChart, SectorPerformanceBar, PnlChart,
PortfolioSummary, MoversList, NewsCard, CandlestickChart, EquityCurveChart,
StrategyCard, WatchlistCard, EmergencyHaltButton, Tip

### Watchlist Page
Shows company info + news with sentiment for all tracked symbols.
Data from `GET /company/{symbol}` and `GET /news/{symbol}` on strategy engine.

## Known Limitations
- MLSignalGenerator trained on daily bars only (59.4% CV accuracy). Will improve
  once 5-minute bars accumulate from the live WebSocket feed. Revisit retraining
  on intraday data in a future phase.
- LightGBM labels remapped from -1/0/1 to 0/1/2 (SELL/HOLD/BUY) for multiclass
  compatibility. Verify mapping direction if modifying ml/train.py.
- NewsSentimentStrategy backtest returns zeros — no historical news data to
  backtest against.
- Strategies need a warmup period after service restart — 5-min bars must
  accumulate before lookback windows are satisfied (30 bars = ~2.5 hours for
  MovingAverageCrossover, 14 for RSI, 6 for VWAP/ORB).

## Skills (Slash Commands)
- `/dev <start|stop|restart|status> [service]` — manage dev services
- `/styling` — dashboard layout and styling reference (colors, typography, component patterns)
- `/permissions <show|reset|add <rule>>` — manage tool permissions

## Files That Must Be Gitignored
`.env`, `data/`, `models/` (trained ML artifacts), `__pycache__/`, `target/` (Rust build)