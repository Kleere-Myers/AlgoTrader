use std::collections::HashMap;
use std::time::Instant;

use crate::models::{Direction, Signal};

/// Risk configuration — defaults match PRD Section 6.2.
pub struct RiskConfig {
    pub max_daily_loss_pct: f64,
    pub max_position_size_pct: f64,
    pub max_open_positions: usize,
    pub min_signal_confidence: f64,
    pub order_throttle_secs: u64,
    pub day_stop_loss_pct: f64,
    pub day_take_profit_pct: f64,
    pub regime_filter_enabled: bool,
    pub regime_filter_threshold_pct: f64,
    pub max_net_exposure_pct: f64,
    pub max_positions_per_strategy: usize,
    /// Tier 1 daily loss threshold — reduce position sizes and max positions by 50%.
    pub daily_loss_tier1_pct: f64,
    /// Tier 2 daily loss threshold — block all new position entries.
    pub daily_loss_tier2_pct: f64,
    /// Daily profit target — flatten all day positions and halt new entries when hit.
    /// 0.0 means disabled.
    pub daily_profit_target_pct: f64,
    /// Boosted net exposure cap when the regime filter confirms the direction.
    /// Only applies to the favored side (longs in uptrend, shorts in downtrend).
    /// The opposite side stays at max_net_exposure_pct.
    pub regime_boosted_exposure_pct: f64,
}

/// Swing-specific risk configuration.
pub struct SwingRiskConfig {
    pub max_swing_positions: usize,
    pub max_portfolio_heat_pct: f64,
    pub per_position_stop_loss_pct: f64,
    pub per_position_take_profit_pct: f64,
    pub min_composite_confidence: f64,
}

impl Default for SwingRiskConfig {
    fn default() -> Self {
        Self {
            max_swing_positions: 6,
            max_portfolio_heat_pct: 0.06,
            per_position_stop_loss_pct: 0.05,
            per_position_take_profit_pct: 0.15,
            min_composite_confidence: 0.65,
        }
    }
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_daily_loss_pct: 0.02,
            max_position_size_pct: 0.10,
            max_open_positions: 4,
            min_signal_confidence: 0.60,
            order_throttle_secs: 300,
            day_stop_loss_pct: 0.01,
            day_take_profit_pct: 0.03,
            regime_filter_enabled: true,
            regime_filter_threshold_pct: 0.01,
            max_net_exposure_pct: 0.40,
            max_positions_per_strategy: 2,
            daily_loss_tier1_pct: 0.02,
            daily_loss_tier2_pct: 0.03,
            daily_profit_target_pct: 0.0,
            regime_boosted_exposure_pct: 0.70,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RiskDecision {
    Approved,
    Rejected(String),
    HaltAll(String),
}

/// Snapshot of current trading state needed for risk evaluation.
pub struct RiskContext {
    pub trading_halted: bool,
    pub account_equity: f64,
    pub daily_loss: f64,
    pub open_position_count: usize,
    pub position_value_for_symbol: f64,
    pub spy_day_change_pct: f64,
    /// Total long dollar exposure across all positions.
    pub net_long_exposure: f64,
    /// Total short dollar exposure across all positions.
    pub net_short_exposure: f64,
    /// Number of open positions for the signal's strategy.
    pub strategy_position_count: usize,
    /// True if the daily profit target has already been hit today.
    pub profit_target_hit: bool,
}

/// Context for swing risk evaluation.
pub struct SwingRiskContext {
    pub trading_halted: bool,
    pub account_equity: f64,
    pub daily_loss: f64,
    pub swing_position_count: usize,
    /// Sum of (stop_loss_distance * position_size / equity) across all swing positions.
    pub current_portfolio_heat: f64,
    pub position_value_for_symbol: f64,
}

/// Risk engine that enforces all 8 checks in order.
pub struct RiskEngine {
    pub config: RiskConfig,
    pub swing_config: SwingRiskConfig,
    last_order_time: HashMap<String, Instant>,
}

impl RiskEngine {
    pub fn new(config: RiskConfig) -> Self {
        Self {
            config,
            swing_config: SwingRiskConfig::default(),
            last_order_time: HashMap::new(),
        }
    }

