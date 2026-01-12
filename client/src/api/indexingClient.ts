const API_BASE = '/api/indexing';

// Types
export interface StartUrlIndexingRequest {
  url: string;
  levels: number;
  respect_robots: boolean;
  skip_render: boolean;  // If true, skip JS rendering (faster, for static pages)
  timeout_secs: number;  // Browser rendering timeout in seconds
  priority: number;
  max_pages: number;  // Maximum pages to crawl (0 = unlimited)
}

export interface FileUpload {
  name: string;
  content: string; // base64 encoded
}

export interface StartFileIndexingRequest {
  files: FileUpload[];
  extract_archives: boolean;
  priority: number;
}

export interface StartIndexingResponse {
  job_id: string;
  sse_url: string;
  status: string;
}

export interface JobProgress {
  processed: number;
  total: number;
  percentage: number;
  current_item?: string | null;
  currentItem?: string | null;
  rate: number;
}

// Enhanced types for SSE events
export interface ItemProcessedEvent {
  job_id: string;
  item_path: string;
  chunks_created: number;
  embeddings_generated: number;
  duration_ms: number;
  timestamp: string;
}

export interface JobErrorEvent {
  job_id: string;
  item_path: string | null;
  error_message: string;
  recoverable: boolean;
  timestamp: string;
}

export interface JobLogEntry {
  id: string;
  type: 'item' | 'error';
  timestamp: string;
  item_path: string;
  // Item-specific fields
  chunks_created?: number;
  embeddings_generated?: number;
  duration_ms?: number;
  // Error-specific fields
  error_message?: string;
  recoverable?: boolean;
}

// Page status for the new page tracking system
export type PageStatus = 'pending' | 'running' | 'completed' | 'error';

// Individual page state for tracking
export interface PageState {
  url: string;
  status: PageStatus;
  // Discovery metadata
  discoveredAt: string;
  parentUrl?: string;
  depth?: number;
  // Processing metadata (when running/completed)
  startedAt?: string;
  completedAt?: string;
  // Result metadata (when completed)
  chunksCreated?: number;
  embeddingsGenerated?: number;
  durationMs?: number;
  // Error metadata (when error)
  errorMessage?: string;
  recoverable?: boolean;
}

export interface EnhancedLiveJobData {
  processed: number;
  total: number;
  percentage: number;
  rate: number;
  currentItem?: string;
  etaSeconds?: number;
  // NEW: Page states keyed by URL for O(1) updates
  pages: Record<string, PageState>;
  // Keep errors array for quick error summary
  errors: JobLogEntry[];
  rateHistory: number[];
  totalChunks: number;
  totalEmbeddings: number;
  source?: string;
  sourceType?: 'url' | 'file' | 'directory';
}

export interface Job {
  id: string;
  job_type: 'url' | 'file_upload' | 'reindex';
  status: 'pending' | 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';
  created_at: string;
  progress: JobProgress | null;
}

export interface JobListResponse {
  jobs: Job[];
  total: number;
}

export interface ListJobsParams {
  status?: string;
  limit?: number;
  offset?: number;
}

export interface DeleteAllDataResponse {
  status: string;
  message: string;
  documents_deleted: number;
  chunks_deleted: number;
}

// ============================================================================
// Job History Types (Persisted Jobs)
// ============================================================================

// Stats for a persisted job
export interface PersistedJobStats {
  items_processed: number;
  items_discovered: number;
  chunks_created: number;
  embeddings_generated: number;
  error_count: number;
  duration_ms: number;
  rate: number;
}

// Persisted job from history API (matches backend JobHistoryItem)
export interface PersistedJob {
  job_id: string;
  job_type: 'url' | 'file_upload' | 'reindex';
  status: 'completed' | 'failed' | 'cancelled';
  source_url: string | null;
  source_domain: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string;
  stats: PersistedJobStats;
}

// Individual item from a persisted job (matches backend JobHistoryItemDetail)
// Matches backend JobHistoryItemDetail struct
export interface PersistedJobItem {
  url: string;  // Maps to item_path on backend
  status: string;
  parent_url: string | null;
  depth: number;
  discovered_at: string;
  started_at: string | null;
  completed_at: string | null;
  chunks_created: number;
  embeddings_generated: number;
  duration_ms: number;
  error_message: string | null;
}

// Response from GET /api/indexing/history
export interface JobHistoryResponse {
  jobs: PersistedJob[];
  total: number;
}

// Response from GET /api/indexing/history/:job_id
export interface JobDetailResponse {
  job: PersistedJob;
  items: PersistedJobItem[];
  errors: string[];
}

// Parameters for listing job history
export interface ListJobHistoryParams {
  limit?: number;
  offset?: number;
}

