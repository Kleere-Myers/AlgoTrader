"use client";

import Link from "next/link";
import type { CompanyInfo } from "@/types";

function TriangleUp({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" className={className}>
      <path d="M8 4l5 8H3z" />
    </svg>
  );
}

function TriangleDown({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" className={className}>
      <path d="M8 12L3 4h10z" />
    </svg>
  );
}

interface WatchlistTableProps {
  symbols: CompanyInfo[];
  loading?: boolean;
}

export default function WatchlistTable({ symbols, loading }: WatchlistTableProps) {
  const sorted = [...symbols].sort((a, b) => (b.change_pct ?? 0) - (a.change_pct ?? 0));

  return (
    <div className="bg-surface-800 rounded-lg border border-surface-600 overflow-hidden">
      <table className="w-full text-sm">
        <thead>
          <tr className="bg-surface-800 text-text-secondary uppercase text-xs">
            <th className="text-left px-3 py-2 font-medium">Symbol</th>
            <th className="text-right px-3 py-2 font-medium">Price</th>
            <th className="text-right px-3 py-2 font-medium">Change</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-surface-600">
          {loading
            ? Array.from({ length: 6 }).map((_, i) => (
                <tr key={i}>
                  <td colSpan={3} className="px-3 py-2.5">
                    <div className="h-4 bg-surface-700 rounded animate-pulse" />
                  </td>
                </tr>
              ))
            : sorted.map((info) => {
                const isPositive = (info.change_pct ?? 0) >= 0;
                const changeColor = isPositive ? "text-gain" : "text-loss";

                return (
                  <tr key={info.symbol} className="hover:bg-surface-700 transition-colors">
                    <td className="px-3 py-2">
                      <Link href={`/quote/${info.symbol}`} className="hover:text-accent-light transition-colors">
                        <div className="font-semibold text-text-primary text-xs">{info.symbol}</div>
                        {info.name && info.name !== info.symbol && (
                          <div className="text-[10px] text-text-secondary truncate max-w-[120px]">{info.name}</div>
                        )}
                      </Link>
                    </td>
                    <td className="text-right px-3 py-2 text-xs font-medium text-text-primary font-mono tabular-nums">
                      {info.current_price != null ? `$${info.current_price.toFixed(2)}` : "—"}
                    </td>
                    <td className="text-right px-3 py-2">
                      {info.change_pct != null ? (
                        <div className={`flex items-center justify-end gap-0.5 ${changeColor}`}>
                          {isPositive ? (
                            <TriangleUp className="w-2.5 h-2.5" />
                          ) : (
                            <TriangleDown className="w-2.5 h-2.5" />
                          )}
                          <span className="text-xs font-medium font-mono tabular-nums">
                            {isPositive ? "+" : ""}{info.change_pct.toFixed(2)}%
                          </span>
                        </div>
                      ) : (
                        <span className="text-xs text-text-secondary">—</span>
                      )}
                    </td>
                  </tr>
                );
              })}
          {!loading && sorted.length === 0 && (
            <tr>
              <td colSpan={3} className="px-3 py-4 text-center text-xs text-text-secondary">
                No symbols in watchlist
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
