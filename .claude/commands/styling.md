Dashboard design system with color tokens, typography, and component patterns for the Next.js dashboard. MUST be consulted before ANY dashboard UI work: adding pages, creating components, modifying tables, building charts with Recharts, adding badges or status indicators, creating modals or dialogs, styling buttons or form elements, adding loading skeletons, fixing responsive layouts, changing theme colors, or adding any visual element to the dashboard. If the task touches files in dashboard/app/ or dashboard/components/, use this skill to get the correct Tailwind classes, hex colors for Recharts, font choices, and spacing conventions.

## Design System — "Slate"

A premium dark UI with slate-blue undertones, electric cyan accents, and precision typography. The aesthetic is institutional-grade: dense with data but clean and breathable.

### Design Principles

These principles explain the thinking behind the tokens. When making judgment calls on something not explicitly covered below, lean on these:

1. **Data over chrome** — Financial data is the star. Use `font-mono tabular-nums` on any number a user might compare across rows (prices, percentages, P&L). This ensures columns align visually without extra layout work.
2. **Depth through subtlety** — Surfaces layer from `surface-950` (body) → `surface-900` (cards) → `surface-800` (elevated/inputs) → `surface-700` (hover). Never skip a step — a card's hover state should be one shade lighter, not two.
3. **Purposeful color** — Cyan accent is reserved for interactive affordances (buttons, active states, focus rings, links). Gain/loss colors only appear on actual financial data. Decorative color is avoided.
4. **Monospace section labels** — Section headers use `font-mono uppercase tracking-widest` at small sizes. This separates navigational labels from data text and creates a distinctive rhythm.

---

## Color Palette

### Surfaces (darkest → lightest)
| Usage | Tailwind | CSS var | Hex |
|---|---|---|---|
| Page body | `bg-surface-950` | `--bg-primary` | `#0c0d10` |
| Cards, panels | `bg-surface-900` | `--bg-surface` | `#14151a` |
| Elevated / inputs | `bg-surface-800` | `--bg-card` | `#1a1b22` |
| Hover states | `bg-surface-700` | `--bg-hover` | `#22232b` |
| Borders, dividers | `border-surface-600` | `--border` | `#2e2f38` |
| Muted / scrollbar | `bg-surface-500` | — | `#3e3f4a` |

### Text
| Usage | Tailwind | Hex |
|---|---|---|
| Primary (headings, values) | `text-text-primary` | `#e4e4e7` |
| Secondary (labels, captions) | `text-text-secondary` | `#8b8d98` |

### Accent (cyan)
| Usage | Tailwind | Hex |
|---|---|---|
| Primary (buttons, active states) | `bg-accent` / `text-accent` | `#06b6d4` |
| Hover / light variant | `text-accent-light` | `#22d3ee` |
| Dark variant | `bg-accent-dark` | `#0891b2` |

### Semantic (gain/loss)
| Usage | Tailwind | Hex |
|---|---|---|
| Positive / gain / BUY | `text-gain` / `bg-gain` | `#34d399` |
| Negative / loss / SELL | `text-loss` / `bg-loss` | `#f87171` |
| Gain badge bg | `bg-gain/10` or `bg-gain/15` | — |
| Loss badge bg | `bg-loss/10` or `bg-loss/15` | — |

### Warning (amber — used sparingly for paper mode, pending states)
| Usage | Tailwind |
|---|---|
| Badge bg | `bg-amber-500/10` |
| Badge text | `text-amber-400` |
| Badge border | `border-amber-500/20` |

---

## Typography

### Fonts

Two font families, loaded via `next/font/google` in `layout.tsx`:

| Font | Variable | Tailwind | Purpose |
|---|---|---|---|
| DM Sans | `--font-sans` | `font-sans` (default) | All UI text — headings, labels, prose |
| JetBrains Mono | `--font-mono` | `font-mono` | Financial data, section labels, code, badges |

