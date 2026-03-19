export type Direction = "BUY" | "SELL" | "HOLD";

export type TradeType = "day" | "swing";

export interface Signal {
  symbol: string;
  direction: Direction;
  confidence: number;
  reason: string;
  strategy_name: string;
  timestamp: string;
  trade_type?: TradeType;
}

export interface Position {
  symbol: string;
  qty: number;
  avg_entry_price: number;
  current_price: number;
  unrealized_pnl: number;
  opened_at: string;
  trade_type?: TradeType;
  stop_loss_price?: number | null;
  take_profit_price?: number | null;
}

export interface Order {
  order_id: string;
  alpaca_id: string | null;
  symbol: string;
  side: "buy" | "sell";
  qty: number;
  filled_price: number | null;
  status: string;
  strategy_name: string;
  submitted_at: string;
  filled_at: string | null;
  trade_type?: TradeType;
}

export type SseEventType =
  | "POSITION_UPDATE"
  | "ORDER_FILL"
  | "TRADING_HALTED"
  | "TRADING_RESUMED"
  | "DAILY_PNL"
  | "RISK_BREACH"
  | "RISK_CONFIG_UPDATED";

export interface SseEvent {
  event_type: SseEventType;
  timestamp: string;
  payload: Record<string, unknown>;
}

export interface AccountInfo {
  equity: number;
  buying_power: number;
  cash: number;
  currency: string;
  status: string;
  mode: string;
  trading_blocked: boolean;
}

export interface Strategy {
  id: string;
  name: string;
  enabled: boolean;
  params: Record<string, unknown>;
  last_signal: Signal | null;
  win_rate: number | null;
}

export interface BacktestResult {
  strategy_name: string;
  symbol: string;
  total_return_pct: number;
  sharpe_ratio: number;
  max_drawdown_pct: number;
  win_rate: number;
  total_trades: number;
  avg_trade_duration_mins: number;
  profit_factor: number;
  period_start: string;
  period_end: string;
}

export interface BacktestEquityPoint {
  timestamp: string;
  equity: number;
}

export interface RiskConfig {
  max_daily_loss_pct: number;
  max_position_size_pct: number;
  max_open_positions: number;
  min_signal_confidence: number;
  order_throttle_secs: number;
  eod_flatten_time_et: string;
}

export interface CompanyInfo {
  symbol: string;
  name: string;
  sector: string | null;
  industry: string | null;
  market_cap: number | null;
  summary: string | null;
  current_price: number | null;
  previous_close: number | null;
  change_pct: number | null;
  fifty_two_week_high: number | null;
  fifty_two_week_low: number | null;
  average_volume: number | null;
}

export interface NewsArticle {
  headline: string;
  summary: string | null;
  source: string | null;
  url: string | null;
  published_at: string | null;
  sentiment: string | null;
  sentiment_score: number | null;
  thumbnail_url?: string | null;
  symbol?: string;
}

export interface MarketIndex {
  symbol: string;
  name: string;
  current_price: number;
  previous_close: number;
  change_abs: number;
  change_pct: number;
  intraday_prices: { timestamp: string; value: number }[];
}

export interface SectorPerformance {
  sector: string;
  symbol: string;
  change_pct: number;
}

export interface MarketMover {
  symbol: string;
  name: string;
  current_price: number | null;
  change_pct: number;
}

export interface PortfolioPnlHistory {
  timestamps: string[];
  equity: number[];
  pnl: number[];
  summary: {
    total_equity: number;
    period_pnl: number;
    period_pnl_pct: number;
    realized_pnl: number;
    buying_power: number;
    cash: number;
    day_positions: number;
    swing_positions: number;
    win_rate: number;
  };
}

export interface OhlcvBar {
  timestamp: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}
