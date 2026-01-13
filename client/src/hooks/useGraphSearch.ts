import { useMemo, useCallback, useState, useEffect } from 'react';
import Fuse from 'fuse.js';
import type { GraphNodeExtended } from '../types/graph';

export interface SearchResult {
  node: GraphNodeExtended;
  score: number;
  matches: Array<{
    key: string;
    value: string;
    indices: Array<[number, number]>;
  }>;
}

export interface UseGraphSearchOptions {
  nodes: GraphNodeExtended[];
  query: string;
  onQueryChange?: (query: string) => void;
  limit?: number;
}

export function useGraphSearch(options: UseGraphSearchOptions) {
  const { nodes, query, onQueryChange, limit = 10 } = options;
  const [debouncedQuery, setDebouncedQuery] = useState(query);

  // Debounce query changes
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(query);
    }, 150);
    return () => clearTimeout(timer);
  }, [query]);

  // Create Fuse.js search index
  const searchIndex = useMemo(() => {
    return new Fuse(nodes, {
      keys: [
        { name: 'label', weight: 0.4 },
        { name: 'category', weight: 0.25 },
        { name: 'tags', weight: 0.2 },
        { name: 'source_domain', weight: 0.1 },
        { name: 'description', weight: 0.05 },
      ],
      threshold: 0.4,
      includeScore: true,
      includeMatches: true,
      ignoreLocation: true,
      useExtendedSearch: true,
    });
  }, [nodes]);

  // Perform search
  const results = useMemo((): SearchResult[] => {
    if (!debouncedQuery.trim()) return [];

    const fuseResults = searchIndex.search(debouncedQuery, { limit });

    return fuseResults.map(result => ({
      node: result.item,
      score: 1 - (result.score || 0), // Convert to 0-1 where 1 is best
      matches: (result.matches || []).map(match => ({
        key: match.key || '',
        value: match.value || '',
        indices: match.indices as Array<[number, number]>,
      })),
    }));
  }, [searchIndex, debouncedQuery, limit]);

  // Get set of matching node IDs
  const matchingNodeIds = useMemo(() => {
    return new Set(results.map(r => r.node.id));
  }, [results]);

  // Check if a specific node matches
  const isNodeMatch = useCallback((nodeId: string): boolean => {
    return matchingNodeIds.has(nodeId);
  }, [matchingNodeIds]);

  // Get best match (for auto-focus)
  const bestMatch = useMemo(() => {
    return results.length > 0 ? results[0] : null;
  }, [results]);

  // Clear search
  const clearSearch = useCallback(() => {
    onQueryChange?.('');
  }, [onQueryChange]);

  // Set search query
  const setQuery = useCallback((newQuery: string) => {
    onQueryChange?.(newQuery);
  }, [onQueryChange]);

  return {
    query,
    debouncedQuery,
    results,
    matchingNodeIds,
    isNodeMatch,
    bestMatch,
    hasResults: results.length > 0,
    isSearching: debouncedQuery.trim().length > 0,
    clearSearch,
    setQuery,
  };
}

// Highlight text with search matches
export function highlightMatches(
  text: string,
  indices: Array<[number, number]>
): Array<{ text: string; highlighted: boolean }> {
  if (!indices || indices.length === 0) {
    return [{ text, highlighted: false }];
  }

  const parts: Array<{ text: string; highlighted: boolean }> = [];
  let lastIndex = 0;

  indices.forEach(([start, end]) => {
    // Add non-highlighted text before match
    if (start > lastIndex) {
      parts.push({ text: text.slice(lastIndex, start), highlighted: false });
    }
    // Add highlighted match
    parts.push({ text: text.slice(start, end + 1), highlighted: true });
    lastIndex = end + 1;
  });

  // Add remaining text
  if (lastIndex < text.length) {
    parts.push({ text: text.slice(lastIndex), highlighted: false });
  }

  return parts;
}