    /// Evaluate a signal against all risk rules. Returns a RiskDecision.
    /// Checks run in the order specified in AGENT_EXECUTION.md.
    pub fn evaluate(&self, signal: &Signal, ctx: &RiskContext) -> RiskDecision {
        // 1. Is trading currently halted?
        if ctx.trading_halted {
            return RiskDecision::Rejected("Trading is halted".into());
        }

        // 2. Tiered daily loss response
        let loss_pct = if ctx.account_equity > 0.0 && ctx.daily_loss < 0.0 {
            ctx.daily_loss.abs() / ctx.account_equity
        } else {
            0.0
        };

        // Tier 3: full halt (existing behavior)
        if loss_pct >= self.config.max_daily_loss_pct {
            return RiskDecision::HaltAll(format!(
                "Daily loss {:.2}% exceeds limit {:.2}%",
                loss_pct * 100.0,
                self.config.max_daily_loss_pct * 100.0,
            ));
        }

        // Tier 2: block all new entries (existing positions managed by stops)
        let tier2_active = loss_pct >= self.config.daily_loss_tier2_pct;
        if tier2_active && signal.direction != Direction::Hold {
            return RiskDecision::Rejected(format!(
                "Daily loss tier 2: {:.2}% loss (>= {:.1}% threshold), blocking new entries",
                loss_pct * 100.0,
                self.config.daily_loss_tier2_pct * 100.0,
            ));
        }

        // Profit target: block new entries after target was hit and positions flattened
        if ctx.profit_target_hit && signal.direction != Direction::Hold {
            return RiskDecision::Rejected(
                "Daily profit target already hit — no new entries until next trading day".into(),
            );
        }

        // Tier 1: reduce effective limits by 50%
        let tier1_active = loss_pct >= self.config.daily_loss_tier1_pct;
        let effective_position_size_pct = if tier1_active {
            self.config.max_position_size_pct * 0.5
        } else {
            self.config.max_position_size_pct
        };
        let effective_max_positions = if tier1_active {
            (self.config.max_open_positions / 2).max(1)
        } else {
            self.config.max_open_positions
        };

        // 3. Is signal confidence >= min_signal_confidence?
        if signal.confidence < self.config.min_signal_confidence {
            return RiskDecision::Rejected(format!(
                "Signal confidence {:.2} below minimum {:.2}",
                signal.confidence, self.config.min_signal_confidence,
            ));
        }

        // 4. Market regime filter — suppress shorts in strong uptrends and longs in strong downtrends
        if self.config.regime_filter_enabled {
            let threshold = self.config.regime_filter_threshold_pct;
            if signal.direction == Direction::Sell && ctx.spy_day_change_pct > threshold {
                return RiskDecision::Rejected(format!(
                    "Market regime filter: SPY up {:.2}% today, suppressing shorts (threshold {:.1}%)",
                    ctx.spy_day_change_pct * 100.0,
                    threshold * 100.0,
                ));
            }
            if signal.direction == Direction::Buy && ctx.spy_day_change_pct < -threshold {
                return RiskDecision::Rejected(format!(
                    "Market regime filter: SPY down {:.2}% today, suppressing longs (threshold {:.1}%)",
                    ctx.spy_day_change_pct * 100.0,
                    threshold * 100.0,
                ));
            }
        }

        // 5. Net exposure cap — prevent excessive directional exposure
        //    When the regime filter is active and confirms this direction, use the
        //    boosted cap for the favored side. The opposite side keeps the base cap
        //    (though the regime filter already blocks it in check 4).
        if ctx.account_equity > 0.0 {
            let threshold = self.config.regime_filter_threshold_pct;
            let regime_bullish = self.config.regime_filter_enabled && ctx.spy_day_change_pct > threshold;
            let regime_bearish = self.config.regime_filter_enabled && ctx.spy_day_change_pct < -threshold;

            let long_cap = if regime_bullish {
                self.config.regime_boosted_exposure_pct
            } else {
                self.config.max_net_exposure_pct
            };
            let short_cap = if regime_bearish {
                self.config.regime_boosted_exposure_pct
            } else {
                self.config.max_net_exposure_pct
            };

            let (add_long, add_short) = match signal.direction {
                Direction::Buy => (ctx.position_value_for_symbol, 0.0),
                Direction::Sell => (0.0, ctx.position_value_for_symbol),
                Direction::Hold => (0.0, 0.0),
            };
            let new_long_pct = (ctx.net_long_exposure + add_long) / ctx.account_equity;
            let new_short_pct = (ctx.net_short_exposure + add_short) / ctx.account_equity;
            if new_long_pct > long_cap {
                let boost_note = if regime_bullish { " (regime-boosted)" } else { "" };
                return RiskDecision::Rejected(format!(
                    "Net long exposure {:.1}% would exceed cap {:.0}%{boost_note}",
                    new_long_pct * 100.0,
                    long_cap * 100.0,
                ));
            }
            if new_short_pct > short_cap {
                let boost_note = if regime_bearish { " (regime-boosted)" } else { "" };
                return RiskDecision::Rejected(format!(
                    "Net short exposure {:.1}% would exceed cap {:.0}%{boost_note}",
                    new_short_pct * 100.0,
                    short_cap * 100.0,
                ));
            }
        }

        // 6. Per-strategy position limit
        if signal.direction != Direction::Hold
            && ctx.strategy_position_count >= self.config.max_positions_per_strategy
        {
            return RiskDecision::Rejected(format!(
                "Strategy {} already has {} positions (max {})",
                signal.strategy_name,
                ctx.strategy_position_count,
                self.config.max_positions_per_strategy,
            ));
        }

        // 7. Is direction HOLD?
        if signal.direction == Direction::Hold {
            return RiskDecision::Approved;
        }

        // 8. Does open position count >= max_open_positions?
        //    Applies symmetrically to BUY and SELL to prevent directional bias.
        //    Uses effective limit (halved during tier 1)
        if ctx.open_position_count >= effective_max_positions {
            let tier_note = if tier1_active { " (reduced by tier 1)" } else { "" };
            return RiskDecision::Rejected(format!(
                "Max open positions ({}{}) reached",
                effective_max_positions, tier_note,
            ));
        }

        // 9. Would new position size exceed max_position_size_pct of equity?
        //    Uses effective limit (halved during tier 1)
        if ctx.account_equity > 0.0 {
            let max_value = ctx.account_equity * effective_position_size_pct;
            if ctx.position_value_for_symbol > max_value {
                let tier_note = if tier1_active { " (reduced by tier 1)" } else { "" };
                return RiskDecision::Rejected(format!(
                    "Position value ${:.2} exceeds max ${:.2} ({:.0}% of equity{})",
                    ctx.position_value_for_symbol,
                    max_value,
                    effective_position_size_pct * 100.0,
                    tier_note,
                ));
            }
        }

        // 10. Was an order submitted for this symbol within throttle window?
        if let Some(last_time) = self.last_order_time.get(&signal.symbol) {
            if last_time.elapsed().as_secs() < self.config.order_throttle_secs {
                return RiskDecision::Rejected(format!(
                    "Order throttle: last order for {} was {}s ago (min {}s)",
                    signal.symbol,
                    last_time.elapsed().as_secs(),
                    self.config.order_throttle_secs,
                ));
            }
        }

        // 11. All checks passed
        RiskDecision::Approved
    }

