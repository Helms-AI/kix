// API response types
export interface StatsResponse {
  total_entries: number;
  messaging_entries: number;
  conversation_entries: number;
  total_chunks: number;
  total_documents: number;
  categories: CategoryCount[];
}

export interface CategoryCount {
  name: string;
  count: number;
}

export interface Entry {
  id: string;
  title: string;
  entry_type: string;
  tags: string[];
  description: string;
  content?: string;
  source_type?: string;
  source_path?: string;
  source_domain?: string;
  slug?: string;
  source_hash?: string;
  created_at?: string;
  updated_at?: string;
  collection_ids?: string[];
}

// Chunk types for search results
export type ChunkType = 'summary' | 'content' | 'code' | 'header';

export interface EntryListResponse {
  entries: Entry[];
  total: number;
}

export interface SearchResult {
  id: string;
  entry_id: string;
  entry_title: string;
  entry_type: string;
  tags: string[];
  chunk_type?: ChunkType;
  source_domain?: string;
  text: string;
  score: number;
}

export interface SearchResponse {
  results: SearchResult[];
  total: number;
  query: string;
}

export interface CategoryInfo {
  name: string;
  entry_count: number;
  entry_type: string;
}

export interface CategoriesResponse {
  categories: CategoryInfo[];
}

export interface GraphNode {
  id: string;
  label: string;
  entry_type: string;
  category: string;
}

export interface GraphEdge {
  source: string;
  target: string;
  weight: number;
}

export interface EntryGraphResponse {
  nodes: GraphNode[];
  edges: GraphEdge[];
}
