"use client";

import { useEffect, useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import type { Strategy } from "@/types";
import { strategyApi } from "@/lib/api";
import StrategyCard from "@/components/StrategyCard";
import { useSymbols } from "@/hooks/useSymbols";

export default function StrategiesPage() {
  const { symbols } = useSymbols();
  const [strategies, setStrategies] = useState<Strategy[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const router = useRouter();

  const fetchStrategies = useCallback(async () => {
    try {
      const data = await strategyApi.getStrategies();
      setStrategies(data);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load strategies");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchStrategies();
  }, [fetchStrategies]);

  if (loading) {
    return (
      <div>
        <h2 className="text-lg font-semibold text-text-primary mb-4">Strategies</h2>
        <p className="text-text-secondary text-sm">Loading strategies...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div>
        <h2 className="text-lg font-semibold text-text-primary mb-4">Strategies</h2>
        <div className="rounded-lg border border-loss/30 bg-loss/10 p-4 text-loss text-sm">
          {error}
        </div>
        <button
          onClick={fetchStrategies}
          className="mt-3 text-sm px-3 py-1.5 rounded bg-accent text-white hover:bg-accent-dark"
        >
          Retry
        </button>
      </div>
    );
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-lg font-semibold text-text-primary">Strategies</h2>
          <p className="text-text-secondary text-sm mt-1">
            Enable/disable strategies, edit parameters, and trigger backtest runs.
          </p>
        </div>
        <button
          onClick={fetchStrategies}
          className="text-xs px-3 py-1.5 rounded border border-surface-600 hover:bg-surface-700 text-text-secondary"
        >
          Refresh
        </button>
      </div>

      {strategies.length === 0 ? (
        <div className="rounded border border-dashed border-surface-600 p-12 text-center text-text-secondary">
          No strategies registered in the strategy engine
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {strategies.map((s) => (
            <StrategyCard
              key={s.id}
              strategy={s}
              symbols={symbols}
              onUpdate={fetchStrategies}
              onBacktestComplete={() => router.push("/backtest")}
            />
          ))}
        </div>
      )}
    </div>
  );
}