    /// Compute stop-loss and take-profit prices for a day trade position.
    pub fn day_stop_take(&self, entry_price: f64, direction: &Direction) -> (f64, f64) {
        match direction {
            Direction::Buy => {
                let stop = entry_price * (1.0 - self.config.day_stop_loss_pct);
                let take = entry_price * (1.0 + self.config.day_take_profit_pct);
                (stop, take)
            }
            _ => {
                let stop = entry_price * (1.0 + self.config.day_stop_loss_pct);
                let take = entry_price * (1.0 - self.config.day_take_profit_pct);
                (stop, take)
            }
        }
    }

    /// Record that an order was submitted for a symbol (for throttle tracking).
    pub fn record_order(&mut self, symbol: &str) {
        self.last_order_time.insert(symbol.to_string(), Instant::now());
    }

    /// Evaluate a swing signal against swing-specific risk rules.
    pub fn evaluate_swing(&self, signal: &Signal, ctx: &SwingRiskContext) -> RiskDecision {
        // 1. Is trading halted?
        if ctx.trading_halted {
            return RiskDecision::Rejected("Trading is halted".into());
        }

        // 2. Daily loss limit (shared with day trading)
        if ctx.account_equity > 0.0 && ctx.daily_loss < 0.0 {
            let loss_pct = ctx.daily_loss.abs() / ctx.account_equity;
            if loss_pct >= self.config.max_daily_loss_pct {
                return RiskDecision::HaltAll(format!(
                    "Daily loss {:.2}% exceeds limit {:.2}%",
                    loss_pct * 100.0,
                    self.config.max_daily_loss_pct * 100.0,
                ));
            }
        }

        // 3. Composite confidence >= min_composite_confidence?
        if signal.confidence < self.swing_config.min_composite_confidence {
            return RiskDecision::Rejected(format!(
                "Composite confidence {:.2} below swing minimum {:.2}",
                signal.confidence, self.swing_config.min_composite_confidence,
            ));
        }

        // 4. Direction is HOLD?
        if signal.direction == Direction::Hold {
            return RiskDecision::Approved;
        }

        // 5. Swing position count < max? (symmetric for BUY and SELL)
        if ctx.swing_position_count >= self.swing_config.max_swing_positions {
            return RiskDecision::Rejected(format!(
                "Max swing positions ({}) reached",
                self.swing_config.max_swing_positions,
            ));
        }

        // 6. Portfolio heat check (applies to both directions)
        if ctx.account_equity > 0.0 {
            let new_heat = ctx.position_value_for_symbol
                * self.swing_config.per_position_stop_loss_pct
                / ctx.account_equity;
            let total_heat = ctx.current_portfolio_heat + new_heat;
            if total_heat > self.swing_config.max_portfolio_heat_pct {
                return RiskDecision::Rejected(format!(
                    "Portfolio heat {:.2}% would exceed limit {:.2}%",
                    total_heat * 100.0,
                    self.swing_config.max_portfolio_heat_pct * 100.0,
                ));
            }
        }

        // 7. All checks passed
        RiskDecision::Approved
    }