// API functions
export const indexingApi = {
  // Start URL indexing
  async startUrlIndexing(request: StartUrlIndexingRequest): Promise<StartIndexingResponse> {
    const response = await fetch(`${API_BASE}/url`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.message || 'Failed to start URL indexing');
    }

    return response.json();
  },

  // Start file indexing
  async startFileIndexing(request: StartFileIndexingRequest): Promise<StartIndexingResponse> {
    const response = await fetch(`${API_BASE}/files`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.message || 'Failed to start file indexing');
    }

    return response.json();
  },

  // List jobs
  async listJobs(params?: ListJobsParams): Promise<JobListResponse> {
    const searchParams = new URLSearchParams();
    if (params?.status) searchParams.set('status', params.status);
    if (params?.limit) searchParams.set('limit', params.limit.toString());
    if (params?.offset) searchParams.set('offset', params.offset.toString());

    const url = `${API_BASE}/jobs${searchParams.toString() ? `?${searchParams}` : ''}`;
    const response = await fetch(url);

    if (!response.ok) {
      throw new Error('Failed to fetch jobs');
    }

    return response.json();
  },

  // Get job details
  async getJob(id: string): Promise<Job> {
    const response = await fetch(`${API_BASE}/jobs/${id}`);

    if (!response.ok) {
      throw new Error('Failed to fetch job details');
    }

    return response.json();
  },

  // Cancel job
  async cancelJob(id: string): Promise<{ status: string; job_id: string }> {
    const response = await fetch(`${API_BASE}/jobs/${id}`, {
      method: 'DELETE',
    });

    if (!response.ok) {
      throw new Error('Failed to cancel job');
    }

    return response.json();
  },

  // Delete all indexed data
  async deleteAllData(): Promise<DeleteAllDataResponse> {
    const response = await fetch(`${API_BASE}/data`, {
      method: 'DELETE',
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.message || 'Failed to delete all data');
    }

    return response.json();
  },

  // ============================================================================
  // Job History API (Persisted Jobs)
  // ============================================================================

  // List persisted job history
  async getJobHistory(params?: ListJobHistoryParams): Promise<JobHistoryResponse> {
    const searchParams = new URLSearchParams();
    if (params?.limit) searchParams.set('limit', params.limit.toString());
    if (params?.offset) searchParams.set('offset', params.offset.toString());

    const url = `${API_BASE}/history${searchParams.toString() ? `?${searchParams}` : ''}`;
    const response = await fetch(url);

    if (!response.ok) {
      throw new Error('Failed to fetch job history');
    }

    return response.json();
  },

  // Get detailed info for a persisted job including all items
  async getJobDetail(jobId: string): Promise<JobDetailResponse> {
    const response = await fetch(`${API_BASE}/history/${jobId}`);

    if (!response.ok) {
      if (response.status === 404) {
        throw new Error('Job not found in history');
      }
      throw new Error('Failed to fetch job details');
    }

    return response.json();
  },

  // Get just the items for a persisted job (for pagination/lazy loading)
  async getJobItems(jobId: string): Promise<PersistedJobItem[]> {
    const response = await fetch(`${API_BASE}/history/${jobId}/items`);

    if (!response.ok) {
      if (response.status === 404) {
        throw new Error('Job not found in history');
      }
      throw new Error('Failed to fetch job items');
    }

    return response.json();
  },
};

// Utility function to convert File to base64
export async function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.readAsDataURL(file);
    reader.onload = () => {
      const base64 = reader.result as string;
      // Remove data URL prefix (e.g., "data:application/pdf;base64,")
      const base64Content = base64.split(',')[1];
      resolve(base64Content);
    };
    reader.onerror = (error) => reject(error);
  });
}

// Format file size
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

// Format duration
export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}m ${secs}s`;
}

// Format duration from milliseconds
export function formatDurationMs(ms: number): string {
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}m ${secs}s`;
}

// Convert PersistedJobItem array to PageState record for PageStatusList compatibility
export function convertPersistedItemsToPages(items: PersistedJobItem[]): Record<string, PageState> {
  const pages: Record<string, PageState> = {};

  for (const item of items) {
    const pageStatus: PageStatus =
      item.status === 'completed' ? 'completed' :
      item.status === 'error' ? 'error' : 'completed';

    pages[item.url] = {
      url: item.url,
      status: pageStatus,
      discoveredAt: item.discovered_at,
      parentUrl: item.parent_url || undefined,
      depth: item.depth,
      startedAt: item.started_at || undefined,
      completedAt: item.completed_at || undefined,
      chunksCreated: item.chunks_created,
      embeddingsGenerated: item.embeddings_generated,
      durationMs: item.duration_ms,
      errorMessage: item.error_message || undefined,
      recoverable: false, // Historical items are not recoverable
    };
  }

  return pages;
}