The body already applies `font-sans antialiased` via layout.tsx. You only need explicit `font-mono` where monospace is desired.

### Type Scale

| Element | Classes |
|---|---|
| Page title | `text-lg font-semibold text-text-primary` |
| Section label | `text-[11px] font-mono font-medium text-text-secondary uppercase tracking-widest` |
| Card title | `text-sm font-semibold text-text-primary` |
| Body text | `text-sm text-text-primary` |
| Caption / label | `text-xs text-text-secondary` |
| Financial value | `text-sm font-mono tabular-nums text-text-primary` |
| Large price | `text-3xl font-bold font-mono tabular-nums text-text-primary` |
| Badge text | `text-xs font-semibold` or `text-[10px] font-semibold` |
| Monospace badge | `text-[11px] font-mono font-medium tracking-wider uppercase` |

---

## Component Patterns

Copy-paste these as starting points. All interactive components need `"use client"` at top of file.

### Card
```
bg-surface-900 rounded-lg border border-surface-600 p-4
```
Hover variant (when the whole card is clickable):
```
bg-surface-900 rounded-lg border border-surface-600 p-4 hover:bg-surface-700 hover:border-surface-500 transition-all duration-150
```

### Table
```
Container: overflow-x-auto
Table:     w-full text-sm text-left border border-surface-600 bg-surface-900 rounded-lg
Header:    bg-surface-800 text-text-secondary uppercase text-xs
Row hover: hover:bg-surface-700 transition-colors
Dividers:  divide-y divide-surface-600
Empty:     px-4 py-8 text-center text-text-secondary
```

### Button (primary)
```
bg-accent text-white text-xs font-medium px-3 py-1.5 rounded-lg hover:bg-accent-dark transition-colors disabled:opacity-50 disabled:cursor-not-allowed
```

### Button (secondary / ghost)
```
text-text-secondary text-xs font-medium px-3 py-1.5 rounded border border-surface-600 hover:bg-surface-700 hover:text-text-primary transition-colors
```

### Badge (gain/loss)
```
Gain: bg-gain/10 text-gain text-xs font-semibold font-mono tabular-nums px-2 py-0.5 rounded-full
Loss: bg-loss/10 text-loss text-xs font-semibold font-mono tabular-nums px-2 py-0.5 rounded-full
```

### Badge (status)
```
Neutral:  bg-surface-600 text-text-secondary text-xs px-2 py-0.5 rounded font-medium
Accent:   bg-accent/15 text-accent-light text-xs px-2 py-0.5 rounded font-medium
Warning:  bg-amber-500/10 text-amber-400 text-xs px-2 py-0.5 rounded font-medium
```

### Tag (small, used for sector/industry)
```
text-[10px] px-1.5 py-0.5 rounded bg-accent/15 text-accent-light
```

### Input / Select
Styled globally in `globals.css` — no extra classes needed beyond standard HTML elements.
- bg: `--bg-surface` (`#14151a`)
- border: `--border` (`#2e2f38`)
- focus: cyan border + subtle `box-shadow` glow
- border-radius: 6px

### Tabs (range selectors, chart toggles)
```
Active:   bg-accent text-white px-3 py-1.5 text-xs font-medium rounded
Inactive: text-text-secondary bg-surface-800 px-3 py-1.5 text-xs font-medium rounded hover:text-text-primary
```

### Nav link (Navbar uses pill-style, not underline)
```
Active:   text-text-primary bg-white/[0.08] px-3 py-1.5 text-[13px] font-medium rounded-md
Inactive: text-text-secondary hover:text-text-primary hover:bg-white/[0.04] px-3 py-1.5 text-[13px] font-medium rounded-md
```

### Loading skeleton
```
bg-surface-800 rounded-lg animate-pulse
```

### Error banner
```
rounded-lg border border-loss/30 bg-loss/10 p-4 text-loss text-sm
```

### Alert / warning box
```
bg-amber-500/10 border border-amber-500/20 rounded-lg p-4 text-sm text-amber-400
```

