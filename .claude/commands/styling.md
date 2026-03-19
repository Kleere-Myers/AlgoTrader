Dashboard layout and styling reference. Consult this skill before creating or modifying any dashboard UI.

## Design System — Yahoo Finance Dark Mode

The dashboard follows Yahoo Finance's dark mode design language. All new pages, components, and UI changes MUST use these tokens consistently.

## Color Palette (Tailwind classes → CSS vars → Hex)

### Backgrounds (darkest → lightest)
| Usage | Tailwind | CSS var | Hex | Yahoo token |
|---|---|---|---|---|
| Page body | `bg-navy-950` | `--bg-primary` | `#101518` | `--yb-midnight` |
| Cards, panels | `bg-navy-900` | `--bg-surface` | `#1d2228` | `--yb-inkwell` |
| Elevated surfaces | `bg-navy-800` | `--bg-card` | `#232a31` | `--yb-batcave` |
| Hover states | `bg-navy-700` | `--bg-hover` | `#2c363f` | `--yb-ramones` |
| Borders, dividers | `border-navy-600` | `--border` | `#3a434c` | — |
| Muted/disabled | `text-navy-500` | — | `#4e5964` | — |

### Text
| Usage | Tailwind | Hex | Yahoo token |
|---|---|---|---|
| Primary (headings, values) | `text-text-primary` | `#f0f3f5` | `--yb-gray-hair` |
| Secondary (labels, captions) | `text-text-secondary` | `#b0b9c1` | `--yb-bob` |

### Semantic Colors
| Usage | Tailwind | Hex | Yahoo token |
|---|---|---|---|
| Positive / gain / up | `text-gain`, `bg-gain` | `#21d87d` | `--yb-sa-stock-up` |
| Negative / loss / down | `text-loss`, `bg-loss` | `#fc7a6e` | `--yb-sa-stock-down` |
| Gain badge background | `bg-gain/15` | — | — |
| Loss badge background | `bg-loss/15` | — | — |

### Accent Colors
| Usage | Tailwind | Hex | Yahoo token |
|---|---|---|---|
| Primary accent (buttons, active tabs) | `bg-accent-purple` | `#9d61ff` | `--yb-grape-jelly` |
| Accent hover | `hover:bg-accent-purple-dark` | `#7c3fe6` | — |
| Accent light (badges) | `text-accent-purple-light` | `#b88aff` | — |
| Links, focus rings | `text-accent-blue`, `ring-accent-blue` | `#12a9ff` | `--yb-sky` |

## Typography

**Font stack:** `"Helvetica Neue", Helvetica, Arial, sans-serif`
**Smoothing:** `-webkit-font-smoothing: antialiased`

| Element | Classes |
|---|---|
| Page title | `text-xl font-semibold text-text-primary` |
| Section heading | `text-lg font-semibold text-text-secondary` |
| Card title | `text-sm font-semibold text-text-primary` |
| Body text | `text-sm text-text-primary` |
| Label / caption | `text-xs text-text-secondary` |
| Uppercase label | `text-xs text-text-secondary uppercase tracking-wide` |
| Large value | `text-2xl font-semibold text-text-primary` |
| Badge text | `text-xs font-semibold` |

## Component Patterns

### Card
```
bg-navy-900 rounded-lg border border-navy-600 p-4
```

### Table
```
Header:   bg-navy-800 text-text-secondary uppercase text-xs
Row:      border-b border-navy-600
Hover:    hover:bg-navy-700
Dividers: divide-y divide-navy-600
```

### Button (primary)
```
bg-accent-purple text-white text-xs font-medium px-3 py-1.5 rounded-lg hover:bg-accent-purple-dark transition-colors
```

### Button (secondary/ghost)
```
text-text-secondary bg-navy-800 text-xs font-medium px-3 py-1.5 rounded-lg hover:text-text-primary hover:bg-navy-700 transition-colors
```

### Badge (gain/loss)
```
Gain: bg-gain/15 text-gain text-xs font-semibold px-2 py-0.5 rounded-full
Loss: bg-loss/15 text-loss text-xs font-semibold px-2 py-0.5 rounded-full
```

### Input / Select
Styled globally in `globals.css`:
- bg: `--bg-surface` (#1d2228)
- border: `--border` (#3a434c)
- focus: `--accent-blue` border + subtle glow
- border-radius: 8px

### Tabs (active/inactive)
```
Active:   bg-accent-purple text-white
Inactive: text-text-secondary bg-navy-800 hover:text-text-primary
```

### Nav link (active/inactive)
```
Active:   text-white border-b-2 border-accent-purple
Inactive: text-text-secondary hover:text-text-primary
```

## Recharts Styling (inline hex values)

When using recharts, apply these colors directly since Tailwind classes don't work in recharts props:

| Element | Property | Value |
|---|---|---|
| Grid lines | `stroke` | `#3a434c` |
| Axis ticks | `fill` | `#b0b9c1` |
| Axis line | `stroke` | `#3a434c` |
| Tooltip bg | `contentStyle.background` | `#232a31` |
| Tooltip border | `contentStyle.border` | `1px solid #3a434c` |
| Tooltip text | `labelStyle.color` | `#b0b9c1` |
| Gain stroke | `stroke` | `#21d87d` |
| Loss stroke | `stroke` | `#fc7a6e` |
| Accent stroke | `stroke` | `#9d61ff` |
| Volume bars | `fill` | `#3a434c` |

Always set `vertical={false}` on `CartesianGrid` and `axisLine={false} tickLine={false}` on axes.

## Layout Structure

- **Top nav** (`Navbar.tsx`): Full-width `bg-navy-900`, horizontal links, active state via `usePathname()`
- **Body**: `bg-navy-950`, full-width with `px-6 py-6`
- **No sidebar** — all navigation is in the top bar
- **Border radius**: `rounded-lg` (8px) on all cards, matching Yahoo Finance

## Directional Indicators

Use triangle SVG icons for gain/loss instead of sparkline charts:
- Up triangle (▲): `<path d="M8 4l5 8H3z" />` in gain color
- Down triangle (▼): `<path d="M8 12L3 4h10z" />` in loss color

## Files Reference

| File | Purpose |
|---|---|
| `tailwind.config.ts` | Color palette, font family |
| `app/globals.css` | CSS variables, scrollbar, input, recharts tooltip |
| `app/layout.tsx` | Root layout with Navbar, body bg |
| `components/Navbar.tsx` | Top navigation bar |
| `app/quote/[symbol]/page.tsx` | Yahoo Finance-style quote detail page |
| `components/WatchlistTable.tsx` | Compact watchlist table (overview page) |

## Rules

1. NEVER use raw gray Tailwind classes (`text-gray-100`, `bg-gray-50`, etc.) — always use the named tokens (`text-text-primary`, `bg-navy-900`, etc.)
2. NEVER use light backgrounds (`bg-white`, `bg-gray-*`) — this is a dark-only theme
3. All cards use `bg-navy-900` with `border border-navy-600` and `rounded-lg`
4. Gain/loss colors must use `text-gain`/`text-loss` (the softer dark-mode variants), never raw green/red
5. Focus states use `accent-blue` (ring/border), not purple
6. Recharts components need inline hex colors — they cannot use Tailwind classes
7. New components must include `"use client"` directive
