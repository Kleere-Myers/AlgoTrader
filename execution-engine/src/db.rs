use rusqlite::{params, Connection, OpenFlags};

use crate::models::{Bar, Order, Position};

/// Database path from env var or default.
fn db_path() -> String {
    std::env::var("DB_PATH")
        .or_else(|_| std::env::var("DUCKDB_PATH")) // backward compat
        .unwrap_or_else(|_| "../data/algotrader.sqlite".into())
}

/// Open a SQLite connection for read-write with WAL mode enabled.
pub fn connect() -> Result<Connection, rusqlite::Error> {
    let con = Connection::open(db_path())?;
    con.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA busy_timeout=5000;
         PRAGMA foreign_keys=ON;",
    )?;
    Ok(con)
}

/// Open a SQLite connection for read-only access.
pub fn connect_readonly() -> Result<Connection, rusqlite::Error> {
    let con = Connection::open_with_flags(
        db_path(),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    con.execute_batch("PRAGMA busy_timeout=5000;")?;
    Ok(con)
}

/// Ensure all required tables exist. Called on startup so the engine
/// self-heals if the DB file is new or was recreated.
pub fn ensure_schema(con: &Connection) -> Result<(), rusqlite::Error> {
    con.execute_batch(
        "CREATE TABLE IF NOT EXISTS ohlcv_bars (
            symbol    TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            open      REAL NOT NULL,
            high      REAL NOT NULL,
            low       REAL NOT NULL,
            close     REAL NOT NULL,
            volume    INTEGER NOT NULL,
            bar_size  TEXT NOT NULL DEFAULT '5min',
            PRIMARY KEY (symbol, timestamp, bar_size)
        );
        CREATE TABLE IF NOT EXISTS signals (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            strategy_name TEXT NOT NULL,
            symbol        TEXT NOT NULL,
            timestamp     TEXT NOT NULL,
            direction     TEXT NOT NULL CHECK (direction IN ('BUY','SELL','HOLD')),
            confidence    REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
            reason        TEXT,
            metadata      TEXT,
            trade_type    TEXT NOT NULL DEFAULT 'day' CHECK (trade_type IN ('day','swing'))
        );
        CREATE TABLE IF NOT EXISTS orders (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            order_id      TEXT NOT NULL UNIQUE,
            alpaca_id     TEXT,
            symbol        TEXT NOT NULL,
            side          TEXT NOT NULL CHECK (side IN ('buy','sell')),
            qty           REAL NOT NULL,
            order_type    TEXT NOT NULL DEFAULT 'market',
            limit_price   REAL,
            filled_price  REAL,
            status        TEXT NOT NULL DEFAULT 'pending',
            strategy_name TEXT,
            created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
            filled_at     TEXT,
            trade_type    TEXT NOT NULL DEFAULT 'day' CHECK (trade_type IN ('day','swing'))
        );
        CREATE TABLE IF NOT EXISTS positions (
            symbol          TEXT PRIMARY KEY,
            side            TEXT NOT NULL DEFAULT 'long' CHECK (side IN ('long','short')),
            qty             REAL NOT NULL,
            avg_entry_price REAL NOT NULL,
            current_price   REAL,
            unrealized_pnl  REAL,
            updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
            trade_type      TEXT NOT NULL DEFAULT 'day' CHECK (trade_type IN ('day','swing')),
            stop_loss_price REAL,
            take_profit_price REAL
        );
        CREATE TABLE IF NOT EXISTS daily_pnl (
            date           TEXT PRIMARY KEY,
            realized_pnl   REAL NOT NULL DEFAULT 0.0,
            unrealized_pnl REAL NOT NULL DEFAULT 0.0,
            total_trades   INTEGER NOT NULL DEFAULT 0,
            win_rate       REAL,
            account_equity REAL
        );
        CREATE TABLE IF NOT EXISTS strategy_config (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            strategy_name TEXT NOT NULL UNIQUE,
            params        TEXT,
            enabled       INTEGER NOT NULL DEFAULT 1,
            created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
            updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
        );
        CREATE TABLE IF NOT EXISTS watched_symbols (
            symbol      TEXT PRIMARY KEY,
            added_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
        );",
    )?;
    Ok(())
}

