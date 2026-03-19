# Agent Context: Execution Engine
# AlgoTrader Personal — execution-engine/ service

## Your Role
You are the Execution Engine agent. You own everything inside `execution-engine/`.
You do not modify files in `strategy-engine/` or `dashboard/` unless explicitly
asked, and you flag any change that touches a shared contract before making it.

This service is the most safety-critical component in the system.
Every order that touches real money passes through here.
When in doubt, reject — do not execute.

---

## Your Service at a Glance

- **Language:** Rust (stable toolchain)
- **HTTP Framework:** Axum
- **Async Runtime:** Tokio
- **Port:** 9101
- **Database:** DuckDB via `duckdb` crate
- **WebSocket:** tokio-tungstenite (Alpaca market data stream)

---

## Cargo.toml Dependencies

```toml
[dependencies]
axum = { version = "0.7", features = ["macros"] }
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = { version = "0.21", features = ["native-tls"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
duckdb = { version = "0.10", features = ["bundled"] }
dotenvy = "0.15"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
tokio-stream = "0.1"
axum-extra = { version = "0.9", features = ["typed-header"] }
```

---

## Module Structure

```
execution-engine/src/
  main.rs          # Axum router setup, service startup, state initialization
  alpaca.rs        # Alpaca REST client + WebSocket market data feed
  risk.rs          # Risk rule enforcement — the safety gate
  orders.rs        # Order construction and submission to Alpaca
  positions.rs     # In-memory position state + DuckDB sync
  db.rs            # DuckDB connection pool and query helpers
  sse.rs           # Server-Sent Events broadcaster for dashboard
  models.rs        # Shared Rust structs (Signal, Order, Position, etc.)
  scheduler.rs     # Market hours logic, EOD flatten job
```

---

