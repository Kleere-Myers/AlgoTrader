"use client";

import { useEffect, useState, useCallback } from "react";
import type { RiskConfig, AccountInfo } from "@/types";
import { executionApi } from "@/lib/api";
import EmergencyHaltButton from "@/components/EmergencyHaltButton";

interface FieldMeta {
  key: keyof RiskConfig;
  label: string;
  description: string;
  format: "pct" | "int" | "secs" | "time";
  readOnly?: boolean;
}

const FIELDS: FieldMeta[] = [
  { key: "max_daily_loss_pct", label: "Max Daily Loss", description: "Halt all trading when daily loss exceeds this % of equity", format: "pct" },
  { key: "max_position_size_pct", label: "Max Position Size", description: "Reject signals that would exceed this % of equity per position", format: "pct" },
  { key: "max_open_positions", label: "Max Open Positions", description: "Reject signals when this many positions are already open (1\u201310)", format: "int" },
  { key: "min_signal_confidence", label: "Min Signal Confidence", description: "Reject signals below this confidence threshold (0.0\u20131.0)", format: "pct" },
  { key: "order_throttle_secs", label: "Order Throttle", description: "Minimum seconds between orders for the same symbol", format: "secs" },
  { key: "eod_flatten_time_et", label: "EOD Flatten Time (ET)", description: "All positions market-sold at this time. Not editable in v1.", format: "time", readOnly: true },
];

function formatDisplay(value: number | string, format: string): string {
  if (format === "pct") return `${((value as number) * 100).toFixed(1)}%`;
  if (format === "secs") return `${value}s`;
  if (format === "time") return value as string;
  return String(value);
}

function toInputValue(value: number | string, format: string): string {
  if (format === "pct") return ((value as number) * 100).toString();
  return String(value);
}

function fromInputValue(input: string, format: string): number {
  if (format === "pct") return parseFloat(input) / 100;
  if (format === "int") return parseInt(input, 10);
  return parseFloat(input);
}

