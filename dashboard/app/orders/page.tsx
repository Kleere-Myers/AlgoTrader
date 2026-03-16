"use client";

import { useEffect, useState, useCallback } from "react";
import type { Order } from "@/types";
import { executionApi } from "@/lib/api";

function formatTime(iso: string | null): string {
  if (!iso) return "—";
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
    filled: "bg-green-100 text-green-700",
    pending: "bg-yellow-100 text-yellow-700",
    new: "bg-yellow-100 text-yellow-700",
    partially_filled: "bg-yellow-100 text-yellow-700",
    cancelled: "bg-gray-100 text-gray-500",
    canceled: "bg-gray-100 text-gray-500",
    rejected: "bg-red-100 text-red-700",
  };
  const style = styles[s] || "bg-gray-100 text-gray-500";
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
          <h2 className="text-2xl font-bold">Orders</h2>
          <p className="text-gray-500 text-sm mt-1">
            Recent order history with fill prices, status, and strategy attribution.
          </p>
        </div>
        <button
          onClick={fetchOrders}
          className="text-xs px-3 py-1.5 rounded border border-gray-200 hover:bg-gray-50 text-gray-600"
        >
          Refresh
        </button>
      </div>

      {error && (
        <div className="rounded-lg border border-red-200 bg-red-50 p-4 text-red-700 text-sm mb-4">
          {error}
        </div>
      )}

      <div className="overflow-x-auto">
        <table className="w-full text-sm text-left border border-gray-200 bg-white rounded-lg">
          <thead className="bg-gray-50 text-gray-500 uppercase text-xs">
            <tr>
              <th className="px-4 py-3">Symbol</th>
              <th className="px-4 py-3">Side</th>
              <th className="px-4 py-3 text-right">Qty</th>
              <th className="px-4 py-3 text-right">Fill Price</th>
              <th className="px-4 py-3">Status</th>
              <th className="px-4 py-3">Strategy</th>
              <th className="px-4 py-3">Submitted</th>
              <th className="px-4 py-3">Filled</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100">
            {loading ? (
              <tr>
                <td className="px-4 py-8 text-center text-gray-400" colSpan={8}>
                  Loading orders...
                </td>
              </tr>
            ) : orders.length === 0 ? (
              <tr>
                <td className="px-4 py-8 text-center text-gray-400" colSpan={8}>
                  No orders yet
                </td>
              </tr>
            ) : (
              orders.map((o) => (
                <tr key={o.order_id} className="hover:bg-gray-50">
                  <td className="px-4 py-2.5 font-medium">{o.symbol}</td>
                  <td className="px-4 py-2.5">
                    <span
                      className={`font-medium ${
                        o.side === "buy" ? "text-green-600" : "text-red-600"
                      }`}
                    >
                      {o.side.toUpperCase()}
                    </span>
                  </td>
                  <td className="px-4 py-2.5 text-right">{o.qty}</td>
                  <td className="px-4 py-2.5 text-right">
                    {o.filled_price !== null ? `$${o.filled_price.toFixed(2)}` : "—"}
                  </td>
                  <td className="px-4 py-2.5">{statusBadge(o.status)}</td>
                  <td className="px-4 py-2.5 text-gray-500">{o.strategy_name}</td>
                  <td className="px-4 py-2.5 text-gray-500 text-xs">{formatTime(o.submitted_at)}</td>
                  <td className="px-4 py-2.5 text-gray-500 text-xs">{formatTime(o.filled_at)}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
