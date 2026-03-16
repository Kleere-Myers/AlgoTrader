# AlgoTrader Personal — Claude Code Project Context

## What This Project Is
A self-hosted personal automated trading system for day trading US equities and ETFs.
Built for personal use only. Not licensed for distribution.
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
  "timestamp": "2026-03-16T14:32:00Z"
}
```
`direction` must be exactly: `BUY`, `SELL`, or `HOLD`
`confidence` must be a float between 0.0 and 1.0

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

## Instrument Universe (v1)
SPY, QQQ, AAPL, MSFT, NVDA, GOOGL
Regular session only: 9:30 AM — 4:00 PM ET
All positions auto-closed by 3:45 PM ET

---

## Non-Negotiable Rules (applies to all agents)

1. `ALPACA_MODE=paper` is the default. Live mode requires explicit instruction.
2. Risk rules in `execution-engine/src/risk.rs` are never relaxed without explicit instruction.
3. API keys always come from environment variables. Never hardcode.
4. Dashboard never writes to DuckDB directly. All writes go through service APIs.
5. Every new strategy must extend `BaseStrategy`. No standalone strategy scripts.
6. All Rust order submission must pass risk validation BEFORE calling Alpaca API.
7. `.env` and `data/` are always in `.gitignore`.

---

## Current Build Phase
Update this line as you progress:
**CURRENT PHASE: Phase 3 — Full Strategy Suite**

## Agent Context Files
- `AGENT_STRATEGY.md` — Python strategy engine agent prompt
- `AGENT_EXECUTION.md` — Rust execution engine agent prompt
- `AGENT_DASHBOARD.md` — Next.js dashboard agent prompt

To activate an agent, start your session with:
"Read CLAUDE.md and AGENT_[NAME].md — you are the [Name] agent."
Until that phrase is used, treat all agent files as reference documentation only.

## Known Limitations
- MLSignalGenerator trained on daily bars only (59.4% CV accuracy). Will improve
  once 5-minute bars accumulate from the live WebSocket feed. Revisit retraining
  on intraday data in a future phase.
- LightGBM labels remapped from -1/0/1 to 0/1/2 (SELL/HOLD/BUY) for multiclass
  compatibility. Verify mapping direction if modifying ml/train.py.

## Files That Must Be Gitignored
`.env`, `data/`, `models/` (trained ML artifacts), `__pycache__/`, `target/` (Rust build)