    /// Compute stop-loss and take-profit prices for a swing position.
    pub fn swing_stop_take(&self, entry_price: f64, direction: &Direction) -> (f64, f64) {
        match direction {
            Direction::Buy => {
                let stop = entry_price * (1.0 - self.swing_config.per_position_stop_loss_pct);
                let take = entry_price * (1.0 + self.swing_config.per_position_take_profit_pct);
                (stop, take)
            }
            _ => {
                let stop = entry_price * (1.0 + self.swing_config.per_position_stop_loss_pct);
                let take = entry_price * (1.0 - self.swing_config.per_position_take_profit_pct);
                (stop, take)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Direction, Signal};

    fn make_signal(direction: Direction, confidence: f64, symbol: &str) -> Signal {
        Signal {
            symbol: symbol.to_string(),
            direction,
            confidence,
            reason: "test".to_string(),
            strategy_name: "TestStrategy".to_string(),
            timestamp: "2026-03-16T14:30:00Z".to_string(),
            trade_type: crate::models::TradeType::Day,
        }
    }

    fn default_ctx() -> RiskContext {
        RiskContext {
            trading_halted: false,
            account_equity: 100_000.0,
            daily_loss: 0.0,
            open_position_count: 0,
            position_value_for_symbol: 5_000.0,
            spy_day_change_pct: 0.0,
            net_long_exposure: 0.0,
            net_short_exposure: 0.0,
            strategy_position_count: 0,
            profit_target_hit: false,
        }
    }

    // Check 1: trading halted
    #[test]
    fn test_reject_when_trading_halted() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.trading_halted = true;
        assert_eq!(
            engine.evaluate(&signal, &ctx),
            RiskDecision::Rejected("Trading is halted".into())
        );
    }

    // Check 2: daily loss limit
    #[test]
    fn test_halt_all_on_daily_loss_breach() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.daily_loss = -2_500.0; // 2.5% of 100k
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::HaltAll(_)));
    }

    #[test]
    fn test_no_halt_when_loss_below_limit() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.daily_loss = -1_000.0; // 1% — below 2% limit
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    // Check 3: signal confidence
    #[test]
    fn test_reject_low_confidence() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.40, "AAPL");
        let ctx = default_ctx();
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("confidence"));
        }
    }

    // Check 4: HOLD is always approved
    #[test]
    fn test_hold_always_approved() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Hold, 0.65, "AAPL");
        let ctx = default_ctx();
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    #[test]
    fn test_hold_with_low_confidence_rejected() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Hold, 0.10, "AAPL");
        let ctx = default_ctx();
        // Confidence check (step 3) runs before HOLD check (step 4)
        assert!(matches!(engine.evaluate(&signal, &ctx), RiskDecision::Rejected(_)));
    }

    // Check 5: max open positions (symmetric for BUY and SELL)
    #[test]
    fn test_reject_buy_at_max_positions() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.open_position_count = 4; // at max
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("Max open positions"));
        }
    }

    #[test]
    fn test_sell_rejected_at_max_positions() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.open_position_count = 4; // at max
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("Max open positions"));
        }
    }

    // Check 6: position size limit
    #[test]
    fn test_reject_oversized_position() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.position_value_for_symbol = 15_000.0; // 15% of 100k, limit is 10%
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("Position value"));
        }
    }

    // Check 7: order throttle
    #[test]
    fn test_reject_throttled_order() {
        let mut engine = RiskEngine::new(RiskConfig::default());
        engine.record_order("AAPL");
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let ctx = default_ctx();
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("throttle"));
        }
    }

    #[test]
    fn test_different_symbol_not_throttled() {
        let mut engine = RiskEngine::new(RiskConfig::default());
        engine.record_order("AAPL");
        let signal = make_signal(Direction::Buy, 0.80, "MSFT");
        let ctx = default_ctx();
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    // Check 8: all checks passed
    #[test]
    fn test_approved_when_all_checks_pass() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let ctx = default_ctx();
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    // Priority: halted takes precedence over daily loss
    #[test]
    fn test_halted_takes_priority_over_loss() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.trading_halted = true;
        ctx.daily_loss = -5_000.0;
        // Should be Rejected (halted), not HaltAll (loss)
        assert!(matches!(engine.evaluate(&signal, &ctx), RiskDecision::Rejected(_)));
    }

    // --- Swing risk tests ---

    fn make_swing_signal(direction: Direction, confidence: f64, symbol: &str) -> Signal {
        Signal {
            symbol: symbol.to_string(),
            direction,
            confidence,
            reason: "composite test".to_string(),
            strategy_name: "CompositeSwing".to_string(),
            timestamp: "2026-03-17T20:05:00Z".to_string(),
            trade_type: crate::models::TradeType::Swing,
        }
    }

    fn default_swing_ctx() -> SwingRiskContext {
        SwingRiskContext {
            trading_halted: false,
            account_equity: 100_000.0,
            daily_loss: 0.0,
            swing_position_count: 0,
            current_portfolio_heat: 0.0,
            position_value_for_symbol: 10_000.0,
        }
    }

    #[test]
    fn test_swing_approved_when_all_checks_pass() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_swing_signal(Direction::Buy, 0.80, "AAPL");
        let ctx = default_swing_ctx();
        assert_eq!(engine.evaluate_swing(&signal, &ctx), RiskDecision::Approved);
    }

    #[test]
    fn test_swing_reject_low_composite_confidence() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_swing_signal(Direction::Buy, 0.40, "AAPL");
        let ctx = default_swing_ctx();
        let result = engine.evaluate_swing(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("confidence"));
        }
    }

    #[test]
    fn test_swing_reject_max_positions() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_swing_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_swing_ctx();
        ctx.swing_position_count = 6; // default max is 6
        let result = engine.evaluate_swing(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
    }

    #[test]
    fn test_swing_sell_rejected_at_max_positions() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_swing_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_swing_ctx();
        ctx.swing_position_count = 6;
        let result = engine.evaluate_swing(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
    }

    #[test]
    fn test_swing_reject_portfolio_heat_exceeded() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_swing_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_swing_ctx();
        ctx.current_portfolio_heat = 0.055; // already near 6% limit
        ctx.position_value_for_symbol = 20_000.0; // would add 1% heat
        let result = engine.evaluate_swing(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
    }

    #[test]
    fn test_swing_hold_always_approved() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_swing_signal(Direction::Hold, 0.80, "AAPL");
        let ctx = default_swing_ctx();
        assert_eq!(engine.evaluate_swing(&signal, &ctx), RiskDecision::Approved);
    }

    #[test]
    fn test_swing_halted_rejected() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_swing_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_swing_ctx();
        ctx.trading_halted = true;
        assert!(matches!(engine.evaluate_swing(&signal, &ctx), RiskDecision::Rejected(_)));
    }

    #[test]
    fn test_swing_stop_take_prices() {
        let engine = RiskEngine::new(RiskConfig::default());
        let (stop, take) = engine.swing_stop_take(100.0, &Direction::Buy);
        assert!((stop - 95.0).abs() < 0.01); // 5% below
        assert!((take - 115.0).abs() < 0.01); // 15% above
    }

    // --- Day stop/take tests ---

    #[test]
    fn test_day_stop_take_long() {
        let engine = RiskEngine::new(RiskConfig::default());
        // Default: 1.0% stop, 3% take
        let (stop, take) = engine.day_stop_take(100.0, &Direction::Buy);
        assert!((stop - 99.0).abs() < 0.01);
        assert!((take - 103.0).abs() < 0.01);
    }

    #[test]
    fn test_day_stop_take_short() {
        let engine = RiskEngine::new(RiskConfig::default());
        let (stop, take) = engine.day_stop_take(100.0, &Direction::Sell);
        assert!((stop - 101.0).abs() < 0.01); // stop above entry for shorts
        assert!((take - 97.0).abs() < 0.01);  // take below entry for shorts
    }

    // --- Regime filter tests ---

    #[test]
    fn test_regime_filter_blocks_short_in_uptrend() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.spy_day_change_pct = 0.015; // SPY up 1.5%, threshold is 1%
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("regime filter"));
        }
    }

    #[test]
    fn test_regime_filter_blocks_long_in_downtrend() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.spy_day_change_pct = -0.015; // SPY down 1.5%
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("regime filter"));
        }
    }

    #[test]
    fn test_regime_filter_allows_long_in_uptrend() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.spy_day_change_pct = 0.015; // SPY up, but buying is fine
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    #[test]
    fn test_regime_filter_allows_short_in_downtrend() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.spy_day_change_pct = -0.015; // SPY down, shorting is fine
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    #[test]
    fn test_regime_filter_allows_within_threshold() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.spy_day_change_pct = 0.005; // SPY up 0.5%, below 1% threshold
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    #[test]
    fn test_regime_filter_disabled() {
        let mut config = RiskConfig::default();
        config.regime_filter_enabled = false;
        let engine = RiskEngine::new(config);
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.spy_day_change_pct = 0.03; // SPY up 3%, but filter disabled
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    // --- Net exposure cap tests ---

    #[test]
    fn test_reject_long_exceeding_net_exposure_cap() {
        let engine = RiskEngine::new(RiskConfig::default()); // 40% cap
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.net_long_exposure = 35_000.0; // 35% already
        ctx.position_value_for_symbol = 10_000.0; // would be 45%
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("long exposure"));
        }
    }

    #[test]
    fn test_reject_short_exceeding_net_exposure_cap() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.net_short_exposure = 35_000.0;
        ctx.position_value_for_symbol = 10_000.0;
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("short exposure"));
        }
    }

    #[test]
    fn test_allow_within_net_exposure_cap() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.net_long_exposure = 20_000.0; // 20%
        ctx.position_value_for_symbol = 10_000.0; // would be 30%, under 40%
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    // --- Regime-boosted exposure tests ---

    #[test]
    fn test_regime_boost_allows_long_in_uptrend() {
        // Base cap is 40%, boosted is 70%. SPY up 1.5% (above 1% threshold).
        // Long exposure at 55% would be blocked by base cap but allowed by boost.
        let engine = RiskEngine::new(RiskConfig::default()); // 40% base, 70% boosted
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.spy_day_change_pct = 0.015; // SPY up 1.5%
        ctx.net_long_exposure = 50_000.0; // 50% already
        ctx.position_value_for_symbol = 10_000.0; // would be 60%, blocked at 40% but ok at 70%
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    #[test]
    fn test_regime_boost_still_caps_at_boosted_limit() {
        // Even with boost, 75% would exceed the 70% boosted cap.
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.spy_day_change_pct = 0.015; // uptrend
        ctx.net_long_exposure = 65_000.0; // 65%
        ctx.position_value_for_symbol = 10_000.0; // would be 75%
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("regime-boosted"));
        }
    }

    #[test]
    fn test_regime_boost_does_not_apply_to_opposite_direction() {
        // SPY up 1.5% — shorts are blocked by regime filter (check 4),
        // but if we test the cap directly, short cap stays at base 40%.
        // Note: this scenario can't actually occur because check 4 rejects
        // the short first. But verify the cap logic is correct in isolation.
        let mut config = RiskConfig::default();
        config.regime_filter_enabled = false; // disable filter to test cap only
        let engine = RiskEngine::new(config);
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.spy_day_change_pct = 0.015; // uptrend — but filter disabled
        ctx.net_short_exposure = 35_000.0; // 35%
        ctx.position_value_for_symbol = 10_000.0; // would be 45%
        // Regime filter disabled, so no boost applies. Base cap 40% blocks this.
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
    }

    #[test]
    fn test_regime_boost_allows_short_in_downtrend() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.spy_day_change_pct = -0.015; // SPY down 1.5%
        ctx.net_short_exposure = 50_000.0; // 50%
        ctx.position_value_for_symbol = 10_000.0; // would be 60%
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    #[test]
    fn test_no_regime_boost_in_flat_market() {
        // SPY at +0.5%, below 1% threshold — no boost, base 40% cap applies.
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.spy_day_change_pct = 0.005; // flat
        ctx.net_long_exposure = 35_000.0;
        ctx.position_value_for_symbol = 10_000.0; // would be 45%
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
    }

    // --- Per-strategy position limit tests ---

    #[test]
    fn test_reject_strategy_at_max_positions() {
        let engine = RiskEngine::new(RiskConfig::default()); // max 2 per strategy
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.strategy_position_count = 2;
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("already has 2 positions"));
        }
    }

    #[test]
    fn test_allow_strategy_below_limit() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.strategy_position_count = 1;
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    // --- Tiered daily loss tests ---

    fn tiered_config() -> RiskConfig {
        RiskConfig {
            max_daily_loss_pct: 0.05,     // tier 3: halt at 5%
            daily_loss_tier1_pct: 0.02,   // tier 1: reduce at 2%
            daily_loss_tier2_pct: 0.03,   // tier 2: block at 3%
            max_open_positions: 8,
            daily_profit_target_pct: 0.0, // disabled by default in tests
            regime_boosted_exposure_pct: 0.70,
            ..RiskConfig::default()
        }
    }

    #[test]
    fn test_tier1_reduces_max_positions() {
        let engine = RiskEngine::new(tiered_config()); // max_open=8, tier1=2%
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.daily_loss = -2_500.0; // 2.5% of 100k, above tier1 (2%) but below tier2 (3%)
        ctx.open_position_count = 4; // effective max = 8/2 = 4, so at limit
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("tier 1"));
        }
    }

    #[test]
    fn test_tier1_allows_below_reduced_limit() {
        let engine = RiskEngine::new(tiered_config());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.daily_loss = -2_500.0; // tier 1 active
        ctx.open_position_count = 3; // below effective max of 4
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    #[test]
    fn test_tier2_blocks_all_new_entries() {
        let engine = RiskEngine::new(tiered_config()); // tier2=3%, halt=5%
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.daily_loss = -3_500.0; // 3.5%, above tier2 but below halt
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("tier 2"));
        }
    }

    #[test]
    fn test_tier2_blocks_sells_too() {
        let engine = RiskEngine::new(tiered_config());
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.daily_loss = -3_500.0;
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("tier 2"));
        }
    }

    #[test]
    fn test_tier3_halts_all_trading() {
        let engine = RiskEngine::new(tiered_config()); // halt at 5%
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.daily_loss = -5_500.0; // 5.5%, above halt threshold
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::HaltAll(_)));
    }

    // --- Daily profit target tests ---

    #[test]
    fn test_profit_target_blocks_new_buy() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.profit_target_hit = true;
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("profit target"));
        }
    }

    #[test]
    fn test_profit_target_blocks_new_sell() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.profit_target_hit = true;
        let result = engine.evaluate(&signal, &ctx);
        assert!(matches!(result, RiskDecision::Rejected(_)));
        if let RiskDecision::Rejected(reason) = result {
            assert!(reason.contains("profit target"));
        }
    }

    #[test]
    fn test_profit_target_allows_hold() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Hold, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.profit_target_hit = true;
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    #[test]
    fn test_no_block_when_profit_target_not_hit() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.profit_target_hit = false;
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }

    #[test]
    fn test_no_tier_when_loss_below_tier1() {
        let engine = RiskEngine::new(tiered_config()); // tier1=2%
        let signal = make_signal(Direction::Buy, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.daily_loss = -1_500.0; // 1.5%, below tier1
        ctx.open_position_count = 7; // below normal max of 8
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
    }
}
