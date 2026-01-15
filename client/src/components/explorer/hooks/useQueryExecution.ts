import { useState, useCallback } from 'react';
import { executeQuery } from '../../../api/explorerClient';
import type { DatabaseInfo, QueryResult, QueryHistoryEntry } from '../types';

interface UseQueryExecutionOptions {
  onSuccess?: (result: QueryResult) => void;
  onError?: (error: string) => void;
  onAddToHistory?: (entry: QueryHistoryEntry) => void;
}

interface UseQueryExecutionReturn {
  query: string;
  setQuery: (query: string) => void;
  queryResult: QueryResult | null;
  isExecuting: boolean;
  error: string | null;
  clearError: () => void;
  clearResult: () => void;
  runQuery: () => Promise<void>;
}

/**
 * Hook for managing query execution state and logic
 */
export function useQueryExecution(
  selectedDatabase: DatabaseInfo | null,
  safeMode: boolean,
  options: UseQueryExecutionOptions = {}
): UseQueryExecutionReturn {
  const [query, setQuery] = useState('');
  const [queryResult, setQueryResult] = useState<QueryResult | null>(null);
  const [isExecuting, setIsExecuting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const clearError = useCallback(() => setError(null), []);
  const clearResult = useCallback(() => setQueryResult(null), []);

  const runQuery = useCallback(async () => {
    if (!query.trim() || !selectedDatabase) return;

    setIsExecuting(true);
    setError(null);
    setQueryResult(null);

    try {
      const response = await executeQuery(selectedDatabase.id, query, {
        safe_mode: safeMode,
        limit: 1000,
      });

      if (response.success && response.result) {
        setQueryResult(response.result);
        options.onSuccess?.(response.result);

        // Add to history
        options.onAddToHistory?.({
          query,
          time: new Date(),
          databaseId: selectedDatabase.id,
        });
      } else {
        const errorMsg = response.error || 'Query failed';
        setError(errorMsg);
        options.onError?.(errorMsg);
      }
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : 'Query execution failed';
      setError(errorMsg);
      options.onError?.(errorMsg);
    } finally {
      setIsExecuting(false);
    }
  }, [query, selectedDatabase, safeMode, options]);

  return {
    query,
    setQuery,
    queryResult,
    isExecuting,
    error,
    clearError,
    clearResult,
    runQuery,
  };
}
