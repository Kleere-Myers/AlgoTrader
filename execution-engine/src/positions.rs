use std::collections::HashMap;

use crate::models::Position;

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

    /// Update or create a position after a fill.
    pub fn update_on_fill(
        &mut self,
        symbol: &str,
        side: &str,
        qty: f64,
        fill_price: f64,
    ) -> Option<Position> {
        let existing = self.positions.get(symbol).cloned();

        match (side, existing) {
            ("buy", None) => {
                let pos = Position {
                    symbol: symbol.to_string(),
                    qty,
                    avg_entry_price: fill_price,
                    current_price: fill_price,
                    unrealized_pnl: 0.0,
                };
                self.positions.insert(symbol.to_string(), pos.clone());
                Some(pos)
            }
            ("buy", Some(mut pos)) => {
                let total_cost = pos.avg_entry_price * pos.qty + fill_price * qty;
                pos.qty += qty;
                pos.avg_entry_price = total_cost / pos.qty;
                pos.current_price = fill_price;
                pos.unrealized_pnl = (pos.current_price - pos.avg_entry_price) * pos.qty;
                self.positions.insert(symbol.to_string(), pos.clone());
                Some(pos)
            }
            ("sell", Some(mut pos)) => {
                pos.qty -= qty;
                if pos.qty <= 0.001 {
                    // Position fully closed
                    self.positions.remove(symbol);
                    return None;
                }
                pos.current_price = fill_price;
                pos.unrealized_pnl = (pos.current_price - pos.avg_entry_price) * pos.qty;
                self.positions.insert(symbol.to_string(), pos.clone());
                Some(pos)
            }
            _ => None,
        }
    }

    /// Calculate approximate value of a position given a price.
    pub fn position_value(&self, symbol: &str, price: f64) -> f64 {
        match self.positions.get(symbol) {
            Some(pos) => pos.qty * price,
            None => price, // new position — value would be 1 share * price as minimum
        }
    }
}
