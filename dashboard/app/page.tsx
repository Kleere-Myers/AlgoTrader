"use client";

import { useEffect, useState } from "react";
import { executionApi } from "@/lib/api";
import { useSseEvents } from "@/hooks/useSseEvents";
import type { AccountInfo, Position } from "@/types";

export default function OverviewPage() {
  const [account, setAccount] = useState<AccountInfo | null>(null);
  const [positions, setPositions] = useState<Position[]>([]);
  const [error, setError] = useState<string | null>(null);
  const { events, isConnected, isTradingHalted } = useSseEvents();

  useEffect(() => {
    const load = async () => {
      try {
        const [acct, pos] = await Promise.all([
          executionApi.getAccount(),
          executionApi.getPositions(),
        ]);
        setAccount(acct);
        setPositions(pos);
      } catch (e: any) {
        setError(e.message);
      }
    };
    load();
    const interval = setInterval(load, 10_000);
    return () => clearInterval(interval);
  }, []);

  // Refresh positions on OrderFill events
  useEffect(() => {
    const fills = events.filter(
      (e) => e.event_type === "ORDER_FILL"
    );
    if (fills.length > 0) {
      executionApi.getPositions().then(setPositions).catch(() => {});
      executionApi.getAccount().then(setAccount).catch(() => {});
    }
  }, [events]);

  return (
    <div>
      {isTradingHalted && (
        <div className="mb-4 rounded-lg bg-red-100 border border-red-500 px-4 py-2 text-red-800 text-sm font-semibold">
          TRADING HALTED
        </div>
      )}

      <h2 className="text-2xl font-bold mb-4">Overview</h2>

      {error && (
        <p className="text-red-500 text-sm mb-4">
          Failed to load: {error} — is the execution engine running?
        </p>
      )}

      <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
        <StatCard label="Equity" value={account ? `$${account.equity.toLocaleString()}` : "--"} />
        <StatCard label="Buying Power" value={account ? `$${account.buying_power.toLocaleString()}` : "--"} />
        <StatCard label="Cash" value={account ? `$${account.cash.toLocaleString()}` : "--"} />
        <StatCard
          label="SSE Stream"
          value={isConnected ? "Connected" : "Disconnected"}
          color={isConnected ? "text-green-600" : "text-gray-400"}
        />
      </div>

      <h3 className="text-lg font-semibold mb-3">Open Positions</h3>
      {positions.length === 0 ? (
        <p className="text-gray-400 text-sm">No open positions</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm text-left border border-gray-200 bg-white rounded-lg">
            <thead className="bg-gray-50 text-gray-500 uppercase text-xs">
              <tr>
                <th className="px-4 py-3">Symbol</th>
                <th className="px-4 py-3">Qty</th>
                <th className="px-4 py-3">Avg Entry</th>
                <th className="px-4 py-3">Current</th>
                <th className="px-4 py-3">Unrealized P&amp;L</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {positions.map((p) => (
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
              ))}
            </tbody>
          </table>
        </div>
      )}

      <h3 className="text-lg font-semibold mt-6 mb-3">Recent Events</h3>
      <div className="rounded-lg border border-gray-200 bg-white p-4 h-48 overflow-y-auto font-mono text-xs">
        {events.length === 0 ? (
          <span className="text-gray-400">Waiting for events...</span>
        ) : (
          events.slice(0, 50).map((e, i) => (
            <div key={i} className="mb-1">
              <span className="text-gray-400">{e.timestamp}</span>{" "}
              <span className="font-semibold">{e.event_type}</span>{" "}
              <span className="text-gray-500">{JSON.stringify(e.payload)}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function StatCard({
  label,
  value,
  color,
}: {
  label: string;
  value: string;
  color?: string;
}) {
  return (
    <div className="rounded-lg border border-gray-200 bg-white p-4 shadow-sm">
      <p className="text-xs text-gray-400 uppercase tracking-wide">{label}</p>
      <p className={`mt-1 text-2xl font-semibold ${color || "text-gray-900"}`}>
        {value}
      </p>
    </div>
  );
}
