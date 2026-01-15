import { useState, useCallback } from 'react';
import type { QueryHistoryEntry } from '../types';

interface UseQueryHistoryReturn {
  history: QueryHistoryEntry[];
  addToHistory: (entry: QueryHistoryEntry) => void;
  clearHistory: () => void;
  removeFromHistory: (index: number) => void;
}

const MAX_HISTORY_ENTRIES = 20;

/**
 * Hook for managing query history
 */
export function useQueryHistory(maxEntries = MAX_HISTORY_ENTRIES): UseQueryHistoryReturn {
  const [history, setHistory] = useState<QueryHistoryEntry[]>([]);

  const addToHistory = useCallback(
    (entry: QueryHistoryEntry) => {
      setHistory(prev => [entry, ...prev.slice(0, maxEntries - 1)]);
    },
    [maxEntries]
  );

  const clearHistory = useCallback(() => {
    setHistory([]);
  }, []);

  const removeFromHistory = useCallback((index: number) => {
    setHistory(prev => prev.filter((_, i) => i !== index));
  }, []);

  return {
    history,
    addToHistory,
    clearHistory,
    removeFromHistory,
  };
}
