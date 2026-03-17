"use client";

import { useEffect, useState } from "react";
import { executionApi } from "@/lib/api";
import { useSseEvents } from "@/hooks/useSseEvents";
import Tip from "@/components/Tip";
import type { Position } from "@/types";

export default function PositionsPage() {
  const [positions, setPositions] = useState<Position[]>([]);
  const [error, setError] = useState<string | null>(null);
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

  const totalPnl = positions.reduce((sum, p) => sum + p.unrealized_pnl, 0);

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-2xl font-bold">Positions</h2>
        {positions.length > 0 && (
          <span
            className={`text-lg font-semibold ${
              totalPnl >= 0 ? "text-green-600" : "text-red-600"
            }`}
          >
            Total P&amp;L: ${totalPnl.toFixed(2)}
          </span>
        )}
      </div>

      {error && (
        <p className="text-red-500 text-sm mb-4">
          Failed to load: {error} — is the execution engine running?
        </p>
      )}

      <div className="overflow-x-auto">
        <table className="w-full text-sm text-left border border-gray-200 bg-white rounded-lg">
          <thead className="bg-gray-50 text-gray-500 uppercase text-xs">
            <tr>
              <th className="px-4 py-3">Symbol</th>
              <th className="px-4 py-3">Qty <Tip text="Number of shares you own of this stock." inline /></th>
              <th className="px-4 py-3">Avg Entry Price <Tip text="The average price you paid per share when buying." inline /></th>
              <th className="px-4 py-3">Current Price <Tip text="What the stock is worth right now per share." inline /></th>
              <th className="px-4 py-3">Unrealized P&amp;L <Tip text="Profit or loss if you sold right now. Green means you're up, red means you're down. 'Unrealized' because you haven't sold yet." inline /></th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100">
            {positions.length === 0 ? (
              <tr>
                <td
                  className="px-4 py-8 text-center text-gray-400"
                  colSpan={5}
                >
                  No open positions
                </td>
              </tr>
            ) : (
              positions.map((p) => (
                <tr key={p.symbol}>
                  <td className="px-4 py-3 font-medium">{p.symbol}</td>
                  <td className="px-4 py-3">{p.qty}</td>
                  <td className="px-4 py-3">${p.avg_entry_price.toFixed(2)}</td>
                  <td className="px-4 py-3">${p.current_price.toFixed(2)}</td>
                  <td
                    className={`px-4 py-3 font-medium ${
                      p.unrealized_pnl >= 0 ? "text-green-600" : "text-red-600"
                    }`}
                  >
                    ${p.unrealized_pnl.toFixed(2)}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
