use std::sync::Arc;

use chrono::{Datelike, NaiveDate, NaiveTime, Utc, Weekday};
use chrono_tz::America::New_York;
use tracing::{error, info, warn};

use crate::db;
use crate::models::{Order, SseEvent, SseEventType, SwingSignalRequest, SwingSignalResponse, TradeType};
use crate::risk::SwingRiskContext;
use crate::AppState;

/// Hardcoded NYSE holidays for 2026 (v1 — no external calendar API).
/// Dates when the market is fully closed.
const NYSE_HOLIDAYS_2026: &[(u32, u32)] = &[
    (1, 1),   // New Year's Day
    (1, 19),  // MLK Day
    (2, 16),  // Presidents' Day
    (4, 3),   // Good Friday
    (5, 25),  // Memorial Day
    (7, 3),   // Independence Day (observed)
    (9, 7),   // Labor Day
    (11, 26), // Thanksgiving
    (12, 25), // Christmas
];

/// Returns true if the given date is a trading day (not weekend, not NYSE holiday).
pub fn is_trading_day(date: NaiveDate) -> bool {
    let weekday = date.weekday();
    if weekday == Weekday::Sat || weekday == Weekday::Sun {
        return false;
    }
    let month = date.month();
    let day = date.day();
    !NYSE_HOLIDAYS_2026.iter().any(|&(m, d)| m == month && d == day)
}

/// Returns true if the current ET time has reached or passed 15:45.
pub fn should_flatten(date: NaiveDate, time: NaiveTime) -> bool {
    if !is_trading_day(date) {
        return false;
    }
    let flatten_time = NaiveTime::from_hms_opt(15, 45, 0).unwrap();
    time >= flatten_time
}

/// Background task: checks every 30 seconds and flattens all positions at 15:45 ET.
pub async fn eod_flatten_loop(state: Arc<AppState>) {
    let mut flattened_today: Option<NaiveDate> = None;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let now_et = Utc::now().with_timezone(&New_York);
        let today = now_et.date_naive();
        let current_time = now_et.time();

        // Already flattened today — skip until next trading day
        if flattened_today == Some(today) {
            continue;
        }

        if !should_flatten(today, current_time) {
            continue;
        }

        info!("EOD auto-flatten triggered at {now_et}");
        flattened_today = Some(today);

        // Get only day-trading positions (swing positions survive overnight)
        let positions = {
            let tracker = state.positions.lock().await;
            tracker.day_positions()
        };

        if positions.is_empty() {
            info!("No open positions to flatten");
            continue;
        }

        for pos in &positions {
            info!(
                symbol = %pos.symbol,
                qty = pos.qty,
                reason = "EOD_AUTO_FLATTEN",
                "Flattening position"
            );

            let qty = pos.qty;
            let symbol = &pos.symbol;

            match state.alpaca.submit_market_order(symbol, qty, "sell").await {
                Ok(alpaca_order) => {
                    let now = chrono::Utc::now().to_rfc3339();
                    let order = Order {
                        order_id: uuid::Uuid::new_v4().to_string(),
                        alpaca_id: Some(alpaca_order.id.clone()),
                        symbol: symbol.clone(),
                        side: "sell".to_string(),
                        qty,
                        filled_price: alpaca_order
                            .filled_avg_price
                            .as_ref()
                            .and_then(|p| p.parse::<f64>().ok()),
                        status: alpaca_order.status.clone(),
                        strategy_name: "EOD_AUTO_FLATTEN".to_string(),
                        created_at: now,
                        filled_at: alpaca_order.filled_at.clone(),
                        trade_type: crate::models::TradeType::Day,
                    };

                    if let Ok(con) = db::connect() {
                        if let Err(e) = db::insert_order(&con, &order) {
                            error!("Failed to insert flatten order: {e}");
                        }
                    }

                    state.risk_engine.lock().await.record_order(symbol);

                    // Poll for fill
                    let fill_price = crate::poll_for_fill(
                        &state,
                        &alpaca_order.id,
                        &order.order_id,
                        "sell",
                        qty,
                        symbol,
                    )
                    .await;

                    // Update position tracker
                    {
                        let mut tracker = state.positions.lock().await;
                        let updated = tracker.update_on_fill(symbol, "sell", qty, fill_price.unwrap_or(0.0), crate::models::TradeType::Day, None, None);
                        if let Ok(con) = db::connect() {
                            match updated {
                                Some(ref p) => {
                                    let _ = db::upsert_position(&con, p);
                                }
                                None => {
                                    let _ = db::delete_position(&con, symbol);
                                }
                            }
                        }
                    }

                    // Broadcast POSITION_UPDATE for each closed position
                    state.broadcaster.send(SseEvent {
                        event_type: SseEventType::PositionUpdate,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        payload: serde_json::json!({
                            "symbol": symbol,
                            "action": "EOD_AUTO_FLATTEN",
                            "qty_sold": qty,
                            "fill_price": fill_price,
                        }),
                    });

                    info!(
                        symbol,
                        qty,
                        fill_price,
                        reason = "EOD_AUTO_FLATTEN",
                        "Position flattened"
                    );
                }
                Err(e) => {
                    error!(
                        symbol,
                        reason = "EOD_AUTO_FLATTEN",
                        "Failed to submit flatten order: {e}"
                    );
                }
            }
        }

        info!(
            count = positions.len(),
            "EOD auto-flatten complete"
        );
    }
}

