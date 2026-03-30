Run a full system health check across all three AlgoTrader services. Report results in a clear summary.

## What to check

### 1. Service Status
Check if each service is listening on its port:
- Strategy Engine: port 9100 (`lsof -i :9100 -sTCP:LISTEN`)
- Execution Engine: port 9101 (`lsof -i :9101 -sTCP:LISTEN`)
- Dashboard: port 9102 (use `ss -tlnp | grep :9102` since Next.js may not show in lsof)

Report PID for running services, "DOWN" for stopped ones.

### 2. API Health (only for running services)

**Execution Engine** (`http://localhost:9101`):
- `GET /account` — show equity, mode (paper/live), trading_blocked, status
- `GET /positions` — count positions, break down by side (long/short) and trade_type (day/swing)
- `GET /risk/config` — show all risk parameters in a compact line

**Strategy Engine** (`http://localhost:9100`):
- `GET /symbols` — show count and list
- `GET /strategies` — count total and enabled strategies

### 3. Recent Errors
Scan the last 50 lines of each log file for ERROR or WARN entries:
- `/tmp/execution-engine.log`
- `/tmp/strategy-engine.log`

Show the count of errors/warnings in the last hour. If there are any, show the 3 most recent.

## Output format

Use this structure:

```
=== AlgoTrader Health Check ===

Services:
  Strategy Engine (9100):  Running (PID xxxxx)
  Execution Engine (9101): Running (PID xxxxx)
  Dashboard (9102):        Running (PID xxxxx)

Account:
  Status: ACTIVE | Mode: paper | Equity: $xxx,xxx.xx | Blocked: false

Positions:
  X total (Y long, Z short | A day, B swing)

Risk Config:
  Daily loss: X% | Pos size: X% | Max positions: X | Min confidence: X | Throttle: Xs | EOD: HH:MM

Strategies:
  X/Y enabled

Symbols:
  X tracked: SYM1, SYM2, ...

Recent Errors (last 1h):
  Execution: X errors, Y warnings
  Strategy: X errors, Y warnings
  [most recent errors if any]
```

## Rules
- Run all checks using Bash tool with curl and shell commands
- Use `python3 -m json.tool` or inline python for JSON parsing
- If a service is down, skip its API checks and note it as unreachable
- Keep output concise — this is a quick status check, not a deep dive
- Do NOT attempt to fix any issues found — just report them
