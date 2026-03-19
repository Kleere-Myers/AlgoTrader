"use client";

import { useEffect, useState, useCallback } from "react";
import type { Order } from "@/types";
import { executionApi } from "@/lib/api";
import Tip from "@/components/Tip";

function formatTime(iso: string | null): string {
  if (!iso) return "\u2014";
  const d = new Date(iso);
  return d.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit",
    hour12: true,
  });
}

function statusBadge(status: string) {
  const s = status.toLowerCase();
  const styles: Record<string, string> = {
    filled: "bg-gain/15 text-gain",
    pending: "bg-yellow-500/15 text-yellow-500",
    new: "bg-yellow-500/15 text-yellow-500",
    partially_filled: "bg-yellow-500/15 text-yellow-500",
    cancelled: "bg-navy-600 text-text-secondary",
    canceled: "bg-navy-600 text-text-secondary",
    rejected: "bg-loss/15 text-loss",
  };
  const style = styles[s] || "bg-navy-600 text-text-secondary";
  return (
    <span className={`text-xs px-2 py-0.5 rounded font-medium ${style}`}>
      {status}
    </span>
  );
}

export default function OrdersPage() {
  const [orders, setOrders] = useState<Order[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchOrders = useCallback(async () => {
    try {
      const data = await executionApi.getOrders();
      setOrders(data.slice(0, 100));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load orders");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchOrders();
  }, [fetchOrders]);

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-2xl font-bold text-text-primary">Orders</h2>
          <p className="text-text-secondary text-sm mt-1">
            Recent order history with fill prices, status, and strategy attribution.
          </p>
        </div>
        <button
          onClick={fetchOrders}
          className="text-xs px-3 py-1.5 rounded border border-navy-600 hover:bg-navy-700 text-text-secondary"
        >
          Refresh
        </button>
      </div>

      {error && (
        <div className="rounded-lg border border-loss/30 bg-loss/10 p-4 text-loss text-sm mb-4">
          {error}
        </div>
      )}

      <div className="overflow-x-auto">
        <table className="w-full text-sm text-left border border-navy-600 bg-navy-900 rounded-lg">
          <thead className="bg-navy-800 text-text-secondary uppercase text-xs">
            <tr>
              <th className="px-4 py-3">Symbol</th>
              <th className="px-4 py-3">Side <Tip text="BUY means shares were purchased. SELL means shares were sold." inline /></th>
              <th className="px-4 py-3 text-right">Qty <Tip text="Number of shares in this order." inline /></th>
              <th className="px-4 py-3 text-right">Fill Price <Tip text="The actual price per share when the order was executed. May differ slightly from the price when the signal was generated (this difference is called slippage)." inline /></th>
              <th className="px-4 py-3">Status <Tip text="'Filled' means the trade completed. 'Pending' means it's waiting. 'Rejected' means the risk rules blocked it." inline /></th>
              <th className="px-4 py-3">Strategy <Tip text="Which trading strategy triggered this order." inline /></th>
              <th className="px-4 py-3">Submitted</th>
              <th className="px-4 py-3">Filled</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-navy-600">
            {loading ? (
              <tr>
                <td className="px-4 py-8 text-center text-text-secondary" colSpan={8}>
                  Loading orders...
                </td>
              </tr>
            ) : orders.length === 0 ? (
              <tr>
                <td className="px-4 py-8 text-center text-text-secondary" colSpan={8}>
                  No orders yet
                </td>
              </tr>
            ) : (
              orders.map((o) => (
                <tr key={o.order_id} className="hover:bg-navy-800">
                  <td className="px-4 py-2.5 font-medium text-text-primary">{o.symbol}</td>
                  <td className="px-4 py-2.5">
                    <span
                      className={`font-medium ${
                        o.side === "buy" ? "text-gain" : "text-loss"
                      }`}
                    >
                      {o.side.toUpperCase()}
                    </span>
                  </td>
                  <td className="px-4 py-2.5 text-right text-text-secondary">{o.qty}</td>
                  <td className="px-4 py-2.5 text-right text-text-secondary">
                    {o.filled_price !== null ? `$${o.filled_price.toFixed(2)}` : "\u2014"}
                  </td>
                  <td className="px-4 py-2.5">{statusBadge(o.status)}</td>
                  <td className="px-4 py-2.5 text-text-secondary">{o.strategy_name}</td>
                  <td className="px-4 py-2.5 text-text-secondary text-xs">{formatTime(o.submitted_at)}</td>
                  <td className="px-4 py-2.5 text-text-secondary text-xs">{formatTime(o.filled_at)}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