---

## Recharts Styling

Recharts props require inline hex values — Tailwind classes don't work there. Use these consistently:

| Element | Property | Value |
|---|---|---|
| Grid lines | `stroke` | `#2e2f38` |
| Axis ticks | `fill` | `#8b8d98` |
| Axis line | `stroke` | `#2e2f38` |
| Tooltip bg | `background` | `#1a1b22` |
| Tooltip border | `border` | `1px solid #2e2f38` |
| Tooltip text | `color` | `#8b8d98` |
| Gain stroke | `stroke` | `#34d399` |
| Loss stroke | `stroke` | `#f87171` |
| Accent stroke | `stroke` | `#06b6d4` |
| Volume bars | `fill` | `#2e2f38` |

Standard axis config (apply to all charts):
```tsx
<CartesianGrid strokeDasharray="3 3" stroke="#2e2f38" vertical={false} />
<XAxis stroke="#2e2f38" tick={{ fill: "#8b8d98", fontSize: 11 }} axisLine={false} tickLine={false} />
<YAxis stroke="#2e2f38" tick={{ fill: "#8b8d98", fontSize: 11 }} axisLine={false} tickLine={false} />
```

Set `isAnimationActive={false}` on chart elements for performance (data updates frequently via SSE).

---

## Layout

- **Navbar**: Sticky top, `backdrop-blur-xl`, semi-transparent `bg-surface-950/80`, `border-b border-white/[0.06]`
- **Main content**: `max-w-[1440px] mx-auto px-6 py-6`
- **No sidebar** — all navigation is in the top bar
- **Border radius**: `rounded-lg` on cards and containers, `rounded-md` on buttons and nav pills, `rounded` on small badges
- **Transitions**: `transition-all duration-150` for hover effects, `transition-colors` for simpler color changes

## Directional Indicators

Use triangle SVGs for gain/loss, sized at `w-3 h-3` or `w-2.5 h-2.5`:
```tsx
// Up triangle (gain)
<svg viewBox="0 0 16 16" fill="currentColor" className="w-3 h-3">
  <path d="M8 4l5 8H3z" />
</svg>

// Down triangle (loss)
<svg viewBox="0 0 16 16" fill="currentColor" className="w-3 h-3">
  <path d="M8 12L3 4h10z" />
</svg>
```

Wrap these in a parent with `text-gain` or `text-loss` so `currentColor` picks up the right shade.

## Files Reference

| File | Purpose |
|---|---|
| `tailwind.config.ts` | Color palette (`surface`, `accent`, `gain`, `loss`), font families |
| `app/globals.css` | CSS variables, scrollbar, input, recharts tooltip, selection color |
| `app/layout.tsx` | Font imports (DM Sans, JetBrains Mono), root layout with Navbar |
| `components/Navbar.tsx` | Sticky top nav with backdrop blur, pill-style active states |

## Rules

1. Use the named tokens (`bg-surface-900`, `text-text-primary`, etc.) — never raw Tailwind grays (`bg-gray-800`) or hardcoded hex in className. The token names carry semantic meaning that makes the codebase scannable.
2. This is a dark-only theme. Light backgrounds (`bg-white`, `bg-gray-50`) will break visual coherence.
3. Financial numbers get `font-mono tabular-nums` so columns of data align naturally. Without this, proportional fonts cause jagged number columns that look unprofessional.
4. Gain/loss colors (`text-gain`/`text-loss`) are calibrated for dark backgrounds. Raw `text-green-500`/`text-red-500` have different luminance and won't match.
5. Focus states use the cyan accent (border + subtle glow), keeping the interactive language consistent.
6. Recharts components need inline hex colors — they can't resolve Tailwind classes. The hex values above are the Recharts equivalents of the Tailwind tokens.
7. New components need `"use client"` at the top of the file since the dashboard uses client-side data fetching and SSE hooks.
