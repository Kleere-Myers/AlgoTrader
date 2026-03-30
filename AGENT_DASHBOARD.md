# Agent Context: Dashboard
# AlgoTrader Personal — dashboard/ service

## Your Role
You are the Dashboard agent. You own everything inside `dashboard/`.
You do not modify files in `strategy-engine/` or `execution-engine/` unless
explicitly asked, and you flag any change that touches a shared contract first.

You are a read-only consumer of both backend services.
You never write to the database directly. All data comes through the service APIs.

**Before making ANY UI changes, run `/styling` to load the design system reference.**

---

## Your Service at a Glance

- **Framework:** Next.js 14 (App Router)
- **Language:** TypeScript
- **Port:** 3000
- **Styling:** Tailwind CSS — Yahoo Finance dark mode theme
- **Charts:** Recharts
- **Real-time:** Server-Sent Events from execution-engine `/stream/events`
- **Layout:** Top navbar (no sidebar), full-width content

---

## Design System

The dashboard uses a **Yahoo Finance dark mode** design language.
Full reference is in `.claude/commands/styling.md` (the `/styling` skill).

Key rules:
- Body: `bg-navy-950` (#101518)
- Cards: `bg-navy-900` with `border border-navy-600 rounded-lg`
- Text: `text-text-primary` (#f0f3f5) and `text-text-secondary` (#b0b9c1)
- Gain/Loss: `text-gain` (#21d87d) / `text-loss` (#fc7a6e) — softer dark-mode variants
- Accent: `bg-accent-purple` (#9d61ff), links use `accent-blue` (#12a9ff)
- Font: Helvetica Neue, antialiased
- NEVER use raw `text-gray-*` classes or light backgrounds

---

## Key Dependencies

```json
{
  "dependencies": {
    "next": "14.x",
    "react": "18.x",
    "react-dom": "18.x",
    "recharts": "^2.x"
  },
  "devDependencies": {
    "typescript": "^5.x",
    "tailwindcss": "^3.x",
    "@types/react": "^18.x",
    "@types/node": "^20.x"
  }
}
```

---

## Environment Variables

```
NEXT_PUBLIC_EXECUTION_URL=http://localhost:9101
NEXT_PUBLIC_STRATEGY_URL=http://localhost:9100
```

---

## Page Map

| Route | Page | Key Data Sources |
|---|---|---|
| `/` | Overview | Market indices, sectors, P&L history, movers, news feed |
| `/watchlist` | Watchlist | GET /company/{symbol}, GET /news/{symbol} |
| `/positions` | Positions | GET /positions (execution), SSE for live updates |
| `/orders` | Orders | GET /orders (execution) |
| `/strategies` | Strategies | GET /strategies (strategy), POST /backtest triggers |
| `/backtest` | Backtest | GET /backtest/{strategy}/{symbol} (strategy) |
| `/risk` | Risk Settings | GET/PATCH /risk/config, trading halt/resume |
| `/logs` | Live Logs | SSE /stream/events — raw event stream |
| `/guide` | User Guide | Static content, no API calls |

---

## App Router Structure

```
dashboard/
  app/
    layout.tsx          # Root layout — Navbar, bg-navy-950 body
    globals.css         # CSS vars, scrollbar, input, recharts overrides
    page.tsx            # Overview (markets, sectors, P&L, movers, news)
    watchlist/page.tsx
    positions/page.tsx
    orders/page.tsx
    strategies/page.tsx
    backtest/page.tsx
    risk/page.tsx
    logs/page.tsx
    guide/page.tsx
  components/
    Navbar.tsx              # Top nav bar with active state
    MarketIndexCard.tsx     # Compact market card with directional arrow
    SparklineChart.tsx      # Minimal recharts line (no axes)
    SectorPerformanceBar.tsx # Horizontal bar chart for sectors
    PnlChart.tsx            # Area chart with 1D/1W/1M/3M/YTD tabs
    PortfolioSummary.tsx    # Financial breakdown sidebar
    MoversList.tsx          # Tabbed gainers/losers list
    NewsCard.tsx            # Editorial news card with thumbnail
    CandlestickChart.tsx    # OHLCV candlestick with signal overlay
    EquityCurveChart.tsx    # Backtest equity line chart
    StrategyCard.tsx        # Strategy toggle/params/backtest card
    WatchlistCard.tsx       # Company info + news sentiment card
    EmergencyHaltButton.tsx # Trading halt control with confirmation
    Tip.tsx                 # Portal-based tooltip
  hooks/
    useSseEvents.ts     # SSE stream with auto-reconnect
    useSymbols.ts       # Symbol list management
  lib/
    api.ts              # Typed fetch wrappers for both service APIs
  types/
    index.ts            # All shared TypeScript types
```

---

## Shared Types (types/index.ts)

**Never change these without flagging it — they mirror Rust and Python structs.**

Key types: `Signal`, `Position`, `Order`, `SseEvent`, `AccountInfo`, `Strategy`,
`BacktestResult`, `RiskConfig`, `CompanyInfo`, `NewsArticle`, `OhlcvBar`

Added for dashboard redesign: `TradeType`, `MarketIndex`, `SectorPerformance`,
`MarketMover`, `PortfolioPnlHistory`, `HistoryRange`

`CompanyInfo` includes extended quote fields: `trailing_pe`, `forward_pe`, `eps`,
`beta`, `dividend_rate`, `dividend_yield`, `open`, `day_high`, `day_low`, `volume`,
`bid`, `ask`, `exchange`, `currency`, `target_mean_price`, etc.

---

## API Client (lib/api.ts)

All API calls go through typed wrappers. Never call fetch directly in pages.

**executionApi** (port 9101):
- `getAccount()`, `getPositions()`, `getOrders()`
- `haltTrading()`, `resumeTrading()`
- `getRiskConfig()`, `patchRiskConfig(patch)`
- `sseUrl` for SSE stream

**strategyApi** (port 9100):
- `getSymbols()`, `addSymbol()`, `removeSymbol()`
- `getStrategies()`, `triggerBacktest()`, `getBacktestResult()`
- `getBars()`, `getHistoricalBars(symbol, range)`, `getCompanyInfo()`, `getNews()`
- `getMarketIndices()`, `getSectorPerformance()`
- `getMarketMovers()`, `getPnlHistory(range)`, `getNewsFeed(limit)`
- `getNewsFeed(limit)`

---

## Recharts Dark Theme

Recharts cannot use Tailwind classes — use inline hex values:
- Grid: `stroke="#3a434c"`, `vertical={false}`
- Axis ticks: `fill="#b0b9c1"`, `axisLine={false}`, `tickLine={false}`
- Tooltip bg: `#232a31`, border: `#3a434c`
- Gain stroke: `#21d87d`, Loss stroke: `#fc7a6e`, Accent: `#9d61ff`

---

## What to Flag Before Doing

- Any change to `types/index.ts` (mirrors Rust/Python structs)
- Any change to SSE event format in `useSseEvents.ts`
- Any new page requiring a new backend API endpoint
- Any new dependency over 50KB
- Any attempt to write data directly to the database or call Alpaca directly
- Any deviation from the Yahoo Finance dark theme (run `/styling` first)
