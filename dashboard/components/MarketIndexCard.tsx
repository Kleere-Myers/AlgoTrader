"use client";

import type { MarketIndex } from "@/types";

interface MarketIndexCardProps {
  index: MarketIndex;
}

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

export default function MarketIndexCard({ index }: MarketIndexCardProps) {
  const isPositive = index.change_abs >= 0;
  const changeColor = isPositive ? "text-gain" : "text-loss";
  const sign = isPositive ? "+" : "";

  const formatPrice = (price: number) => {
    if (price >= 1000) return price.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    if (price >= 1) return price.toFixed(2);
    return price.toFixed(4);
  };

  return (
    <div className="bg-surface-900 rounded-lg px-3.5 py-3 border border-surface-600 min-w-[155px] flex-shrink-0 hover:bg-surface-700 hover:border-surface-500 transition-all duration-150">
      <p className="text-[11px] font-medium text-text-secondary leading-none truncate">{index.name}</p>
      <p className="text-[15px] font-semibold text-text-primary mt-1 leading-tight font-mono tabular-nums">
        {formatPrice(index.current_price)}
      </p>
      <div className={`flex items-center gap-0.5 mt-0.5 ${changeColor}`}>
        {isPositive ? (
          <TriangleUp className="w-3 h-3" />
        ) : (
          <TriangleDown className="w-3 h-3" />
        )}
        <span className="text-xs font-medium font-mono tabular-nums">
          {sign}{index.change_abs.toFixed(2)} ({sign}{index.change_pct.toFixed(2)}%)
        </span>
      </div>
    </div>
  );
}
