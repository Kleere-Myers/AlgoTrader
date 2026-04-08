use std::collections::HashMap;

use crate::models::{AlpacaPosition, Position, PositionSide, TradeType};

/// In-memory position tracker, synced to DuckDB on changes.
#[derive(Debug, Clone)]
pub struct PositionTracker {
    positions: HashMap<String, Position>,
}

impl PositionTracker {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
        }
    }

    /// Load positions from a Vec (e.g. from DuckDB on startup).
    pub fn load(&mut self, positions: Vec<Position>) {
        for pos in positions {
            self.positions.insert(pos.symbol.clone(), pos);
        }
    }

    pub fn get(&self, symbol: &str) -> Option<&Position> {
        self.positions.get(symbol)
    }

    pub fn count(&self) -> usize {
        self.positions.len()
    }

    pub fn all(&self) -> Vec<Position> {
        self.positions.values().cloned().collect()
    }

    /// Return only day-trading positions (for EOD flatten).
    pub fn day_positions(&self) -> Vec<Position> {
        self.positions.values()
            .filter(|p| p.trade_type == TradeType::Day)
            .cloned()
            .collect()
    }

    /// Return only swing-trading positions.
    pub fn swing_positions(&self) -> Vec<Position> {
        self.positions.values()
            .filter(|p| p.trade_type == TradeType::Swing)
            .cloned()
            .collect()
    }

    /// Calculate unrealized P&L based on position side.
    fn calc_pnl(side: &PositionSide, entry: f64, current: f64, qty: f64) -> f64 {
        match side {
            PositionSide::Long => (current - entry) * qty,
            PositionSide::Short => (entry - current) * qty,
        }
    }

    /// Count positions opened by a given strategy.
    pub fn count_by_strategy(&self, strategy_name: &str) -> usize {
        self.positions.values()
            .filter(|p| p.strategy_name == strategy_name)
            .count()
    }

    /// Calculate net dollar exposure: sum of (long values) - sum of (short values).
    /// Returns (net_long_value, net_short_value) as absolute dollar amounts.
    pub fn net_exposure(&self) -> (f64, f64) {
        let mut long_value = 0.0;
        let mut short_value = 0.0;
        for pos in self.positions.values() {
            let value = pos.qty * pos.current_price;
            match pos.side {
                PositionSide::Long => long_value += value,
                PositionSide::Short => short_value += value,
            }
        }
        (long_value, short_value)
    }

    /// Update or create a position after a fill.
    pub fn update_on_fill(
        &mut self,
        symbol: &str,
        side: &str,
        qty: f64,
        fill_price: f64,
        trade_type: TradeType,
        stop_loss_price: Option<f64>,
        take_profit_price: Option<f64>,
    ) -> Option<Position> {
        self.update_on_fill_with_strategy(symbol, side, qty, fill_price, trade_type, stop_loss_price, take_profit_price, "")
    }

    /// Update or create a position after a fill, with strategy tracking.
    pub fn update_on_fill_with_strategy(
        &mut self,
        symbol: &str,
        side: &str,
        qty: f64,
        fill_price: f64,
        trade_type: TradeType,
        stop_loss_price: Option<f64>,
        take_profit_price: Option<f64>,
        strategy_name: &str,
    ) -> Option<Position> {
        let existing = self.positions.get(symbol).cloned();

        match (side, existing) {
            // Open long position
            ("buy", None) => {
                let pos = Position {
                    symbol: symbol.to_string(),
                    side: PositionSide::Long,
                    qty,
                    avg_entry_price: fill_price,
                    current_price: fill_price,
                    unrealized_pnl: 0.0,
                    trade_type,
                    stop_loss_price,
                    take_profit_price,
                    strategy_name: strategy_name.to_string(),
                };
                self.positions.insert(symbol.to_string(), pos.clone());
                Some(pos)
            }
            // Add to long position
            ("buy", Some(mut pos)) if pos.side == PositionSide::Long => {
                let total_cost = pos.avg_entry_price * pos.qty + fill_price * qty;
                pos.qty += qty;
                pos.avg_entry_price = total_cost / pos.qty;
                pos.current_price = fill_price;
                pos.unrealized_pnl = Self::calc_pnl(&pos.side, pos.avg_entry_price, pos.current_price, pos.qty);
                self.positions.insert(symbol.to_string(), pos.clone());
                Some(pos)
            }
            // Cover short position (buy to close)
            ("buy", Some(mut pos)) if pos.side == PositionSide::Short => {
                pos.qty -= qty;
                if pos.qty <= 0.001 {
                    self.positions.remove(symbol);
                    return None;
                }
                pos.current_price = fill_price;
                pos.unrealized_pnl = Self::calc_pnl(&pos.side, pos.avg_entry_price, pos.current_price, pos.qty);
                self.positions.insert(symbol.to_string(), pos.clone());
                Some(pos)
            }
            // Open short position
            ("sell", None) => {
                let pos = Position {
                    symbol: symbol.to_string(),
                    side: PositionSide::Short,
                    qty,
                    avg_entry_price: fill_price,
                    current_price: fill_price,
                    unrealized_pnl: 0.0,
                    trade_type,
                    stop_loss_price,
                    take_profit_price,
                    strategy_name: strategy_name.to_string(),
                };
                self.positions.insert(symbol.to_string(), pos.clone());
                Some(pos)
            }
            // Add to short position
            ("sell", Some(mut pos)) if pos.side == PositionSide::Short => {
                let total_cost = pos.avg_entry_price * pos.qty + fill_price * qty;
                pos.qty += qty;
                pos.avg_entry_price = total_cost / pos.qty;
                pos.current_price = fill_price;
                pos.unrealized_pnl = Self::calc_pnl(&pos.side, pos.avg_entry_price, pos.current_price, pos.qty);
                self.positions.insert(symbol.to_string(), pos.clone());
                Some(pos)
            }
            // Close long position (sell to close)
            ("sell", Some(mut pos)) if pos.side == PositionSide::Long => {
                pos.qty -= qty;
                if pos.qty <= 0.001 {
                    self.positions.remove(symbol);
                    return None;
                }
                pos.current_price = fill_price;
                pos.unrealized_pnl = Self::calc_pnl(&pos.side, pos.avg_entry_price, pos.current_price, pos.qty);
                self.positions.insert(symbol.to_string(), pos.clone());
                Some(pos)
            }
            _ => None,
        }
    }

    /// Update current_price and unrealized_pnl for a position without changing qty.
    pub fn update_price(&mut self, symbol: &str, current_price: f64) -> Option<Position> {
        if let Some(pos) = self.positions.get_mut(symbol) {
            pos.current_price = current_price;
            pos.unrealized_pnl = Self::calc_pnl(&pos.side, pos.avg_entry_price, current_price, pos.qty);
            Some(pos.clone())
        } else {
            None
        }
    }

    /// Sync local positions with Alpaca's actual holdings.
    /// - Updates qty and prices for positions that exist on both sides.
    /// - Removes local positions that Alpaca no longer has.
    /// - Adds positions that Alpaca has but we don't track locally.
    /// Returns the list of symbols that were changed or added.
    pub fn sync_with_alpaca(&mut self, alpaca_positions: &[AlpacaPosition]) -> Vec<String> {
        let mut changed = Vec::new();

        // Build a map of Alpaca positions by symbol (long AND short)
        let alpaca_map: HashMap<String, &AlpacaPosition> = alpaca_positions
            .iter()
            .map(|p| (p.symbol.clone(), p))
            .collect();

        // Remove local positions that Alpaca no longer has
        let local_symbols: Vec<String> = self.positions.keys().cloned().collect();
        for sym in &local_symbols {
            if !alpaca_map.contains_key(sym) {
                self.positions.remove(sym);
                changed.push(sym.clone());
            }
        }

        // Update existing / add new from Alpaca
        for (sym, ap) in &alpaca_map {
            let qty: f64 = ap.qty.parse::<f64>().unwrap_or(0.0).abs();
            let avg_entry: f64 = ap.avg_entry_price.parse().unwrap_or(0.0);
            let current: f64 = ap.current_price.parse().unwrap_or(0.0);
            let pnl: f64 = ap.unrealized_pl.parse().unwrap_or(0.0);
            let pos_side = if ap.side == "short" {
                PositionSide::Short
            } else {
                PositionSide::Long
            };

            if qty <= 0.0 {
                continue;
            }

            if let Some(pos) = self.positions.get_mut(sym) {
                // Update qty and prices if they differ
                if (pos.qty - qty).abs() > 0.001
                    || (pos.current_price - current).abs() > 0.001
                    || (pos.avg_entry_price - avg_entry).abs() > 0.001
                    || pos.side != pos_side
                {
                    pos.qty = qty;
                    pos.side = pos_side;
                    pos.avg_entry_price = avg_entry;
                    pos.current_price = current;
                    pos.unrealized_pnl = pnl;
                    changed.push(sym.clone());
                }
            } else {
                // New position from Alpaca we don't have locally
                self.positions.insert(sym.clone(), Position {
                    symbol: sym.clone(),
                    side: pos_side,
                    qty,
                    avg_entry_price: avg_entry,
                    current_price: current,
                    unrealized_pnl: pnl,
                    trade_type: TradeType::Day, // Default; can't know from Alpaca
                    stop_loss_price: None,
                    take_profit_price: None,
                    strategy_name: String::new(),
                });
                changed.push(sym.clone());
            }
        }

        changed
    }

    /// Total unrealized P&L across all day-trading positions.
    pub fn day_unrealized_pnl(&self) -> f64 {
        self.positions.values()
            .filter(|p| p.trade_type == TradeType::Day)
            .map(|p| p.unrealized_pnl)
            .sum()
    }

    /// Calculate approximate value of a position given a price.
    pub fn position_value(&self, symbol: &str, price: f64) -> f64 {
        match self.positions.get(symbol) {
            Some(pos) => pos.qty * price,
            None => price, // new position — value would be 1 share * price as minimum
        }
    }
}