/// Returns true if current ET time is within market hours (9:30-16:00).
fn is_market_hours(time: NaiveTime) -> bool {
    let open = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
    let close = NaiveTime::from_hms_opt(16, 0, 0).unwrap();
    time >= open && time <= close
}

/// Background task: after market close (4:05 PM ET), fetch daily bars and generate swing signals.
pub async fn daily_swing_signal_loop(state: Arc<AppState>) {
    let mut triggered_today: Option<NaiveDate> = None;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let now_et = Utc::now().with_timezone(&New_York);
        let today = now_et.date_naive();
        let current_time = now_et.time();

        // Already triggered today
        if triggered_today == Some(today) {
            continue;
        }

        if !is_trading_day(today) {
            continue;
        }

        // Trigger at 16:05 ET (5 minutes after close to ensure daily bars are finalized)
        let trigger_time = NaiveTime::from_hms_opt(16, 5, 0).unwrap();
        if current_time < trigger_time {
            continue;
        }

        info!("Daily swing signal scan triggered at {now_et}");
        triggered_today = Some(today);

        let http = reqwest::Client::new();

        for symbol in &state.symbols {
            // Fetch daily bars from Alpaca (last 150 for weekly EMA lookback)
            let bars = match state.alpaca.get_daily_bars(symbol, 150).await {
                Ok(b) => b,
                Err(e) => {
                    error!(symbol, "Failed to fetch daily bars from Alpaca: {e}");
                    continue;
                }
            };

            if bars.is_empty() {
                warn!(symbol, "No daily bars returned from Alpaca");
                continue;
            }

            // Store daily bars in DuckDB
            if let Ok(con) = db::connect() {
                for bar in &bars {
                    if let Err(e) = db::upsert_bar(&con, bar, "1d") {
                        error!(symbol, "Failed to upsert daily bar: {e}");
                    }
                }
            }

            // Call strategy engine POST /signal/swing
            let req = SwingSignalRequest {
                symbol: symbol.clone(),
                bars_daily: bars,
            };

            let url = format!("{}/signal/swing", state.strategy_engine_url);
            let resp = match http.post(&url).json(&req).send().await {
                Ok(r) => r,
                Err(e) => {
                    error!(symbol, "Failed to call swing signal endpoint: {e}");
                    continue;
                }
            };

            let swing_resp: SwingSignalResponse = match resp.json().await {
                Ok(r) => r,
                Err(e) => {
                    error!(symbol, "Failed to parse swing signal response: {e}");
                    continue;
                }
            };

            let composite = swing_resp.composite;
            info!(
                symbol,
                direction = ?composite.direction,
                confidence = composite.confidence,
                reason = %composite.reason,
                "Swing composite signal"
            );

            // Skip HOLD signals
            if composite.direction == crate::models::Direction::Hold {
                continue;
            }

            // Evaluate through swing risk engine
            let risk_decision = {
                let engine = state.risk_engine.lock().await;
                let positions = state.positions.lock().await;
                let equity = *state.account_equity.lock().await;
                let daily_pnl = *state.daily_pnl.lock().await;
                let halted = *state.trading_halted.lock().await;

                let swing_positions = positions.swing_positions();
                let current_heat: f64 = swing_positions.iter().map(|p| {
                    if let (Some(sl), _) = (p.stop_loss_price, p.take_profit_price) {
                        let distance = (p.avg_entry_price - sl).abs() / p.avg_entry_price;
                        (p.qty * p.avg_entry_price * distance) / equity
                    } else {
                        engine.swing_config.per_position_stop_loss_pct
                            * (p.qty * p.avg_entry_price) / equity
                    }
                }).sum();

                let ctx = SwingRiskContext {
                    trading_halted: halted,
                    account_equity: equity,
                    daily_loss: daily_pnl,
                    swing_position_count: swing_positions.len(),
                    current_portfolio_heat: current_heat,
                    position_value_for_symbol: positions.position_value(symbol, composite.confidence * equity * engine.swing_config.per_position_stop_loss_pct / engine.swing_config.per_position_stop_loss_pct),
                };

                engine.evaluate_swing(&composite, &ctx)
            };

            match risk_decision {
                crate::risk::RiskDecision::Approved => {
                    let side = match composite.direction {
                        crate::models::Direction::Buy => "buy",
                        crate::models::Direction::Sell => "sell",
                        _ => continue,
                    };

                    // Calculate position size: use max_position_size_pct of equity
                    let equity = *state.account_equity.lock().await;
                    let engine = state.risk_engine.lock().await;
                    let position_value = equity * engine.config.max_position_size_pct;
                    // Get last close price from the signal context
                    drop(engine);

                    // Fetch current price for qty calculation
                    let current_price = match state.alpaca.get_daily_bars(symbol, 1).await {
                        Ok(bars) if !bars.is_empty() => bars.last().unwrap().close,
                        _ => {
                            warn!(symbol, "Cannot determine price for swing order sizing");
                            continue;
                        }
                    };

                    let qty = (position_value / current_price).floor();
                    if qty < 1.0 {
                        warn!(symbol, "Calculated qty < 1, skipping");
                        continue;
                    }

                    // Calculate stop/take prices
                    let (stop_loss, take_profit) = {
                        let engine = state.risk_engine.lock().await;
                        engine.swing_stop_take(current_price, &composite.direction)
                    };

                    info!(
                        symbol,
                        side,
                        qty,
                        stop_loss,
                        take_profit,
                        "Submitting swing order"
                    );

                    match state.alpaca.submit_market_order(symbol, qty, side).await {
                        Ok(alpaca_order) => {
                            let now = chrono::Utc::now().to_rfc3339();
                            let order = Order {
                                order_id: uuid::Uuid::new_v4().to_string(),
                                alpaca_id: Some(alpaca_order.id.clone()),
                                symbol: symbol.clone(),
                                side: side.to_string(),
                                qty,
                                filled_price: alpaca_order.filled_avg_price.as_ref().and_then(|p| p.parse::<f64>().ok()),
                                status: alpaca_order.status.clone(),
                                strategy_name: composite.strategy_name.clone(),
                                created_at: now,
                                filled_at: alpaca_order.filled_at.clone(),
                                trade_type: TradeType::Swing,
                            };

                            if let Ok(con) = db::connect() {
                                if let Err(e) = db::insert_order(&con, &order) {
                                    error!("Failed to insert swing order: {e}");
                                }
                            }

                            state.risk_engine.lock().await.record_order(symbol);

                            let fill_price = crate::poll_for_fill(
                                &state, &alpaca_order.id, &order.order_id, side, qty, symbol,
                            ).await;

                            if let Some(fp) = fill_price {
                                let mut tracker = state.positions.lock().await;
                                let pos = tracker.update_on_fill(
                                    symbol, side, qty, fp,
                                    TradeType::Swing,
                                    Some(stop_loss),
                                    Some(take_profit),
                                );
                                if let Ok(con) = db::connect() {
                                    match pos {
                                        Some(ref p) => { let _ = db::upsert_position(&con, p); }
                                        None => { let _ = db::delete_position(&con, symbol); }
                                    }
                                }
                            }

                            state.broadcaster.send(SseEvent {
                                event_type: SseEventType::OrderFill,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                payload: serde_json::json!({
                                    "symbol": symbol,
                                    "side": side,
                                    "qty": qty,
                                    "trade_type": "swing",
                                    "stop_loss": stop_loss,
                                    "take_profit": take_profit,
                                }),
                            });
                        }
                        Err(e) => {
                            error!(symbol, "Swing order submission failed: {e}");
                        }
                    }
                }
                crate::risk::RiskDecision::Rejected(reason) => {
                    warn!(symbol, reason, "Swing signal rejected by risk engine");
                }
                crate::risk::RiskDecision::HaltAll(reason) => {
                    error!(reason, "Daily loss limit breached during swing evaluation");
                    *state.trading_halted.lock().await = true;
                }
            }
        }

        info!("Daily swing signal scan complete");
    }
}

