import type {
  StatsResponse,
  EntryListResponse,
  Entry,
  SearchResponse,
  CategoriesResponse,
  EntryGraphResponse,
} from '../types';

const API_BASE = '/api';

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }
  return response.json();
}

export const api = {
  // Get dashboard statistics
  getStats: () => fetchJson<StatsResponse>(`${API_BASE}/stats`),

  // List all entries with optional filters
  getEntries: (params?: {
    category?: string;
    tag?: string;
    entry_type?: string;
    limit?: number;
    offset?: number;
  }) => {
    const searchParams = new URLSearchParams();
    if (params?.category) searchParams.set('category', params.category);
    if (params?.tag) searchParams.set('tag', params.tag);
    if (params?.entry_type) searchParams.set('entry_type', params.entry_type);
    if (params?.limit) searchParams.set('limit', params.limit.toString());
    if (params?.offset) searchParams.set('offset', params.offset.toString());
    const query = searchParams.toString();
    return fetchJson<EntryListResponse>(`${API_BASE}/patterns${query ? `?${query}` : ''}`);
  },

  // Get a specific entry by ID
  getEntry: (id: string) => fetchJson<Entry>(`${API_BASE}/patterns/${encodeURIComponent(id)}`),

  // Get related entries
  getRelatedEntries: (id: string) =>
    fetchJson<EntryListResponse>(`${API_BASE}/patterns-related/${encodeURIComponent(id)}`),

  // List all categories
  getCategories: () => fetchJson<CategoriesResponse>(`${API_BASE}/categories`),

  // Search entries
  search: (params: {
    q: string;
    entry_type?: string;
    category?: string;
    tag?: string;
    source_domain?: string;
    limit?: number;
  }) => {
    const searchParams = new URLSearchParams();
    searchParams.set('q', params.q);
    if (params.entry_type) searchParams.set('entry_type', params.entry_type);
    if (params.category) searchParams.set('category', params.category);
    if (params.tag) searchParams.set('tag', params.tag);
    if (params.source_domain) searchParams.set('source_domain', params.source_domain);
    if (params.limit) searchParams.set('limit', params.limit.toString());
    return fetchJson<SearchResponse>(`${API_BASE}/search?${searchParams.toString()}`);
  },

  // Get entry graph data
  getEntryGraph: () => fetchJson<EntryGraphResponse>(`${API_BASE}/graph`),
};