export default function RiskPage() {
  const [config, setConfig] = useState<RiskConfig | null>(null);
  const [account, setAccount] = useState<AccountInfo | null>(null);
  const [edited, setEdited] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [saveError, setSaveError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [fetchError, setFetchError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const [cfg, acct] = await Promise.all([
        executionApi.getRiskConfig(),
        executionApi.getAccount(),
      ]);
      setConfig(cfg);
      setAccount(acct);
      setFetchError(null);
    } catch (e) {
      setFetchError(e instanceof Error ? e.message : "Failed to load risk config");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  const handleInputChange = (key: string, value: string) => {
    setEdited((prev) => ({ ...prev, [key]: value }));
    setErrors((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
    setSaveError(null);
  };

  const handleSave = async () => {
    if (!config) return;

    const changedKeys = Object.keys(edited);
    if (changedKeys.length === 0) return;

    // Client-side validation
    const newErrors: Record<string, string> = {};
    for (const key of changedKeys) {
      const meta = FIELDS.find((f) => f.key === key);
      if (!meta) continue;
      const raw = edited[key];
      const num = parseFloat(raw);

      if (isNaN(num)) {
        newErrors[key] = "Must be a number";
        continue;
      }

      if (meta.format === "pct" && (num < 0 || num > 100)) {
        newErrors[key] = "Must be between 0 and 100";
      }
      if (meta.key === "max_open_positions" && (num < 1 || num > 10 || !Number.isInteger(num))) {
        newErrors[key] = "Must be an integer between 1 and 10";
      }
      if (meta.key === "order_throttle_secs" && num < 0) {
        newErrors[key] = "Must be non-negative";
      }
    }

    if (Object.keys(newErrors).length > 0) {
      setErrors(newErrors);
      return;
    }

    setSaving(true);
    setSaveError(null);

    const patch: Partial<RiskConfig> = {};
    for (const key of changedKeys) {
      const meta = FIELDS.find((f) => f.key === key)!;
      (patch as any)[key] = fromInputValue(edited[key], meta.format);
    }

    const result = await executionApi.patchRiskConfig(patch);
    if (result.error) {
      setSaveError(result.error);
    } else if (result.data) {
      setConfig(result.data);
      setEdited({});
    }
    setSaving(false);
  };

  const hasChanges = Object.keys(edited).length > 0;

  if (loading) {
    return (
      <div>
        <h2 className="text-lg font-semibold text-text-primary mb-4">Risk Settings</h2>
        <p className="text-text-secondary text-sm">Loading risk configuration...</p>
      </div>
    );
  }

  if (fetchError) {
    return (
      <div>
        <h2 className="text-lg font-semibold text-text-primary mb-4">Risk Settings</h2>
        <div className="rounded-lg border border-loss/30 bg-loss/10 p-4 text-loss text-sm">{fetchError}</div>
        <button onClick={fetchData} className="mt-3 text-sm px-3 py-1.5 rounded bg-accent text-white hover:bg-accent-dark">Retry</button>
      </div>
    );
  }

  return (
    <div>
      <h2 className="text-lg font-semibold text-text-primary mb-1">Risk Settings</h2>
      <p className="text-text-secondary text-sm mb-6">
        View and edit risk rule thresholds. Changes take effect immediately.
      </p>

      {/* Config fields */}
      <div className="rounded-lg border border-surface-600 bg-surface-900 mb-6">
        <div className="divide-y divide-surface-600">
          {config && FIELDS.map((field) => {
            const currentValue = config[field.key];
            const inputValue = edited[field.key] ?? toInputValue(currentValue, field.format);
            const error = errors[field.key];

            return (
              <div key={field.key} className="px-5 py-4 flex items-start gap-4">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <label className="text-sm font-medium text-text-primary">{field.label}</label>
                    {field.readOnly && (
                      <span className="text-xs px-1.5 py-0.5 rounded bg-surface-600 text-text-secondary">read-only</span>
                    )}
                  </div>
                  <p className="text-xs text-text-secondary mt-0.5">{field.description}</p>
                  {error && <p className="text-xs text-loss mt-1">{error}</p>}
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  {field.readOnly ? (
                    <div className="relative group">
                      <span className="text-sm font-mono bg-surface-800 border border-surface-600 rounded px-3 py-1.5 text-text-secondary cursor-help">
                        {String(currentValue)}
                      </span>
                      <div className="absolute bottom-full right-0 mb-1 hidden group-hover:block bg-surface-800 text-text-primary text-xs rounded px-2 py-1 whitespace-nowrap">
                        EOD flatten time is not configurable in v1
                      </div>
                    </div>
                  ) : (
                    <div className="flex items-center gap-1">
                      <input
                        type="text"
                        value={inputValue}
                        onChange={(e) => handleInputChange(field.key, e.target.value)}
                        className={`w-24 text-sm font-mono border rounded px-3 py-1.5 text-right bg-surface-800 text-text-primary focus:outline-none focus:ring-1 focus:ring-accent ${
                          error ? "border-loss bg-loss/10" : "border-surface-600"
                        }`}
                      />
                      {field.format === "pct" && <span className="text-xs text-text-secondary">%</span>}
                      {field.format === "secs" && <span className="text-xs text-text-secondary">sec</span>}
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>

        {/* Save bar */}
        <div className="px-5 py-3 bg-surface-800 border-t border-surface-600 flex items-center justify-between">
          {saveError && (
            <p className="text-sm text-loss">{saveError}</p>
          )}
          {!saveError && <span />}
          <div className="flex gap-2">
            {hasChanges && (
              <button
                onClick={() => { setEdited({}); setErrors({}); setSaveError(null); }}
                className="text-xs px-3 py-1.5 rounded border border-surface-600 text-text-secondary hover:bg-surface-700"
              >
                Discard
              </button>
            )}
            <button
              onClick={handleSave}
              disabled={!hasChanges || saving}
              className="text-xs px-4 py-1.5 rounded bg-accent text-white hover:bg-accent-dark disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {saving ? "Saving..." : "Save Changes"}
            </button>
          </div>
        </div>
      </div>

      {/* Emergency halt */}
      <EmergencyHaltButton
        isHalted={account?.trading_blocked ?? false}
        onToggle={fetchData}
      />
    </div>
  );
}