## Core Structs (models.rs)

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Signal {
    pub symbol: String,
    pub direction: Direction,
    pub confidence: f64,
    pub reason: String,
    pub strategy_name: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Direction {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Position {
    pub symbol: String,
    pub qty: f64,
    pub avg_entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub opened_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Order {
    pub order_id: String,
    pub alpaca_id: Option<String>,
    pub symbol: String,
    pub side: String,          // "buy" or "sell"
    pub qty: f64,
    pub filled_price: Option<f64>,
    pub status: String,
    pub strategy_name: String,
    pub submitted_at: String,
    pub filled_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SseEvent {
    pub event_type: SseEventType,
    pub timestamp: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SseEventType {
    PositionUpdate,
    OrderFill,
    TradingHalted,
    TradingResumed,
    DailyPnl,
    RiskBreach,
}
```

---

## Risk Rules (risk.rs) — NON-NEGOTIABLE

These rules are the last gate before any order reaches Alpaca.
**Never make these rules more permissive without explicit written instruction.**
It is always safe to make them more restrictive.

```rust
pub struct RiskConfig {
    pub max_daily_loss_pct: f64,        // Default: 0.02 (2% of account equity)
    pub max_position_size_pct: f64,     // Default: 0.10 (10% of equity per symbol)
    pub max_open_positions: usize,      // Default: 4
    pub min_signal_confidence: f64,     // Default: 0.60
    pub order_throttle_secs: u64,       // Default: 300 (5 min per symbol)
    pub eod_flatten_time_et: &'static str, // Default: "15:45"
}

pub enum RiskDecision {
    Approved,
    Rejected(String),   // String = reason for rejection
    HaltAll(String),    // Daily loss limit breached — halt all trading
}
```

### Risk Checks (run in this order)
1. Is trading currently halted? → Reject immediately
2. Is daily loss >= max_daily_loss_pct of equity? → HaltAll
3. Is signal confidence >= min_signal_confidence? → Reject if not
4. Is direction HOLD? → Return Approved (no order needed)
5. Does open position count >= max_open_positions (for BUY)? → Reject
6. Would new position size exceed max_position_size_pct of equity? → Reject
7. Was an order submitted for this symbol within throttle window? → Reject
8. All checks passed → Approved

Log every rejection with reason to tracing and write to DuckDB `signals` table.

---

## Alpaca Integration (alpaca.rs)

### Environment Variables
```
ALPACA_API_KEY      — from .env via dotenvy
ALPACA_SECRET_KEY   — from .env via dotenvy
ALPACA_MODE         — "paper" or "live" — DEFAULT IS PAPER
```

### Base URLs
```rust
const PAPER_BASE_URL: &str = "https://paper-api.alpaca.markets";
const LIVE_BASE_URL: &str  = "https://api.alpaca.markets";
const DATA_WS_URL: &str    = "wss://stream.data.alpaca.markets/v2/iex";

fn base_url() -> &'static str {
    match std::env::var("ALPACA_MODE").as_deref() {
        Ok("live") => LIVE_BASE_URL,
        _ => PAPER_BASE_URL,    // default to paper
    }
}
```

### Key REST Endpoints Used
- `GET /v2/account` — fetch equity and buying power on startup
- `GET /v2/positions` — sync positions on startup
- `POST /v2/orders` — submit market orders
- `GET /v2/orders` — fetch open orders on startup
- `DELETE /v2/orders/{id}` — cancel order

### WebSocket Feed
Subscribe to 5-minute bars for all 6 instruments on connect:
```json
{"action": "subscribe", "bars": ["SPY", "QQQ", "AAPL", "MSFT", "NVDA", "GOOGL"]}
```
On each bar received:
1. Upsert to DuckDB `ohlcv_bars`
2. POST bar to Strategy Engine `POST /signal`
3. For each signal returned: run risk check → submit order if approved

---

## Axum REST Endpoints

| Method | Path | Description |
|---|---|---|
| GET | /positions | Current open positions with unrealized P&L |
| GET | /orders | Last 100 orders from DuckDB |
| GET | /account | Equity, buying power, today's realized P&L |
| POST | /trading/halt | Set trading_halted = true, broadcast SSE TradingHalted |
| POST | /trading/resume | Set trading_halted = false, broadcast SSE TradingResumed |
| GET | /positions/day | Day-trade positions only |
| GET | /positions/swing | Swing positions only |
| GET | /stream/events | SSE endpoint — keep-alive, push SseEvents to dashboard |
| GET | /health | Returns `{"status": "ok", "mode": "paper|live"}` |

### CORS
Allow all origins from localhost only:
`http://localhost:9102` — dashboard in development

---

## SSE Broadcaster (sse.rs)

Use a `tokio::sync::broadcast` channel with capacity 100.
The `/stream/events` handler subscribes to the channel and streams events
as `text/event-stream` with `data: {json}\n\n` format.

Broadcast events on:
- Every position update after an order fills
- Every order fill confirmation from Alpaca
- Position price refreshes (every 15s via quote_refresh_loop)
- Position sync corrections (every ~5 min, payload has `action: "SYNC"`)
- Trading halt / resume state changes
- Daily P&L updates (every 5 minutes during market hours)
- Risk breach rejections (so dashboard can surface them)

---

## Scheduler Background Loops (scheduler.rs)

### EOD Flatten (`eod_flatten_loop`)
Every 30s, checks if time >= 15:45 ET. Flattens all day-trade positions.
If a flatten order fails (e.g. qty mismatch), syncs with Alpaca's actual
holdings and retries with the corrected qty. Swing positions are exempt.

### Quote Refresh (`quote_refresh_loop`)
Every 15s during extended hours (4 AM – 8 PM ET), fetches latest trade prices
from Alpaca (`/v2/stocks/trades/latest`) and updates position `current_price`
and `unrealized_pnl`. Broadcasts SSE `POSITION_UPDATE` so the dashboard updates.
Every ~5 minutes, does a full position sync with Alpaca's `/v2/positions` to
correct any qty drift (manual trades, partial fills, etc.).

### Swing Signal Scanner (`daily_swing_signal_loop`)
Fires once at 4:05 PM ET. Fetches daily bars, generates composite swing signals.

### Swing Stop/Take Monitor (`swing_stop_check_loop`)
Every 60s during market hours. Checks swing positions against stop-loss and
take-profit levels.

Use `chrono` with `America/New_York` timezone.
Simple NYSE holiday list is acceptable for v1 — no need for external calendar API.

---

## Database Access (db.rs)

Tables you write:
- `ohlcv_bars` — upsert on each bar received
- `orders` — insert on submission, update on fill
- `positions` — upsert after every fill
- `daily_pnl` — upsert daily summary at EOD

Tables you read:
- `signals` — for rejection logging (write) and audit queries (read)
- `strategy_config` — read to know which strategies are active
- `ohlcv_bars` — read for recent price context

Tables you never modify:
- `backtest_results` — owned by strategy-engine

---

## Startup Sequence (main.rs)

```
1. Load .env via dotenvy
2. Initialize tracing subscriber
3. Connect to DuckDB
4. Fetch Alpaca account — verify auth works
5. Load positions from DuckDB, then sync with Alpaca /v2/positions
   (corrects qty/price drift, adds missing positions, removes stale ones)
6. Build shared AppState
7. Launch WebSocket feed connection (spawn Tokio task)
8. Launch EOD flatten scheduler (spawn Tokio task)
8b. Launch daily swing signal scanner (spawn Tokio task)
8c. Launch swing stop/take monitor (spawn Tokio task)
8d. Launch quote refresh loop (spawn Tokio task)
9. Start Axum HTTP server on port 9101
```

If Alpaca auth fails on startup, log error and exit — do not start the server
in a degraded state.

---

## Testing Requirements

- Unit test all risk rule logic in `risk.rs` — every rejection path must have a test
- Mock Alpaca HTTP calls using `wiremock` or similar — never hit real Alpaca in tests
- Test the EOD flatten trigger with mocked time
- Test SSE broadcaster with a test subscriber
- Run tests: `cargo test`
- Run with logging: `RUST_LOG=debug cargo run`

---

## What to Flag Before Doing

- Any change to the Signal struct fields or the Direction enum variants
- Any change to the SseEvent format
- Any loosening of risk rules
- Any change that affects how orders are submitted (new order types, etc.)
- Any change to the DuckDB schema
- Any change to the Alpaca base URL selection logic
