"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { executionApi } from "@/lib/api";
import { useSseEvents } from "@/hooks/useSseEvents";
import Tip from "@/components/Tip";
import type { Position } from "@/types";

export default function PositionsPage() {
  const [positions, setPositions] = useState<Position[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [flattenConfirm, setFlattenConfirm] = useState(false);
  const [flattenLoading, setFlattenLoading] = useState(false);
  const [closingSymbol, setClosingSymbol] = useState<string | null>(null);
  const { events } = useSseEvents();

  const loadPositions = async () => {
    try {
      const pos = await executionApi.getPositions();
      setPositions(pos);
      setError(null);
    } catch (e: any) {
      setError(e.message);
    }
  };

  useEffect(() => {
    loadPositions();
    const interval = setInterval(loadPositions, 10_000);
    return () => clearInterval(interval);
  }, []);

  // Refresh on position-related SSE events
  useEffect(() => {
    const relevant = events.filter(
      (e) =>
        e.event_type === "POSITION_UPDATE" ||
        e.event_type === "ORDER_FILL"
    );
    if (relevant.length > 0) {
      loadPositions();
    }
  }, [events]);

  const handleFlatten = async () => {
    if (!flattenConfirm) {
      setFlattenConfirm(true);
      return;
    }
    setFlattenLoading(true);
    try {
      await executionApi.flattenDayPositions();
      await loadPositions();
    } catch (e: any) {
      setError(e.message);
    } finally {
      setFlattenLoading(false);
      setFlattenConfirm(false);
    }
  };

  const handleClose = async (symbol: string) => {
    setClosingSymbol(symbol);
    try {
      await executionApi.closePosition(symbol);
      await loadPositions();
    } catch (e: any) {
      setError(e.message);
    } finally {
      setClosingSymbol(null);
    }
  };

  const totalPnl = positions.reduce((sum, p) => sum + p.unrealized_pnl, 0);
  const totalValue = positions.reduce((sum, p) => sum + p.current_price * p.qty, 0);
  const dayPositions = positions.filter((p) => p.trade_type === "day" || !p.trade_type);
  const swingPositions = positions.filter((p) => p.trade_type === "swing");

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-text-primary">Positions</h2>
        <div className="flex items-center gap-4">
          {positions.length > 0 && (
            <>
              <span className="text-sm text-text-secondary">
                Value: <span className="text-text-primary font-medium font-mono tabular-nums">${totalValue.toFixed(2)}</span>
              </span>
              <span
                className={`text-lg font-semibold ${
                  totalPnl >= 0 ? "text-gain" : "text-loss"
                }`}
              >
                P&L: {totalPnl >= 0 ? "+" : ""}${totalPnl.toFixed(2)}
              </span>
            </>
          )}
          {dayPositions.length > 0 && (
            <>
              {flattenConfirm ? (
                <div className="flex items-center gap-2">
                  <span className="text-xs text-loss">Close {dayPositions.length} day position{dayPositions.length !== 1 ? "s" : ""}?</span>
                  <button
                    onClick={handleFlatten}
                    disabled={flattenLoading}
                    className="bg-loss text-white text-xs font-medium px-3 py-1.5 rounded-md hover:bg-red-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {flattenLoading ? "Closing..." : "Confirm"}
                  </button>
                  <button
                    onClick={() => setFlattenConfirm(false)}
                    className="text-text-secondary text-xs font-medium px-3 py-1.5 rounded border border-surface-600 hover:bg-surface-700 hover:text-text-primary transition-colors"
                  >
                    Cancel
                  </button>
                </div>
              ) : (
                <button
                  onClick={handleFlatten}
                  className="text-text-secondary text-xs font-medium px-3 py-1.5 rounded border border-surface-600 hover:bg-loss/10 hover:text-loss hover:border-loss/30 transition-colors"
                >
                  Flatten All Day
                </button>
              )}
            </>
          )}
        </div>
      </div>

      {error && (
        <p className="text-loss text-sm mb-4">
          Failed to load: {error} — is the execution engine running?
        </p>
      )}

      {/* Summary badges */}
      {positions.length > 0 && (
        <div className="flex gap-3 mb-4">
          <span className="text-xs px-2.5 py-1 rounded-full bg-accent/15 text-accent-light font-medium">
            {positions.length} position{positions.length !== 1 ? "s" : ""}
          </span>
          {dayPositions.length > 0 && (
            <span className="text-xs px-2.5 py-1 rounded-full bg-surface-700 text-text-secondary font-medium">
              {dayPositions.length} day
            </span>
          )}
          {swingPositions.length > 0 && (
            <span className="text-xs px-2.5 py-1 rounded-full bg-surface-700 text-text-secondary font-medium">
              {swingPositions.length} swing
            </span>
          )}
        </div>
      )}

      <p className="text-[10px] text-text-secondary mb-3">
        Prices update every ~15s from Alpaca latest trades during extended hours (4 AM – 8 PM ET).
      </p>

      <div className="overflow-x-auto">
        <table className="w-full text-sm text-left border border-surface-600 bg-surface-900 rounded-lg">
          <thead className="bg-surface-800 text-text-secondary uppercase text-xs">
            <tr>
              <th className="px-4 py-3">Symbol</th>
              <th className="px-4 py-3">Side</th>
              <th className="px-4 py-3">Type</th>
              <th className="px-4 py-3">Qty <Tip text="Number of shares held." inline /></th>
              <th className="px-4 py-3">Avg Entry <Tip text="Average price paid per share." inline /></th>
              <th className="px-4 py-3">Last Price <Tip text="Price at last fill — not a live quote." inline /></th>
              <th className="px-4 py-3">Market Value <Tip text="Current shares × last known price." inline /></th>
              <th className="px-4 py-3">P&L <Tip text="Unrealized profit or loss based on last fill price." inline /></th>
              <th className="px-4 py-3">P&L % <Tip text="Percentage gain or loss from entry price." inline /></th>
              <th className="px-4 py-3">Stop Loss <Tip text="Auto-sell trigger if price drops to this level." inline /></th>
              <th className="px-4 py-3">Take Profit <Tip text="Auto-sell trigger if price rises to this level." inline /></th>
              <th className="px-4 py-3 w-16"></th>
            </tr>
          </thead>
          <tbody className="divide-y divide-surface-600">
            {positions.length === 0 ? (
              <tr>
                <td
                  className="px-4 py-8 text-center text-text-secondary"
                  colSpan={12}
                >
                  No open positions
                </td>
              </tr>
            ) : (
              positions.map((p) => {
                const marketValue = p.current_price * p.qty;
                const isShort = p.side === "short";
                const pnlPct =
                  p.avg_entry_price > 0
                    ? isShort
                      ? ((p.avg_entry_price - p.current_price) / p.avg_entry_price) * 100
                      : ((p.current_price - p.avg_entry_price) / p.avg_entry_price) * 100
                    : 0;
                const isPositive = p.unrealized_pnl >= 0;
                const pnlColor = isPositive ? "text-gain" : "text-loss";
                const tradeType = p.trade_type || "day";
                const isClosing = closingSymbol === p.symbol;

                // Distance to stop/take as percentage from current
                const stopDist =
                  p.stop_loss_price != null && p.current_price > 0
                    ? ((p.stop_loss_price - p.current_price) / p.current_price) * 100
                    : null;
                const takeDist =
                  p.take_profit_price != null && p.current_price > 0
                    ? ((p.take_profit_price - p.current_price) / p.current_price) * 100
                    : null;

                return (
                  <tr key={p.symbol} className="hover:bg-surface-700 transition-colors">
                    <td className="px-4 py-3">
                      <Link
                        href={`/quote/${p.symbol}`}
                        className="font-semibold text-text-primary hover:text-accent-light transition-colors"
                      >
                        {p.symbol}
                      </Link>
                    </td>
                    <td className="px-4 py-3">
                      <span
                        className={`text-[10px] font-semibold font-mono uppercase tracking-wider px-1.5 py-0.5 rounded ${
                          isShort
                            ? "bg-loss/10 text-loss"
                            : "bg-gain/10 text-gain"
                        }`}
                      >
                        {isShort ? "short" : "long"}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <span
                        className={`text-[10px] font-semibold uppercase px-1.5 py-0.5 rounded ${
                          tradeType === "swing"
                            ? "bg-accent/15 text-accent-light"
                            : "bg-surface-700 text-text-secondary"
                        }`}
                      >
                        {tradeType}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-text-primary font-mono tabular-nums">{p.qty}</td>
                    <td className="px-4 py-3 text-text-secondary font-mono tabular-nums">
                      ${p.avg_entry_price.toFixed(2)}
                    </td>
                    <td className="px-4 py-3 text-text-primary font-mono tabular-nums">
                      ${p.current_price.toFixed(2)}
                    </td>
                    <td className="px-4 py-3 text-text-primary font-mono tabular-nums">
                      ${marketValue.toFixed(2)}
                    </td>
                    <td className={`px-4 py-3 font-medium font-mono tabular-nums ${pnlColor}`}>
                      {isPositive ? "+" : ""}${p.unrealized_pnl.toFixed(2)}
                    </td>
                    <td className={`px-4 py-3 font-medium font-mono tabular-nums ${pnlColor}`}>
                      {isPositive ? "+" : ""}{pnlPct.toFixed(2)}%
                    </td>
                    <td className="px-4 py-3 font-mono tabular-nums">
                      {p.stop_loss_price != null ? (
                        <div>
                          <span className="text-text-primary">${p.stop_loss_price.toFixed(2)}</span>
                          {stopDist != null && (
                            <span className="text-[10px] text-text-secondary ml-1">
                              ({stopDist > 0 ? "+" : ""}{stopDist.toFixed(1)}%)
                            </span>
                          )}
                        </div>
                      ) : (
                        <span className="text-text-secondary">—</span>
                      )}
                    </td>
                    <td className="px-4 py-3 font-mono tabular-nums">
                      {p.take_profit_price != null ? (
                        <div>
                          <span className="text-text-primary">${p.take_profit_price.toFixed(2)}</span>
                          {takeDist != null && (
                            <span className="text-[10px] text-text-secondary ml-1">
                              (+{takeDist.toFixed(1)}%)
                            </span>
                          )}
                        </div>
                      ) : (
                        <span className="text-text-secondary">—</span>
                      )}
                    </td>
                    <td className="px-4 py-3">
                      <button
                        onClick={() => handleClose(p.symbol)}
                        disabled={isClosing || closingSymbol !== null}
                        className="text-text-secondary text-xs font-medium px-2 py-1 rounded border border-surface-600 hover:bg-loss/10 hover:text-loss hover:border-loss/30 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                        title={`Close ${p.symbol} position`}
                      >
                        {isClosing ? "..." : "Close"}
                      </button>
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
