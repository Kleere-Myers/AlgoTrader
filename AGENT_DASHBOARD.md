# Agent Context: Dashboard
# AlgoTrader Personal — dashboard/ service

## Your Role
You are the Dashboard agent. You own everything inside `dashboard/`.
You do not modify files in `strategy-engine/` or `execution-engine/` unless
explicitly asked, and you flag any change that touches a shared contract first.

You are a read-only consumer of both backend services.
You never write to DuckDB directly. All data comes through the service APIs.

---

## Your Service at a Glance

- **Framework:** Next.js 14 (App Router)
- **Language:** TypeScript
- **Port:** 3000
- **Styling:** Tailwind CSS (no external component library required)
- **Charts:** Recharts
- **Real-time:** Server-Sent Events from execution-engine `/stream/events`

---

## Key Dependencies

```json
{
  "dependencies": {
    "next": "14.x",
    "react": "18.x",
    "react-dom": "18.x",
    "recharts": "^2.x",
    "tailwindcss": "^3.x"
  },
  "devDependencies": {
    "typescript": "^5.x",
    "@types/react": "^18.x",
    "@types/node": "^20.x"
  }
}
```

---

## Environment Variables

```
NEXT_PUBLIC_EXECUTION_URL=http://localhost:8080
NEXT_PUBLIC_STRATEGY_URL=http://localhost:8000
```

Both are browser-accessible (NEXT_PUBLIC_). No server-side proxying needed for
localhost development. When migrating to cloud, update these to point to deployed
service URLs.

---

## Page Map

| Route | Page | Key Data Sources |
|---|---|---|
| `/` | Overview | GET /account, GET /positions, SSE /stream/events |
| `/positions` | Positions | GET /positions (execution), SSE for live updates |
| `/orders` | Orders | GET /orders (execution) |
| `/strategies` | Strategies | GET /strategies (strategy), POST /backtest triggers |
| `/backtest` | Backtest Results | GET /backtest/{strategy}/{symbol} (strategy) |
| `/risk` | Risk Settings | GET /account, POST /trading/halt, POST /trading/resume |
| `/logs` | Live Logs | SSE /stream/events — show raw event stream |

---

## Shared Types (types/index.ts)

Define these TypeScript types to match the backend contracts exactly.
**Never change these without flagging it — they mirror Rust and Python structs.**

```typescript
export type Direction = "BUY" | "SELL" | "HOLD";

export interface Signal {
  symbol: string;
  direction: Direction;
  confidence: number;        // 0.0 to 1.0
  reason: string;
  strategy_name: string;
  timestamp: string;         // ISO 8601 UTC
}

export interface Position {
  symbol: string;
  qty: number;
  avg_entry_price: number;
  current_price: number;
  unrealized_pnl: number;
  opened_at: string;
}

export interface Order {
  order_id: string;
  alpaca_id: string | null;
  symbol: string;
  side: "buy" | "sell";
  qty: number;
  filled_price: number | null;
  status: string;
  strategy_name: string;
  submitted_at: string;
  filled_at: string | null;
}

export type SseEventType =
  | "PositionUpdate"
  | "OrderFill"
  | "TradingHalted"
  | "TradingResumed"
  | "DailyPnl"
  | "RiskBreach";

export interface SseEvent {
  event_type: SseEventType;
  timestamp: string;
  payload: Record<string, unknown>;
}

export interface AccountInfo {
  equity: number;
  buying_power: number;
  cash: number;
  currency: string;
  status: string;
  mode: string;
  trading_blocked: boolean;
}

export interface Strategy {
  id: string;
  name: string;
  enabled: boolean;
  params: Record<string, unknown>;
  last_signal: Signal | null;
  win_rate: number | null;
}

export interface BacktestResult {
  strategy_name: string;
  symbol: string;
  total_return_pct: number;
  sharpe_ratio: number;
  max_drawdown_pct: number;
  win_rate: number;
  total_trades: number;
  avg_trade_duration_mins: number;
  profit_factor: number;
  period_start: string;
  period_end: string;
}
```

---

## SSE Hook (hooks/useSseEvents.ts)

Create a reusable hook for subscribing to the execution engine SSE stream:

```typescript
// Usage: const { events, isConnected, isTradingHalted } = useSseEvents()
// Reconnects automatically on disconnect
// Exposes isTradingHalted derived from TradingHalted/TradingResumed events
```

The SSE stream must be consumed in a Client Component (`"use client"`).
Keep the SSE connection at a high level (e.g. layout) so it persists across
page navigation without reconnecting.

---

## Component Specs

