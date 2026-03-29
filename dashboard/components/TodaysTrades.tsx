"use client";

import { useEffect, useState, useCallback } from "react";
import Link from "next/link";
import type { Order } from "@/types";
import { executionApi } from "@/lib/api";

interface RoundTrip {
  symbol: string;
  side: "long" | "short";
  qty: number;
  entryPrice: number;
  exitPrice: number;
  pnl: number;
  pnlPct: number;
  strategy: string;
  exitTime: string;
}

/** Get the created/submitted timestamp from an order, handling both field names */
function orderTime(o: Order): string {
  return o.created_at || o.submitted_at || "";
}

/** Parse timestamps that may lack T/Z separators (e.g. "2026-03-23 19:45:03.727") */
function parseTimestamp(ts: string): Date {
  // Replace space separator with T for ISO parsing
  const normalized = ts.includes("T") ? ts : ts.replace(" ", "T");
  // Only append Z if no timezone info present (no Z, no +/- offset)
  if (/[Z]$/i.test(normalized) || /[+-]\d{2}:\d{2}$/.test(normalized)) {
    return new Date(normalized);
  }
  return new Date(normalized + "Z");
}

function pairRoundTrips(orders: Order[]): RoundTrip[] {
  const bySymbol: Record<string, Order[]> = {};
  for (const o of orders) {
    if (o.status.toLowerCase() !== "filled" || o.filled_price === null) continue;
    (bySymbol[o.symbol] ??= []).push(o);
  }

  const trips: RoundTrip[] = [];
  for (const symbol of Object.keys(bySymbol)) {
    const symbolOrders = bySymbol[symbol].sort(
      (a, b) => parseTimestamp(a.filled_at || orderTime(a)).getTime() - parseTimestamp(b.filled_at || orderTime(b)).getTime()
    );

    const pending: Order[] = [];
    for (const o of symbolOrders) {
      const matchIdx = pending.findIndex((p) => p.side !== o.side);
      if (matchIdx !== -1) {
        const entry = pending.splice(matchIdx, 1)[0];
        const isLong = entry.side === "buy";
        const entryPrice = entry.filled_price!;
        const exitPrice = o.filled_price!;
        const qty = Math.min(entry.qty, o.qty);
        const pnl = isLong
          ? (exitPrice - entryPrice) * qty
          : (entryPrice - exitPrice) * qty;
        const pnlPct = isLong
          ? ((exitPrice - entryPrice) / entryPrice) * 100
          : ((entryPrice - exitPrice) / entryPrice) * 100;
        trips.push({
          symbol,
          side: isLong ? "long" : "short",
          qty,
          entryPrice,
          exitPrice,
          pnl,
          pnlPct,
          strategy: o.strategy_name || entry.strategy_name,
          exitTime: o.filled_at || orderTime(o),
        });
      } else {
        pending.push(o);
      }
    }
  }

  return trips.sort(
    (a, b) => parseTimestamp(b.exitTime).getTime() - parseTimestamp(a.exitTime).getTime()
  );
}

function formatTime(ts: string): string {
  const d = parseTimestamp(ts);
  return d.toLocaleTimeString("en-US", {
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });
}

function formatCurrency(n: number): string {
  const abs = Math.abs(n);
  if (abs >= 1000) return `${n >= 0 ? "" : "-"}$${(abs / 1000).toFixed(1)}k`;
  return `${n >= 0 ? "" : "-"}$${abs.toFixed(2)}`;
}