/// Background task: every 60 seconds during market hours, check swing positions
/// against their stop-loss and take-profit levels.
pub async fn swing_stop_check_loop(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        let now_et = Utc::now().with_timezone(&New_York);
        let today = now_et.date_naive();
        let current_time = now_et.time();

        if !is_trading_day(today) || !is_market_hours(current_time) {
            continue;
        }

        // Get swing positions with stop/take levels
        let swing_positions = {
            let tracker = state.positions.lock().await;
            tracker.swing_positions()
        };

        if swing_positions.is_empty() {
            continue;
        }

        for pos in &swing_positions {
            let stop_loss = match pos.stop_loss_price {
                Some(sl) => sl,
                None => continue,
            };
            let take_profit = match pos.take_profit_price {
                Some(tp) => tp,
                None => continue,
            };

            // Get current price from Alpaca
            let current_price = match state.alpaca.get_daily_bars(&pos.symbol, 1).await {
                Ok(bars) if !bars.is_empty() => bars.last().unwrap().close,
                _ => continue,
            };

            let hit_stop = current_price <= stop_loss;
            let hit_take = current_price >= take_profit;

            if !hit_stop && !hit_take {
                continue;
            }

            let reason = if hit_stop { "STOP_LOSS" } else { "TAKE_PROFIT" };
            info!(
                symbol = %pos.symbol,
                current_price,
                stop_loss,
                take_profit,
                reason,
                "Swing position exit triggered"
            );

            // Close position
            match state.alpaca.submit_market_order(&pos.symbol, pos.qty, "sell").await {
                Ok(alpaca_order) => {
                    let now = chrono::Utc::now().to_rfc3339();
                    let order = Order {
                        order_id: uuid::Uuid::new_v4().to_string(),
                        alpaca_id: Some(alpaca_order.id.clone()),
                        symbol: pos.symbol.clone(),
                        side: "sell".to_string(),
                        qty: pos.qty,
                        filled_price: alpaca_order.filled_avg_price.as_ref().and_then(|p| p.parse::<f64>().ok()),
                        status: alpaca_order.status.clone(),
                        strategy_name: format!("SWING_{reason}"),
                        created_at: now,
                        filled_at: alpaca_order.filled_at.clone(),
                        trade_type: TradeType::Swing,
                    };

                    if let Ok(con) = db::connect() {
                        if let Err(e) = db::insert_order(&con, &order) {
                            error!("Failed to insert swing exit order: {e}");
                        }
                    }

                    let fill_price = crate::poll_for_fill(
                        &state, &alpaca_order.id, &order.order_id, "sell", pos.qty, &pos.symbol,
                    ).await;

                    {
                        let mut tracker = state.positions.lock().await;
                        let updated = tracker.update_on_fill(
                            &pos.symbol, "sell", pos.qty, fill_price.unwrap_or(0.0),
                            TradeType::Swing, None, None,
                        );
                        if let Ok(con) = db::connect() {
                            match updated {
                                Some(ref p) => { let _ = db::upsert_position(&con, p); }
                                None => { let _ = db::delete_position(&con, &pos.symbol); }
                            }
                        }
                    }

                    state.broadcaster.send(SseEvent {
                        event_type: SseEventType::PositionUpdate,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        payload: serde_json::json!({
                            "symbol": pos.symbol,
                            "action": reason,
                            "trade_type": "swing",
                            "qty_sold": pos.qty,
                            "fill_price": fill_price,
                        }),
                    });

                    info!(
                        symbol = %pos.symbol,
                        reason,
                        fill_price,
                        "Swing position closed"
                    );
                }
                Err(e) => {
                    error!(symbol = %pos.symbol, "Failed to submit swing exit order: {e}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_flatten_fires_at_345pm_et_on_trading_day() {
        // Monday March 16, 2026 is a trading day
        let date = NaiveDate::from_ymd_opt(2026, 3, 16).unwrap();
        assert!(is_trading_day(date));

        // Before 15:45 — should not flatten
        let before = NaiveTime::from_hms_opt(15, 44, 59).unwrap();
        assert!(!should_flatten(date, before));

        // Exactly 15:45 — should flatten
        let at = NaiveTime::from_hms_opt(15, 45, 0).unwrap();
        assert!(should_flatten(date, at));

        // After 15:45 — should flatten
        let after = NaiveTime::from_hms_opt(15, 50, 0).unwrap();
        assert!(should_flatten(date, after));
    }

    #[test]
    fn test_flatten_skips_weekends() {
        // Saturday March 14, 2026
        let saturday = NaiveDate::from_ymd_opt(2026, 3, 14).unwrap();
        assert_eq!(saturday.weekday(), Weekday::Sat);
        assert!(!is_trading_day(saturday));
        let time = NaiveTime::from_hms_opt(15, 45, 0).unwrap();
        assert!(!should_flatten(saturday, time));

        // Sunday March 15, 2026
        let sunday = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
        assert_eq!(sunday.weekday(), Weekday::Sun);
        assert!(!is_trading_day(sunday));
        assert!(!should_flatten(sunday, time));
    }

    #[test]
    fn test_flatten_skips_nyse_holidays() {
        // Christmas 2026 is a Friday
        let christmas = NaiveDate::from_ymd_opt(2026, 12, 25).unwrap();
        assert_eq!(christmas.weekday(), Weekday::Fri);
        assert!(!is_trading_day(christmas));
        let time = NaiveTime::from_hms_opt(15, 45, 0).unwrap();
        assert!(!should_flatten(christmas, time));
    }

    #[test]
    fn test_regular_weekday_is_trading_day() {
        // Tuesday March 17, 2026
        let tuesday = NaiveDate::from_ymd_opt(2026, 3, 17).unwrap();
        assert_eq!(tuesday.weekday(), Weekday::Tue);
        assert!(is_trading_day(tuesday));
    }

    #[test]
    fn test_early_morning_does_not_trigger() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 16).unwrap();
        let morning = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
        assert!(!should_flatten(date, morning));
    }
}
