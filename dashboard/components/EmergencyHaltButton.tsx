"use client";

import { useState } from "react";
import { executionApi } from "@/lib/api";

interface EmergencyHaltButtonProps {
  isHalted: boolean;
  onToggle: () => void;
}

export default function EmergencyHaltButton({ isHalted, onToggle }: EmergencyHaltButtonProps) {
  const [confirming, setConfirming] = useState(false);
  const [loading, setLoading] = useState(false);

  const now = new Date();
  const etOffset = new Date().toLocaleString("en-US", { timeZone: "America/New_York" });
  const etHour = new Date(etOffset).getHours();
  const etMinute = new Date(etOffset).getMinutes();
  const etTime = etHour * 60 + etMinute;
  const marketOpen = 9 * 60 + 30; // 9:30 AM ET
  const marketClose = 16 * 60;     // 4:00 PM ET
  const isMarketHours = etTime >= marketOpen && etTime < marketClose && now.getDay() >= 1 && now.getDay() <= 5;

  const handleClick = async () => {
    if (!isHalted && !confirming) {
      setConfirming(true);
      return;
    }

    setLoading(true);
    try {
      if (isHalted) {
        await executionApi.resumeTrading();
      } else {
        await executionApi.haltTrading();
      }
      onToggle();
    } catch (e) {
      console.error("Failed to toggle trading halt:", e);
    } finally {
      setLoading(false);
      setConfirming(false);
    }
  };

  return (
    <div className="rounded-lg border border-loss/30 bg-loss/10 p-5">
      <h3 className="text-sm font-semibold text-loss mb-2">Emergency Trading Halt</h3>
      <p className="text-xs text-loss/80 mb-4">
        {isHalted
          ? "Trading is currently halted. Resume to allow order submission."
          : "Immediately halt all order submission. Open positions will NOT be closed automatically."}
      </p>

      {confirming && !isHalted && (
        <div className="mb-3 rounded border border-loss/40 bg-loss/15 p-3">
          <p className="text-sm text-loss font-medium mb-2">
            Are you sure you want to halt all trading?
          </p>
          <div className="flex gap-2">
            <button
              onClick={handleClick}
              disabled={loading}
              className="px-3 py-1.5 rounded bg-loss text-white text-sm font-semibold hover:bg-red-700 disabled:opacity-50"
            >
              {loading ? "Halting..." : "Yes, Halt Trading"}
            </button>
            <button
              onClick={() => setConfirming(false)}
              className="px-3 py-1.5 rounded border border-navy-600 text-sm text-text-secondary hover:bg-navy-700"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {!confirming && (
        <div className="relative group inline-block">
          <button
            onClick={handleClick}
            disabled={loading || (!isHalted && !isMarketHours)}
            className={`px-4 py-2 rounded font-semibold text-sm text-white disabled:opacity-50 disabled:cursor-not-allowed ${
              isHalted
                ? "bg-gain hover:bg-green-700"
                : "bg-loss hover:bg-red-700"
            }`}
          >
            {loading
              ? isHalted ? "Resuming..." : "Halting..."
              : isHalted ? "Resume Trading" : "Halt Trading"}
          </button>
          {!isHalted && !isMarketHours && (
            <div className="absolute bottom-full left-0 mb-1 hidden group-hover:block bg-navy-800 text-text-primary text-xs rounded px-2 py-1 whitespace-nowrap">
              Market is closed (hours: 9:30 AM — 4:00 PM ET, Mon-Fri)
            </div>
          )}
        </div>
      )}
    </div>
  );
}