### RiskStatusBar
- Persistent top-of-page banner
- Shows: daily P&L used vs limit as a progress bar
- Color: green → yellow → red as loss approaches limit
- When `trading_halted = true`: shows red banner "TRADING HALTED"
- Always visible regardless of current page

### EquityCurveChart
- Recharts `LineChart`
- X axis: timestamps, Y axis: account equity in dollars
- Show a dotted reference line at starting equity
- Tooltip shows exact equity and % change from start

### CandlestickChart
- Recharts `ComposedChart` with custom candlestick rendering
- Overlay BUY signals as green upward triangles
- Overlay SELL signals as red downward triangles
- Show volume bars at bottom in secondary Y axis

### PositionsTable
- Sortable by symbol, P&L, duration
- Unrealized P&L column: green text if positive, red if negative
- Duration column: show as "2h 14m" format
- Updates in real-time via SSE PositionUpdate events

### StrategyCard
- Shows: strategy name, enabled toggle, last signal direction + confidence badge
- Win rate badge: color-coded (green >55%, yellow 45-55%, red <45%)
- "Run Backtest" button: calls POST /backtest/{strategy}/{symbol}, shows loading state
- Params section: expandable, shows current params as editable key-value pairs

### EmergencyHaltButton
- Large red button on the /risk page
- Calls POST /trading/halt or POST /trading/resume depending on current state
- Shows confirmation dialog before halting
- Disabled during market-closed hours (grey out, tooltip explains why)

---

## App Router Structure

```
dashboard/
  app/
    layout.tsx          # Root layout — includes RiskStatusBar, SSE provider
    page.tsx            # Overview page (/)
    positions/
      page.tsx
    orders/
      page.tsx
    strategies/
      page.tsx
    backtest/
      page.tsx
    risk/
      page.tsx
    logs/
      page.tsx
  components/
    RiskStatusBar.tsx   # "use client" — reads SSE state
    EquityCurveChart.tsx
    CandlestickChart.tsx
    PositionsTable.tsx  # "use client" — real-time updates
    StrategyCard.tsx    # "use client" — toggle, backtest button
    EmergencyHaltButton.tsx  # "use client"
  hooks/
    useSseEvents.ts     # "use client" hook
    usePositions.ts
    useOrders.ts
    useStrategies.ts
  lib/
    api.ts              # Typed fetch wrappers for both service APIs
  types/
    index.ts            # All shared TypeScript types (see above)
```

---

## API Client (lib/api.ts)

Create typed fetch wrappers. Never call fetch directly in page/component files.

```typescript
// All functions return typed responses or throw on error
export const executionApi = {
  getPositions: (): Promise<Position[]>
  getOrders: (): Promise<Order[]>
  getAccount: (): Promise<AccountInfo>
  haltTrading: (): Promise<void>
  resumeTrading: (): Promise<void>
}

export const strategyApi = {
  getStrategies: (): Promise<Strategy[]>
  updateStrategy: (id: string, patch: Partial<Strategy>): Promise<Strategy>
  triggerBacktest: (strategy: string, symbol: string): Promise<BacktestResult>
  getBacktestResult: (strategy: string, symbol: string): Promise<BacktestResult>
}
```

---

## Rendering Strategy

Use React Server Components (RSC) for initial data fetching on page load.
Use Client Components (`"use client"`) only where real-time interactivity is needed:
- Anything that subscribes to SSE
- Anything with user input (toggles, buttons, editable params)
- Anything that needs to re-render on SSE events

Pages that are mostly static (orders history, backtest results) can be RSC with
a manual refresh button rather than live polling.

---

## Styling Guidelines

- Use Tailwind utility classes only — no custom CSS files unless absolutely necessary
- Color palette: dark navy sidebar, white content area, blue accents (`blue-600`)
- P&L positive: `text-green-600`, P&L negative: `text-red-600`
- Trading halted state: `bg-red-100 border-red-500` banner
- Paper mode indicator: subtle yellow badge in the nav ("PAPER MODE")
- Live mode indicator: subtle green badge in the nav ("LIVE MODE")
- Always show which mode is active so there's never ambiguity

---

## Testing Requirements

- Component tests using React Testing Library for key components
- Test PositionsTable renders correctly with mock position data
- Test RiskStatusBar shows halted state correctly
- Test EmergencyHaltButton shows confirmation dialog before calling API
- Test useSseEvents hook reconnects on disconnect
- Run tests: `npm test`

---

## What to Flag Before Doing

- Any change to the TypeScript types in `types/index.ts`
- Any change to the SSE event format expected by `useSseEvents.ts`
- Any addition of a new page that requires a new API endpoint
- Any new dependency over 50KB that could affect bundle size
- Any attempt to write data directly to DuckDB or call Alpaca directly
