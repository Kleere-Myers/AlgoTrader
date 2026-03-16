"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import type { SseEvent } from "@/types";
import { executionApi } from "@/lib/api";

export function useSseEvents() {
  const [events, setEvents] = useState<SseEvent[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const [isTradingHalted, setIsTradingHalted] = useState(false);
  const esRef = useRef<EventSource | null>(null);

  const connect = useCallback(() => {
    if (esRef.current) {
      esRef.current.close();
    }

    const es = new EventSource(executionApi.sseUrl);
    esRef.current = es;

    es.onopen = () => setIsConnected(true);

    es.onmessage = (e) => {
      try {
        const event: SseEvent = JSON.parse(e.data);
        setEvents((prev) => [event, ...prev].slice(0, 200));

        if (event.event_type === "TRADING_HALTED") {
          setIsTradingHalted(true);
        }
        if (event.event_type === "TRADING_RESUMED") {
          setIsTradingHalted(false);
        }
      } catch {
        // ignore malformed events
      }
    };

    es.onerror = () => {
      setIsConnected(false);
      es.close();
      // Reconnect after 3 seconds
      setTimeout(connect, 3000);
    };
  }, []);

  useEffect(() => {
    connect();
    return () => {
      esRef.current?.close();
    };
  }, [connect]);

  return { events, isConnected, isTradingHalted };
}