/// Upsert an OHLCV bar into the ohlcv_bars table.
pub fn upsert_bar(con: &Connection, bar: &Bar, bar_size: &str) -> Result<(), rusqlite::Error> {
    con.execute(
        "INSERT OR REPLACE INTO ohlcv_bars (symbol, timestamp, open, high, low, close, volume, bar_size) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![bar.symbol, bar.timestamp, bar.open, bar.high, bar.low, bar.close, bar.volume, bar_size],
    )?;
    Ok(())
}

/// Insert an order into the orders table.
pub fn insert_order(con: &Connection, order: &Order) -> Result<(), rusqlite::Error> {
    con.execute(
        "INSERT INTO orders (order_id, alpaca_id, symbol, side, qty, filled_price, status, strategy_name, created_at, filled_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            order.order_id,
            order.alpaca_id,
            order.symbol,
            order.side,
            order.qty,
            order.filled_price,
            order.status,
            order.strategy_name,
            order.created_at,
            order.filled_at,
        ],
    )?;
    Ok(())
}

/// Update an order's status, fill price, and fill time.
pub fn update_order_fill(
    con: &Connection,
    order_id: &str,
    status: &str,
    filled_price: Option<f64>,
    filled_at: Option<&str>,
) -> Result<(), rusqlite::Error> {
    con.execute(
        "UPDATE orders SET status = ?1, filled_price = ?2, filled_at = ?3 WHERE order_id = ?4",
        params![status, filled_price, filled_at, order_id],
    )?;
    Ok(())
}

/// Upsert a position in the positions table.
pub fn upsert_position(con: &Connection, pos: &Position) -> Result<(), rusqlite::Error> {
    let trade_type_str = match pos.trade_type {
        crate::models::TradeType::Day => "day",
        crate::models::TradeType::Swing => "swing",
    };
    let side_str = match pos.side {
        crate::models::PositionSide::Long => "long",
        crate::models::PositionSide::Short => "short",
    };
    con.execute(
        "INSERT OR REPLACE INTO positions (symbol, side, qty, avg_entry_price, current_price, unrealized_pnl, updated_at, trade_type, stop_loss_price, take_profit_price) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%d %H:%M:%f', 'now'), ?7, ?8, ?9)",
        params![pos.symbol, side_str, pos.qty, pos.avg_entry_price, pos.current_price, pos.unrealized_pnl, trade_type_str, pos.stop_loss_price, pos.take_profit_price],
    )?;
    Ok(())
}

/// Remove a position (when fully closed).
pub fn delete_position(con: &Connection, symbol: &str) -> Result<(), rusqlite::Error> {
    con.execute("DELETE FROM positions WHERE symbol = ?1", params![symbol])?;
    Ok(())
}

/// Load all positions from the database.
pub fn load_positions(con: &Connection) -> Result<Vec<Position>, rusqlite::Error> {
    let mut stmt = con.prepare(
        "SELECT symbol, qty, avg_entry_price, current_price, unrealized_pnl, \
         COALESCE(trade_type, 'day'), stop_loss_price, take_profit_price, \
         COALESCE(side, 'long') FROM positions",
    )?;
    let rows = stmt.query_map([], |row| {
        let tt_str: String = row.get(5)?;
        let trade_type = if tt_str == "swing" {
            crate::models::TradeType::Swing
        } else {
            crate::models::TradeType::Day
        };
        let side_str: String = row.get(8)?;
        let side = if side_str == "short" {
            crate::models::PositionSide::Short
        } else {
            crate::models::PositionSide::Long
        };
        Ok(Position {
            symbol: row.get(0)?,
            side,
            qty: row.get(1)?,
            avg_entry_price: row.get(2)?,
            current_price: row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
            unrealized_pnl: row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
            trade_type,
            stop_loss_price: row.get(6)?,
            take_profit_price: row.get(7)?,
            strategy_name: String::new(),
        })
    })?;
    let mut positions = Vec::new();
    for row in rows {
        positions.push(row?);
    }
    Ok(positions)
}

