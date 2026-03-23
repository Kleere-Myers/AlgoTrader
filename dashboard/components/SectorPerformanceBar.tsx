"use client";

import type { SectorPerformance } from "@/types";

interface SectorPerformanceBarProps {
  sectors: SectorPerformance[];
}

export default function SectorPerformanceBar({
  sectors,
}: SectorPerformanceBarProps) {
  const sorted = [...sectors].sort((a, b) => b.change_pct - a.change_pct);
  const maxAbs = Math.max(...sorted.map((s) => Math.abs(s.change_pct)), 0.01);

  return (
    <div className="bg-surface-800 rounded-lg p-3 border border-surface-600">
      <div className="space-y-1.5">
        {sorted.map((sector) => {
          const isPositive = sector.change_pct >= 0;
          const barWidthPct = (Math.abs(sector.change_pct) / maxAbs) * 50;

          return (
            <div key={sector.sector} className="flex items-center gap-2">
              <span className="text-xs text-text-secondary w-28 text-right shrink-0 truncate">
                {sector.sector}
              </span>
              <div className="flex-1 h-4 relative">
                {/* Center line */}
                <div className="absolute left-1/2 top-0 bottom-0 w-px bg-surface-600" />
                {/* Bar */}
                <div
                  className="absolute top-0.5 h-3 rounded-sm transition-all"
                  style={{
                    backgroundColor: isPositive ? "#34d399" : "#f87171",
                    width: `${barWidthPct}%`,
                    ...(isPositive
                      ? { left: "50%" }
                      : { right: "50%" }),
                  }}
                />
              </div>
              <span
                className={`text-xs font-medium w-14 text-right shrink-0 ${
                  isPositive ? "text-gain" : "text-loss"
                }`}
              >
                {isPositive ? "+" : ""}
                {sector.change_pct.toFixed(2)}%
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
