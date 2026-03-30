#!/usr/bin/env bash
#
# AlgoTrader Watchdog — runs via cron every 5 minutes
#
# Checks:
#   1. Are services listening on their ports?
#   2. During market hours: is the execution engine receiving bars?
#   3. Restarts any service that fails checks
#
# Install:
#   crontab -e
#   */5 * * * * /home/mmyers/Projects/AlgoTrader/scripts/watchdog.sh >> /tmp/watchdog.log 2>&1

set -euo pipefail

PROJECT_DIR="/home/mmyers/Projects/AlgoTrader"
LOG_PREFIX="[$(date '+%Y-%m-%d %H:%M:%S')]"

# Market hours in ET (9:30 AM - 4:00 PM)
is_market_hours() {
    local et_hour et_min
    et_hour=$(TZ="America/New_York" date '+%-H')
    et_min=$(TZ="America/New_York" date '+%-M')
    local mins=$(( et_hour * 60 + et_min ))
    # 9:35 AM = 575 min, 4:00 PM = 960 min
    [[ "$mins" -ge 575 && "$mins" -le 960 ]]
}

# Check if a port is listening
is_listening() {
    lsof -i ":$1" -sTCP:LISTEN >/dev/null 2>&1 || ss -tlnp 2>/dev/null | grep -q ":$1 "
}

# Check if execution engine received a bar in the last 10 minutes
bars_flowing() {
    local last_bar
    last_bar=$(grep "Received bar" /tmp/execution-engine.log 2>/dev/null | tail -1 | grep -oP '\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}' || echo "")
    if [[ -z "$last_bar" ]]; then
        return 1
    fi
    local last_epoch
    last_epoch=$(date -d "$last_bar" +%s 2>/dev/null || echo 0)
    local now_epoch
    now_epoch=$(date +%s)
    local age=$(( now_epoch - last_epoch ))
    # Bars come every 5 min; allow 10 min grace
    [[ "$age" -lt 600 ]]
}

start_strategy() {
    echo "$LOG_PREFIX Starting strategy engine..."
    cd "$PROJECT_DIR" && set -a && source .env && set +a
    cd strategy-engine && .venv/bin/uvicorn main:app --port 9100 > /tmp/strategy-engine.log 2>&1 &
    sleep 3
    if is_listening 9100; then
        echo "$LOG_PREFIX Strategy engine started (PID $!)"
    else
        echo "$LOG_PREFIX FAILED to start strategy engine"
    fi
}

start_execution() {
    echo "$LOG_PREFIX Starting execution engine..."
    cd "$PROJECT_DIR/execution-engine"
    /home/mmyers/.cargo/bin/cargo run > /tmp/execution-engine.log 2>&1 &
    local pid=$!
    # Wait up to 30s for port
    for i in $(seq 1 15); do
        is_listening 9101 && break
        sleep 2
    done
    if is_listening 9101; then
        echo "$LOG_PREFIX Execution engine started (PID $pid)"
        # Re-apply risk config
        sleep 2
        curl -s -X PATCH http://localhost:9101/risk/config \
            -H "Content-Type: application/json" \
            -d '{"max_daily_loss_pct": 0.05, "max_position_size_pct": 0.20, "max_open_positions": 8, "min_signal_confidence": 0.50, "order_throttle_secs": 120}' > /dev/null
        echo "$LOG_PREFIX Risk config applied"
    else
        echo "$LOG_PREFIX FAILED to start execution engine"
    fi
}

start_dashboard() {
    echo "$LOG_PREFIX Starting dashboard..."
    cd "$PROJECT_DIR/dashboard" && npm run dev > /tmp/dashboard.log 2>&1 &
    sleep 3
    if is_listening 9102; then
        echo "$LOG_PREFIX Dashboard started"
    else
        echo "$LOG_PREFIX FAILED to start dashboard"
    fi
}

restart_execution() {
    echo "$LOG_PREFIX Stopping execution engine..."
    lsof -ti :9101 2>/dev/null | xargs -r kill 2>/dev/null
    sleep 2
    start_execution
}

# --- Main checks ---

issues=0

# 1. Strategy engine
if ! is_listening 9100; then
    echo "$LOG_PREFIX Strategy engine DOWN — restarting"
    start_strategy
    issues=$((issues + 1))
fi

# 2. Execution engine
if ! is_listening 9101; then
    echo "$LOG_PREFIX Execution engine DOWN — restarting"
    start_execution
    issues=$((issues + 1))
elif is_market_hours && ! bars_flowing; then
    echo "$LOG_PREFIX Execution engine UP but no bars in 10 min during market hours — restarting"
    restart_execution
    issues=$((issues + 1))
fi

# 3. Dashboard
if ! is_listening 9102; then
    echo "$LOG_PREFIX Dashboard DOWN — restarting"
    start_dashboard
    issues=$((issues + 1))
fi

# Only log "all healthy" during market hours to keep logs clean
if [[ "$issues" -eq 0 ]] && is_market_hours; then
    echo "$LOG_PREFIX All services healthy"
fi
