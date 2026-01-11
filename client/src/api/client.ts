import type {
  StatsResponse,
  EntryListResponse,
  Entry,
  SearchResponse,
  CategoriesResponse,
  EntryGraphResponse,
  ChunksResponse,
} from '../types';

const API_BASE = '/api';

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }
  return response.json();
}

async function postJson<T>(url: string, body: unknown): Promise<T> {
  const response = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(errorBody || `HTTP error! status: ${response.status}`);
  }
  return response.json();
}

// Types for indexing operations
interface ReindexEntryRequest {
  skip_render?: boolean;
  timeout_secs?: number;
  priority?: number;
}

interface ReindexByDomainRequest {
  domain: string;
  skip_render?: boolean;
  timeout_secs?: number;
  priority?: number;
}

interface StartIndexingResponse {
  job_id: string;
  sse_url: string;
  status: string;
}

interface ReindexResponse {
  entries_queued: number;
  job_ids: string[];
  status: string;
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
    return fetchJson<EntryListResponse>(`${API_BASE}/entries${query ? `?${query}` : ''}`);
  },

  // Get a specific entry by ID
  getEntry: (id: string) => fetchJson<Entry>(`${API_BASE}/entries/${encodeURIComponent(id)}`),

  // Get related entries
  getRelatedEntries: (id: string) =>
    fetchJson<EntryListResponse>(`${API_BASE}/entries-related/${encodeURIComponent(id)}`),

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

  // Get chunks for an entry
  getChunks: (id: string) =>
    fetchJson<ChunksResponse>(`${API_BASE}/entries-chunks/${encodeURIComponent(id)}`),

  // Re-index a single entry
  reindexEntry: (id: string, options?: ReindexEntryRequest) =>
    postJson<StartIndexingResponse>(
      `${API_BASE}/indexing/reindex/${encodeURIComponent(id)}`,
      options || {}
    ),

  // Re-index all entries from a domain
  reindexByDomain: (domain: string, options?: Omit<ReindexByDomainRequest, 'domain'>) =>
    postJson<ReindexResponse>(`${API_BASE}/indexing/reindex-domain`, {
      domain,
      ...options,
    }),
};
