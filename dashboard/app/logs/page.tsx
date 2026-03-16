"use client";

import { useEffect, useRef, useState } from "react";
import { useSseEvents } from "@/hooks/useSseEvents";
import type { SseEvent, SseEventType } from "@/types";

const EVENT_COLORS: Record<SseEventType, string> = {
  ORDER_FILL: "bg-green-100 text-green-700",
  RISK_BREACH: "bg-red-100 text-red-700",
  TRADING_HALTED: "bg-red-100 text-red-700",
  TRADING_RESUMED: "bg-green-100 text-green-700",
  POSITION_UPDATE: "bg-blue-100 text-blue-700",
  RISK_CONFIG_UPDATED: "bg-yellow-100 text-yellow-700",
  DAILY_PNL: "bg-gray-100 text-gray-500",
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
          <h2 className="text-2xl font-bold">Logs</h2>
          <p className="text-gray-500 text-sm mt-1">
            Real-time signal and order event stream from the execution engine.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={() => setAutoScroll(!autoScroll)}
            className={`text-xs px-3 py-1.5 rounded border ${
              autoScroll
                ? "border-blue-200 bg-blue-50 text-blue-600"
                : "border-gray-200 text-gray-600 hover:bg-gray-50"
            }`}
          >
            {autoScroll ? "Auto-scroll: ON" : "Auto-scroll: OFF"}
          </button>
          <button
            onClick={handleClear}
            className="text-xs px-3 py-1.5 rounded border border-gray-200 hover:bg-gray-50 text-gray-600"
          >
            Clear
          </button>
        </div>
      </div>

      <div className="rounded-lg border border-gray-200 bg-white shadow-sm overflow-hidden">
        {/* Status bar */}
        <div className="px-4 py-3 bg-gray-50 border-b border-gray-200 flex items-center justify-between">
          <span className="text-xs text-gray-500 uppercase tracking-wide">
            SSE Event Stream
          </span>
          <div className="flex items-center gap-3">
            <span className="text-xs text-gray-400">{displayEvents.length} events</span>
            <span className="inline-flex items-center gap-1.5 text-xs">
              <span
                className={`w-2 h-2 rounded-full ${
                  isConnected ? "bg-green-500" : "bg-gray-300"
                }`}
              />
              <span className={isConnected ? "text-green-600" : "text-gray-400"}>
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
            <div className="text-gray-400 text-center py-12">Waiting for events...</div>
          ) : (
            <div className="space-y-0.5">
              {orderedEvents.map((event, i) => {
                const colorClass = EVENT_COLORS[event.event_type] || "bg-gray-100 text-gray-500";
                return (
                  <div
                    key={i}
                    className="flex items-start gap-2 py-1 px-1 rounded hover:bg-gray-50"
                  >
                    <span className="text-gray-400 shrink-0 w-16">
                      {formatTimestamp(event.timestamp)}
                    </span>
                    <span
                      className={`shrink-0 px-1.5 py-0.5 rounded text-[10px] font-medium leading-tight ${colorClass}`}
                    >
                      {event.event_type}
                    </span>
                    <span className="text-gray-500 truncate">
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
