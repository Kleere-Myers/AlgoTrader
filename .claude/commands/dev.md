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
  - Start: cd strategy-engine && ../.venv/bin/uvicorn main:app --port 8000 &
  - Port: 8000
  - Process pattern: "uvicorn main:app --port 8000"

Execution Engine (Rust Axum):
  - Directory: execution-engine/
  - Start: cd execution-engine && PATH="$HOME/.cargo/bin:$PATH" cargo run &
  - Port: 8080
  - Process pattern: "target/debug/execution-engine"

Dashboard (Next.js):
  - Directory: dashboard/
  - Start: cd dashboard && npm run dev &
  - Port: 3000
  - Process pattern: "next dev --port 3000"

## Behavior

**start [service]**
Check if the target service(s) are already running (check the port with `lsof -i :PORT`). If already running, say so and skip. Otherwise start the service in the background. Wait a few seconds and verify the port is listening. Report success or failure for each service.

**stop [service]**
Find processes using the service port(s) with `lsof -ti :PORT` and kill them. Confirm each service stopped.

**restart [service]**
Stop then start the target service(s).

**status**
For each of the 3 services, check if the port is in use with `lsof -i :PORT`. Report running/stopped for each, and show the PID if running.

Always report results clearly, one line per service.
