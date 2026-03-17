"use client";

import { useEffect, useState, useCallback } from "react";
import { strategyApi } from "@/lib/api";

const FALLBACK_SYMBOLS = ["SPY", "QQQ", "AAPL", "MSFT", "NVDA", "GOOGL"];

interface UseSymbolsReturn {
  symbols: string[];
  addSymbol: (symbol: string) => Promise<void>;
  removeSymbol: (symbol: string) => Promise<void>;
  error: string | null;
}

export function useSymbols(): UseSymbolsReturn {
  const [symbols, setSymbols] = useState<string[]>(FALLBACK_SYMBOLS);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    strategyApi
      .getSymbols()
      .then((data) => {
        if (data.symbols && data.symbols.length > 0) {
          setSymbols(data.symbols);
        }
      })
      .catch(() => {});
  }, []);

  const addSymbol = useCallback(async (symbol: string) => {
    setError(null);
    try {
      const data = await strategyApi.addSymbol(symbol);
      setSymbols(data.symbols);
    } catch (e: any) {
      setError(e.message || "Failed to add symbol");
      throw e;
    }
  }, []);

  const removeSymbol = useCallback(async (symbol: string) => {
    setError(null);
    try {
      const data = await strategyApi.removeSymbol(symbol);
      setSymbols(data.symbols);
    } catch (e: any) {
      setError(e.message || "Failed to remove symbol");
      throw e;
    }
  }, []);

  return { symbols, addSymbol, removeSymbol, error };
}
