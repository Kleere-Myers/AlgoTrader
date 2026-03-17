export type Direction = "BUY" | "SELL" | "HOLD";

export interface Signal {
  symbol: string;
  direction: Direction;
  confidence: number;
  reason: string;
  strategy_name: string;
  timestamp: string;
}

export interface Position {
  symbol: string;
  qty: number;
  avg_entry_price: number;
  current_price: number;
  unrealized_pnl: number;
  opened_at: string;
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
}

export interface OhlcvBar {
  timestamp: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}
