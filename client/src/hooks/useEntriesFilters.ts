import { useSearchParams } from 'react-router-dom';
import { useCallback, useMemo } from 'react';

export type SearchMode = 'fts' | 'vector' | 'hybrid';
export type ViewMode = 'grid' | 'list' | 'compact';
export type EntryType = 'all' | 'document' | 'article' | 'pdf' | 'code';

export interface EntriesFilters {
  q: string;
  type: EntryType;
  mode: SearchMode;
  view: ViewMode;
  domain: string;
  tag: string;
}

const DEFAULT_FILTERS: EntriesFilters = {
  q: '',
  type: 'all',
  mode: 'hybrid',
  view: 'grid',
  domain: '',
  tag: '',
};

export function useEntriesFilters() {
  const [searchParams, setSearchParams] = useSearchParams();

  const filters: EntriesFilters = useMemo(() => ({
    q: searchParams.get('q') || DEFAULT_FILTERS.q,
    type: (searchParams.get('type') as EntryType) || DEFAULT_FILTERS.type,
    mode: (searchParams.get('mode') as SearchMode) || DEFAULT_FILTERS.mode,
    view: (searchParams.get('view') as ViewMode) || DEFAULT_FILTERS.view,
    domain: searchParams.get('domain') || DEFAULT_FILTERS.domain,
    tag: searchParams.get('tag') || DEFAULT_FILTERS.tag,
  }), [searchParams]);

  const setFilter = useCallback(<K extends keyof EntriesFilters>(
    key: K,
    value: EntriesFilters[K]
  ) => {
    setSearchParams(prev => {
      const newParams = new URLSearchParams(prev);

      // Only set param if it's not the default value
      const isDefault = value === DEFAULT_FILTERS[key] || value === '';

      if (isDefault) {
        newParams.delete(key);
      } else {
        newParams.set(key, String(value));
      }

      return newParams;
    });
  }, [setSearchParams]);

  const setMultipleFilters = useCallback((updates: Partial<EntriesFilters>) => {
    setSearchParams(prev => {
      const newParams = new URLSearchParams(prev);

      Object.entries(updates).forEach(([key, value]) => {
        const defaultValue = DEFAULT_FILTERS[key as keyof EntriesFilters];
        const isDefault = value === defaultValue || value === '';

        if (isDefault) {
          newParams.delete(key);
        } else {
          newParams.set(key, String(value));
        }
      });

      return newParams;
    });
  }, [setSearchParams]);

  const clearFilters = useCallback(() => {
    setSearchParams(new URLSearchParams());
  }, [setSearchParams]);

  const hasActiveFilters = useMemo(() => {
    return (
      filters.q !== '' ||
      filters.type !== 'all' ||
      filters.domain !== '' ||
      filters.tag !== ''
    );
  }, [filters]);

  const activeFilterCount = useMemo(() => {
    let count = 0;
    if (filters.q) count++;
    if (filters.type !== 'all') count++;
    if (filters.domain) count++;
    if (filters.tag) count++;
    return count;
  }, [filters]);

  return {
    filters,
    setFilter,
    setMultipleFilters,
    clearFilters,
    hasActiveFilters,
    activeFilterCount,
  };
}
