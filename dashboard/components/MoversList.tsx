"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import type { MarketMover } from "@/types";

interface MoversListProps {
  gainers: MarketMover[];
  losers: MarketMover[];
}

export default function MoversList({ gainers, losers }: MoversListProps) {
  const [activeTab, setActiveTab] = useState<"gainers" | "losers">(
    gainers.length > 0 ? "gainers" : "losers"
  );

  // Auto-switch to tab with data when props update
  useEffect(() => {
    if (gainers.length === 0 && losers.length > 0) setActiveTab("losers");
    else if (losers.length === 0 && gainers.length > 0) setActiveTab("gainers");
  }, [gainers.length, losers.length]);

  const items = activeTab === "gainers" ? gainers.slice(0, 5) : losers.slice(0, 5);

  return (
    <div className="bg-navy-800 rounded-lg border border-navy-600 overflow-hidden">
      {/* Tabs */}
      <div className="flex border-b border-navy-600">
        <button
          onClick={() => setActiveTab("gainers")}
          className={`flex-1 px-4 py-2.5 text-xs font-semibold transition-colors ${
            activeTab === "gainers"
              ? "text-white bg-accent-purple/20 border-b-2 border-accent-purple"
              : "text-text-secondary hover:text-text-primary"
          }`}
        >
          Top Gainers
        </button>
        <button
          onClick={() => setActiveTab("losers")}
          className={`flex-1 px-4 py-2.5 text-xs font-semibold transition-colors ${
            activeTab === "losers"
              ? "text-white bg-accent-purple/20 border-b-2 border-accent-purple"
              : "text-text-secondary hover:text-text-primary"
          }`}
        >
          Top Losers
        </button>
      </div>

      {/* List */}
      <div className="divide-y divide-navy-600">
        {items.map((mover) => {
          const isPositive = mover.change_pct >= 0;
          return (
            <div
              key={mover.symbol}
              className="flex items-center justify-between px-4 py-2.5"
            >
              <Link href={`/quote/${mover.symbol}`} className="flex items-center gap-2 min-w-0 hover:text-accent-purple-light transition-colors">
                <span className="text-sm font-bold text-text-primary">
                  {mover.symbol}
                </span>
                <span className="text-xs text-text-secondary truncate">
                  {mover.name}
                </span>
              </Link>
              <span
                className={`text-xs font-semibold px-2 py-0.5 rounded-full shrink-0 ${
                  isPositive
                    ? "bg-gain/15 text-gain"
                    : "bg-loss/15 text-loss"
                }`}
              >
                {isPositive ? "+" : ""}
                {mover.change_pct.toFixed(2)}%
              </span>
            </div>
          );
        })}
        {items.length === 0 && (
          <p className="text-xs text-text-secondary text-center py-4">
            No data available
          </p>
        )}
      </div>
    </div>
  );
}