/// Load recent orders from the database.
pub fn load_orders(con: &Connection, limit: usize) -> Result<Vec<Order>, rusqlite::Error> {
    let sql = format!(
        "SELECT order_id, COALESCE(alpaca_id, ''), symbol, side, qty, filled_price, status, \
         COALESCE(strategy_name, ''), created_at, filled_at, \
         COALESCE(trade_type, 'day') \
         FROM orders ORDER BY created_at DESC LIMIT {}",
        limit
    );
    let mut stmt = con.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let alpaca_id: String = row.get(1)?;
        let strategy_name: String = row.get(7)?;
        let tt_str: String = row.get(10)?;
        let trade_type = if tt_str == "swing" {
            crate::models::TradeType::Swing
        } else {
            crate::models::TradeType::Day
        };
        Ok(Order {
            order_id: row.get(0)?,
            alpaca_id: if alpaca_id.is_empty() { None } else { Some(alpaca_id) },
            symbol: row.get(2)?,
            side: row.get(3)?,
            qty: row.get(4)?,
            filled_price: row.get(5)?,
            status: row.get(6)?,
            strategy_name: if strategy_name.is_empty() { String::new() } else { strategy_name },
            created_at: row.get(8)?,
            filled_at: row.get(9)?,
            trade_type,
        })
    })?;
    let mut orders = Vec::new();
    for row in rows {
        orders.push(row?);
    }
    Ok(orders)
}

/// Insert a signal into the signals table (proxied from strategy engine).
pub fn insert_signal(
    con: &Connection,
    strategy_name: &str,
    symbol: &str,
    timestamp: &str,
    direction: &str,
    confidence: f64,
    reason: &str,
    trade_type: &str,
) -> Result<(), rusqlite::Error> {
    con.execute(
        "INSERT INTO signals (strategy_name, symbol, timestamp, direction, confidence, reason, trade_type) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![strategy_name, symbol, timestamp, direction, confidence, reason, trade_type],
    )?;
    Ok(())
}

/// Add a symbol to the watched_symbols table.
pub fn add_watched_symbol(con: &Connection, symbol: &str) -> Result<(), rusqlite::Error> {
    con.execute(
        "INSERT OR IGNORE INTO watched_symbols (symbol) VALUES (?1)",
        params![symbol],
    )?;
    Ok(())
}

/// Remove a symbol from the watched_symbols table.
pub fn remove_watched_symbol(con: &Connection, symbol: &str) -> Result<usize, rusqlite::Error> {
    let affected = con.execute(
        "DELETE FROM watched_symbols WHERE symbol = ?1",
        params![symbol],
    )?;
    Ok(affected)
}

/// Fetch recent bars for a symbol from the database (for sending to strategy engine).
pub fn get_recent_bars(
    con: &Connection,
    symbol: &str,
    bar_size: &str,
    limit: usize,
) -> Result<Vec<Bar>, rusqlite::Error> {
    let mut stmt = con.prepare(
        "SELECT symbol, timestamp, open, high, low, close, volume \
         FROM ohlcv_bars WHERE symbol = ?1 AND bar_size = ?2 \
         ORDER BY timestamp DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![symbol, bar_size, limit as i64], |row| {
        Ok(Bar {
            symbol: row.get(0)?,
            timestamp: row.get(1)?,
            open: row.get(2)?,
            high: row.get(3)?,
            low: row.get(4)?,
            close: row.get(5)?,
            volume: row.get(6)?,
        })
    })?;
    let mut bars: Vec<Bar> = Vec::new();
    for row in rows {
        bars.push(row?);
    }
    // Reverse so oldest is first (ascending order)
    bars.reverse();
    Ok(bars)
}
