"use client";

import { useEffect, useRef, useState } from "react";
import { useSseEvents } from "@/hooks/useSseEvents";
import type { SseEvent, SseEventType } from "@/types";

const EVENT_COLORS: Record<SseEventType, string> = {
  ORDER_FILL: "bg-gain/15 text-gain",
  RISK_BREACH: "bg-loss/15 text-loss",
  TRADING_HALTED: "bg-loss/15 text-loss",
  TRADING_RESUMED: "bg-gain/15 text-gain",
  POSITION_UPDATE: "bg-accent-purple/15 text-accent-purple-light",
  RISK_CONFIG_UPDATED: "bg-yellow-500/15 text-yellow-500",
  DAILY_PNL: "bg-navy-600 text-text-secondary",
};

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

function payloadSummary(payload: Record<string, unknown>): string {
  const entries = Object.entries(payload);
  if (entries.length === 0) return "";
  return entries
    .slice(0, 4)
    .map(([k, v]) => {
      const val = typeof v === "number" ? (Number.isInteger(v) ? v : (v as number).toFixed(2)) : String(v);
      return `${k}=${val}`;
    })
    .join("  ");
}

export default function LogsPage() {
  const { events, isConnected } = useSseEvents();
  const [displayEvents, setDisplayEvents] = useState<SseEvent[]>([]);
  const [autoScroll, setAutoScroll] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Sync SSE events into local display buffer
  useEffect(() => {
    setDisplayEvents(events.slice(0, 200));
  }, [events]);

  // Auto-scroll to bottom on new events
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [displayEvents, autoScroll]);

  const handleClear = () => {
    setDisplayEvents([]);
  };

  // Reverse so newest is at the bottom (events arrive newest-first from the hook)
  const orderedEvents = [...displayEvents].reverse();

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-2xl font-bold text-text-primary">Logs</h2>
          <p className="text-text-secondary text-sm mt-1">
            Real-time signal and order event stream from the execution engine.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={() => setAutoScroll(!autoScroll)}
            className={`text-xs px-3 py-1.5 rounded border ${
              autoScroll
                ? "border-accent-purple/40 bg-accent-purple/15 text-accent-purple-light"
                : "border-navy-600 text-text-secondary hover:bg-navy-700"
            }`}
          >
            {autoScroll ? "Auto-scroll: ON" : "Auto-scroll: OFF"}
          </button>
          <button
            onClick={handleClear}
            className="text-xs px-3 py-1.5 rounded border border-navy-600 hover:bg-navy-700 text-text-secondary"
          >
            Clear
          </button>
        </div>
      </div>

      <div className="rounded-lg border border-navy-600 bg-navy-900 overflow-hidden">
        {/* Status bar */}
        <div className="px-4 py-3 bg-navy-800 border-b border-navy-600 flex items-center justify-between">
          <span className="text-xs text-text-secondary uppercase tracking-wide">
            SSE Event Stream
          </span>
          <div className="flex items-center gap-3">
            <span className="text-xs text-text-secondary">{displayEvents.length} events</span>
            <span className="inline-flex items-center gap-1.5 text-xs">
              <span
                className={`w-2 h-2 rounded-full ${
                  isConnected ? "bg-gain" : "bg-gray-500"
                }`}
              />
              <span className={isConnected ? "text-gain" : "text-text-secondary"}>
                {isConnected ? "Connected" : "Disconnected"}
              </span>
            </span>
          </div>
        </div>

        {/* Event log */}
        <div
          ref={scrollRef}
          className="p-3 h-[calc(100vh-280px)] min-h-[400px] overflow-y-auto font-mono text-xs"
        >
          {orderedEvents.length === 0 ? (
            <div className="text-text-secondary text-center py-12">Waiting for events...</div>
          ) : (
            <div className="space-y-0.5">
              {orderedEvents.map((event, i) => {
                const colorClass = EVENT_COLORS[event.event_type] || "bg-navy-600 text-text-secondary";
                return (
                  <div
                    key={i}
                    className="flex items-start gap-2 py-1 px-1 rounded hover:bg-navy-800"
                  >
                    <span className="text-text-secondary shrink-0 w-16">
                      {formatTimestamp(event.timestamp)}
                    </span>
                    <span
                      className={`shrink-0 px-1.5 py-0.5 rounded text-[10px] font-medium leading-tight ${colorClass}`}
                    >
                      {event.event_type}
                    </span>
                    <span className="text-text-secondary truncate">
                      {payloadSummary(event.payload)}
                    </span>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
