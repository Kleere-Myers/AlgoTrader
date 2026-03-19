Manage AlgoTrader dev services. Parse the argument to determine the action and target.

Usage: /dev <action> [service]

Actions: start, stop, restart, status
Services: all (default), strategy, execution, dashboard

Examples:
  /dev start          → start all 3 services
  /dev stop dashboard → stop only the dashboard
  /dev restart        → restart all 3 services
  /dev status         → show which services are running

## Service definitions

Strategy Engine (Python FastAPI):
  - Directory: strategy-engine/
  - Start: cd /home/mmyers/Projects/AlgoTrader && set -a && source .env && set +a && cd strategy-engine && .venv/bin/uvicorn main:app --port 9100 > /tmp/strategy-engine.log 2>&1 &
  - Port: 9100
  - Process pattern: "uvicorn main:app --port 9100"
  - Log file: /tmp/strategy-engine.log
  - NOTE: Must source .env before starting so Alpaca API keys are available for news/market endpoints

Execution Engine (Rust Axum):
  - Directory: execution-engine/
  - Start: cd execution-engine && /home/mmyers/.cargo/bin/cargo run > /tmp/execution-engine.log 2>&1 &
  - Port: 9101
  - Process pattern: "target/debug/execution-engine"
  - Log file: /tmp/execution-engine.log

Post-start hooks (after BOTH strategy + execution engines are up):
  - Re-apply aggressive risk settings via PATCH /risk/config (check memory for values)

Dashboard (Next.js):
  - Directory: dashboard/
  - Start: cd /home/mmyers/Projects/AlgoTrader/dashboard && npm run dev > /tmp/dashboard.log 2>&1 &
  - Port: 9102
  - Process pattern: "next dev --port 9102"
  - Log file: /tmp/dashboard.log

## Behavior

**start [service]**
Check if the target service(s) are already running (check the port with `lsof -i :PORT`). If already running, say so and skip. Otherwise start the service in the background. Wait a few seconds and verify the port is listening. Report success or failure for each service.

**stop [service]**
Find processes using the service port(s) with `lsof -ti :PORT` and kill them. Also check `ss -tlnp | grep :PORT` for lingering processes (especially Next.js which may leave orphaned `next-server` processes). Confirm each service stopped.

**restart [service]**
Stop then start the target service(s).

**status**
For each of the 3 services, check if the port is in use with `lsof -i :PORT`. Report running/stopped for each, and show the PID if running.

Always report results clearly, one line per service.

**Post-start/restart hook:**
After the execution engine starts, re-apply the user's preferred risk settings by running:
```
curl -s -X PATCH http://localhost:9101/risk/config \
  -H "Content-Type: application/json" \
  -d '{"max_daily_loss_pct": 0.05, "max_position_size_pct": 0.20, "max_open_positions": 8, "min_signal_confidence": 0.50, "order_throttle_secs": 120}'
```
Report the applied risk config in the status output.
