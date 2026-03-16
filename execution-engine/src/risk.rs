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
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_daily_loss_pct: 0.02,
            max_position_size_pct: 0.10,
            max_open_positions: 4,
            min_signal_confidence: 0.60,
            order_throttle_secs: 300,
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
}

/// Risk engine that enforces all 8 checks in order.
pub struct RiskEngine {
    pub config: RiskConfig,
    last_order_time: HashMap<String, Instant>,
}

impl RiskEngine {
    pub fn new(config: RiskConfig) -> Self {
        Self {
            config,
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

        // 2. Is daily loss >= max_daily_loss_pct of equity?
        if ctx.account_equity > 0.0 {
            let loss_pct = ctx.daily_loss.abs() / ctx.account_equity;
            if ctx.daily_loss < 0.0 && loss_pct >= self.config.max_daily_loss_pct {
                return RiskDecision::HaltAll(format!(
                    "Daily loss {:.2}% exceeds limit {:.2}%",
                    loss_pct * 100.0,
                    self.config.max_daily_loss_pct * 100.0,
                ));
            }
        }

        // 3. Is signal confidence >= min_signal_confidence?
        if signal.confidence < self.config.min_signal_confidence {
            return RiskDecision::Rejected(format!(
                "Signal confidence {:.2} below minimum {:.2}",
                signal.confidence, self.config.min_signal_confidence,
            ));
        }

        // 4. Is direction HOLD?
        if signal.direction == Direction::Hold {
            return RiskDecision::Approved;
        }

        // 5. Does open position count >= max_open_positions (for BUY)?
        if signal.direction == Direction::Buy
            && ctx.open_position_count >= self.config.max_open_positions
        {
            return RiskDecision::Rejected(format!(
                "Max open positions ({}) reached",
                self.config.max_open_positions,
            ));
        }

        // 6. Would new position size exceed max_position_size_pct of equity?
        if ctx.account_equity > 0.0 {
            let max_value = ctx.account_equity * self.config.max_position_size_pct;
            if ctx.position_value_for_symbol > max_value {
                return RiskDecision::Rejected(format!(
                    "Position value ${:.2} exceeds max ${:.2} ({:.0}% of equity)",
                    ctx.position_value_for_symbol,
                    max_value,
                    self.config.max_position_size_pct * 100.0,
                ));
            }
        }

        // 7. Was an order submitted for this symbol within throttle window?
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

        // 8. All checks passed
        RiskDecision::Approved
    }

    /// Record that an order was submitted for a symbol (for throttle tracking).
    pub fn record_order(&mut self, symbol: &str) {
        self.last_order_time.insert(symbol.to_string(), Instant::now());
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
        }
    }

    fn default_ctx() -> RiskContext {
        RiskContext {
            trading_halted: false,
            account_equity: 100_000.0,
            daily_loss: 0.0,
            open_position_count: 0,
            position_value_for_symbol: 5_000.0,
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

    // Check 5: max open positions (BUY only)
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
    fn test_sell_allowed_at_max_positions() {
        let engine = RiskEngine::new(RiskConfig::default());
        let signal = make_signal(Direction::Sell, 0.80, "AAPL");
        let mut ctx = default_ctx();
        ctx.open_position_count = 4;
        assert_eq!(engine.evaluate(&signal, &ctx), RiskDecision::Approved);
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
}