export default function TodaysTrades() {
  const [orders, setOrders] = useState<Order[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchOrders = useCallback(async () => {
    try {
      const all = await executionApi.getOrders();
      // Filter to today's orders (UTC day)
      const now = new Date();
      const startOfDayUTC = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
      const today = all.filter((o) => {
        const t = parseTimestamp(orderTime(o));
        return t.getTime() >= startOfDayUTC;
      });
      setOrders(today);
    } catch {}
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchOrders();
    const interval = setInterval(fetchOrders, 30_000);
    return () => clearInterval(interval);
  }, [fetchOrders]);

  const filled = orders.filter((o) => o.status.toLowerCase() === "filled");
  const roundTrips = pairRoundTrips(orders);
  const wins = roundTrips.filter((rt) => rt.pnl > 0);
  const losses = roundTrips.filter((rt) => rt.pnl < 0);
  const totalPnl = roundTrips.reduce((sum, rt) => sum + rt.pnl, 0);
  const winRate = roundTrips.length > 0 ? (wins.length / roundTrips.length) * 100 : 0;
  const openTrades = filled.length - roundTrips.length * 2;

  if (loading) {
    return (
      <div className="bg-surface-900 rounded-lg border border-surface-600 p-4">
        <div className="bg-surface-800 rounded h-4 w-32 animate-pulse mb-3" />
        <div className="space-y-2">
          {Array.from({ length: 3 }).map((_, i) => (
            <div key={i} className="bg-surface-800 rounded h-12 animate-pulse" />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="bg-surface-900 rounded-lg border border-surface-600 p-4">
      {/* Header */}
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-[11px] font-mono font-medium text-text-secondary uppercase tracking-widest">
          Today&apos;s Trades
        </h3>
        <span className="text-xs text-text-secondary font-mono">
          {filled.length} fill{filled.length !== 1 ? "s" : ""}
        </span>
      </div>

      {/* Summary Stats */}
      {roundTrips.length > 0 && (
        <div className="grid grid-cols-4 gap-3 mb-4">
          <div className="bg-surface-800 rounded-lg px-3 py-2">
            <div className="text-[10px] text-text-secondary uppercase tracking-wider mb-0.5">Net P&L</div>
            <div className={`text-sm font-mono tabular-nums font-semibold ${totalPnl >= 0 ? "text-gain" : "text-loss"}`}>
              {totalPnl >= 0 ? "+" : ""}{formatCurrency(totalPnl)}
            </div>
          </div>
          <div className="bg-surface-800 rounded-lg px-3 py-2">
            <div className="text-[10px] text-text-secondary uppercase tracking-wider mb-0.5">Win Rate</div>
            <div className="text-sm font-mono tabular-nums font-semibold text-text-primary">
              {winRate.toFixed(0)}%
            </div>
          </div>
          <div className="bg-surface-800 rounded-lg px-3 py-2">
            <div className="text-[10px] text-text-secondary uppercase tracking-wider mb-0.5">W / L</div>
            <div className="text-sm font-mono tabular-nums font-semibold">
              <span className="text-gain">{wins.length}</span>
              <span className="text-text-secondary mx-1">/</span>
              <span className="text-loss">{losses.length}</span>
            </div>
          </div>
          <div className="bg-surface-800 rounded-lg px-3 py-2">
            <div className="text-[10px] text-text-secondary uppercase tracking-wider mb-0.5">Round Trips</div>
            <div className="text-sm font-mono tabular-nums font-semibold text-text-primary">
              {roundTrips.length}
            </div>
          </div>
        </div>
      )}

      {/* Trade List */}
      {roundTrips.length === 0 && filled.length === 0 ? (
        <p className="text-text-secondary text-sm text-center py-4">No trades today</p>
      ) : (
        <div className="space-y-1.5 max-h-[320px] overflow-y-auto pr-1">
          {/* Completed round trips */}
          {roundTrips.map((rt, i) => (
            <div
              key={i}
              className="flex items-center gap-3 bg-surface-800 rounded-lg px-3 py-2 hover:bg-surface-700 transition-colors"
            >
              {/* P&L indicator bar */}
              <div
                className={`w-1 h-8 rounded-full flex-shrink-0 ${
                  rt.pnl >= 0 ? "bg-gain" : "bg-loss"
                }`}
              />

              {/* Symbol + strategy */}
              <div className="min-w-[80px]">
                <Link
                  href={`/quote/${rt.symbol}`}
                  className="text-sm font-semibold text-text-primary hover:text-accent transition-colors"
                >
                  {rt.symbol}
                </Link>
                <div className="text-[10px] text-text-secondary truncate max-w-[100px]">
                  {rt.strategy}
                </div>
              </div>

              {/* Side badge */}
              <span
                className={`text-[10px] font-semibold font-mono uppercase tracking-wider px-1.5 py-0.5 rounded ${
                  rt.side === "long"
                    ? "bg-gain/10 text-gain"
                    : "bg-loss/10 text-loss"
                }`}
              >
                {rt.side}
              </span>

              {/* Entry → Exit */}
              <div className="flex-1 text-center">
                <div className="text-xs font-mono tabular-nums text-text-secondary">
                  ${rt.entryPrice.toFixed(2)}
                  <span className="mx-1.5 text-surface-500">&rarr;</span>
                  ${rt.exitPrice.toFixed(2)}
                </div>
                <div className="text-[10px] text-text-secondary">
                  {rt.qty} share{rt.qty !== 1 ? "s" : ""}
                </div>
              </div>

              {/* P&L */}
              <div className="text-right min-w-[70px]">
                <div
                  className={`text-sm font-mono tabular-nums font-semibold ${
                    rt.pnl >= 0 ? "text-gain" : "text-loss"
                  }`}
                >
                  {rt.pnl >= 0 ? "+" : ""}{formatCurrency(rt.pnl)}
                </div>
                <div
                  className={`text-[10px] font-mono tabular-nums ${
                    rt.pnlPct >= 0 ? "text-gain" : "text-loss"
                  }`}
                >
                  {rt.pnlPct >= 0 ? "+" : ""}{rt.pnlPct.toFixed(2)}%
                </div>
              </div>

              {/* Time */}
              <div className="text-[10px] text-text-secondary font-mono min-w-[55px] text-right">
                {formatTime(rt.exitTime)}
              </div>
            </div>
          ))}

          {/* Unpaired fills (open trades entered today) */}
          {openTrades > 0 && filled.filter((o) => {
            const rtSymbols = new Set(roundTrips.map((rt) => rt.symbol));
            return !rtSymbols.has(o.symbol) ||
              filled.filter((f) => f.symbol === o.symbol).length % 2 !== 0;
          }).reduce<Order[]>((acc, o) => {
            if (!acc.find((a) => a.symbol === o.symbol && a.side === o.side)) acc.push(o);
            return acc;
          }, []).map((o) => (
            <div
              key={o.order_id}
              className="flex items-center gap-3 bg-surface-800 rounded-lg px-3 py-2 opacity-60"
            >
              <div className="w-1 h-8 rounded-full flex-shrink-0 bg-accent" />
              <div className="min-w-[80px]">
                <Link
                  href={`/quote/${o.symbol}`}
                  className="text-sm font-semibold text-text-primary hover:text-accent transition-colors"
                >
                  {o.symbol}
                </Link>
                <div className="text-[10px] text-text-secondary truncate max-w-[100px]">
                  {o.strategy_name}
                </div>
              </div>
              <span className="text-[10px] font-semibold font-mono uppercase tracking-wider px-1.5 py-0.5 rounded bg-accent/15 text-accent-light">
                open
              </span>
              <div className="flex-1 text-center text-xs font-mono tabular-nums text-text-secondary">
                {o.side.toUpperCase()} @ ${o.filled_price?.toFixed(2) ?? "—"}
              </div>
              <div className="text-[10px] text-text-secondary font-mono min-w-[55px] text-right">
                {formatTime(o.filled_at || orderTime(o))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
