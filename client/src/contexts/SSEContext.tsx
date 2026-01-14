import {
  createContext,
  useContext,
  useState,
  useCallback,
  useRef,
  useEffect,
  type ReactNode,
} from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type {
  EnhancedLiveJobData,
  JobLogEntry,
  PageState,
  CodeExtractionStats,
  LanguageStats,
  PatternStats,
  RejectionReason,
} from '../api/indexingClient';
import type { SSEEvent, SSEEventType } from '../hooks/useSSE';

// ============================================================================
// Types
// ============================================================================

interface SSEContextType {
  /** Whether the SSE connection is established */
  isConnected: boolean;
  /** Whether attempting to connect */
  isConnecting: boolean;
  /** Connection error if any */
  connectionError: Event | null;
  /** Number of reconnection attempts */
  reconnectAttempts: number;
  /** Maximum reconnection attempts allowed */
  maxReconnectAttempts: number;
  /** Manual reconnect function */
  reconnect: () => void;
  /** Live job data keyed by job ID */
  jobs: Record<string, EnhancedLiveJobData>;
  /** Clear pages for a specific job */
  clearJobLog: (jobId: string) => void;
  /** Clear all job data */
  clearAllJobs: () => void;
}

// Backend SSE event structure
interface BackendSSEEvent {
  id: string;
  timestamp: string;
  data: {
    type: SSEEventType;
    job_id: string;
    processed?: number;
    total?: number;
    percentage?: number;
    rate?: number;
    current_item?: string;
    item_path?: string;
    chunks_created?: number;
    embeddings_generated?: number;
    duration_ms?: number;
    error_message?: string;
    recoverable?: boolean;
    source?: { url?: string; depth?: number; path?: string };
    total_items?: number;
    total_processed?: number;
    total_chunks?: number;
    errors?: string[];
    reason?: string;
    // Item discovered fields
    discovered_at?: string;
    parent_url?: string;
    depth?: number;
    // Item started fields
    started_at?: string;
    // Code extraction fields (NEW)
    url?: string;
    blocks_found?: number;
    patterns_matched?: string[];
    languages?: { language: string; count: number }[];
    validation_stats?: {
      total_extracted: number;
      passed_validation: number;
      rejected_too_short: number;
      rejected_placeholder: number;
      rejected_no_structure: number;
      rejected_high_prose: number;
    };
  };
}

// ============================================================================
// Constants
// ============================================================================

const MAX_RATE_SAMPLES = 20;

// Log entry ID counter
let logEntryIdCounter = 0;
function generateLogEntryId(): string {
  return `log-${Date.now()}-${logEntryIdCounter++}`;
}

// Helper to merge language stats from code_extraction events
function mergeLanguageStats(
  existing: LanguageStats[],
  eventLangs: { language: string; count: number }[]
): LanguageStats[] {
  const langMap = new Map<string, LanguageStats>();

  // Add existing stats
  for (const lang of existing) {
    langMap.set(lang.language, { ...lang });
  }

  // Merge in new counts
  for (const { language, count } of eventLangs) {
    const current = langMap.get(language);
    if (current) {
      current.block_count += count;
    } else {
      langMap.set(language, {
        language,
        block_count: count,
        total_lines: 0, // Updated incrementally
        percentage: 0,  // Recalculated below
      });
    }
  }

  // Recalculate percentages
  const allLangs = Array.from(langMap.values());
  const totalBlocks = allLangs.reduce((sum, l) => sum + l.block_count, 0);
  for (const lang of allLangs) {
    lang.percentage = totalBlocks > 0 ? (lang.block_count / totalBlocks) * 100 : 0;
  }

  return allLangs.sort((a, b) => b.block_count - a.block_count);
}

// Helper to merge pattern stats from code_extraction events
function mergePatternStats(
  existing: PatternStats[],
  eventPatterns: string[]
): PatternStats[] {
  const patternMap = new Map<string, PatternStats>();

  // Add existing stats
  for (const p of existing) {
    patternMap.set(p.pattern, { ...p });
  }

  // Merge in new patterns (each occurrence = +1)
  for (const pattern of eventPatterns) {
    const current = patternMap.get(pattern);
    if (current) {
      current.match_count += 1;
    } else {
      patternMap.set(pattern, {
        pattern,
        match_count: 1,
        percentage: 0, // Recalculated below
      });
    }
  }

  // Recalculate percentages
  const allPatterns = Array.from(patternMap.values());
  const totalMatches = allPatterns.reduce((sum, p) => sum + p.match_count, 0);
  for (const pattern of allPatterns) {
    pattern.percentage = totalMatches > 0 ? (pattern.match_count / totalMatches) * 100 : 0;
  }

  return allPatterns.sort((a, b) => b.match_count - a.match_count);
}

// Helper to merge validation stats from code_extraction events
function mergeValidationStats(
  existing: { total_extracted: number; passed: number; pass_rate: number; rejection_reasons: RejectionReason[] } | undefined,
  eventValidation: BackendSSEEvent['data']['validation_stats']
): { total_extracted: number; passed: number; pass_rate: number; rejection_reasons: RejectionReason[] } {
  if (!eventValidation) {
    return existing || { total_extracted: 0, passed: 0, pass_rate: 100, rejection_reasons: [] };
  }

  const total = (existing?.total_extracted || 0) + eventValidation.total_extracted;
  const passed = (existing?.passed || 0) + eventValidation.passed_validation;

  // Aggregate rejection reasons
  const reasonMap = new Map<string, number>();

  // Add existing reasons
  if (existing?.rejection_reasons) {
    for (const r of existing.rejection_reasons) {
      reasonMap.set(r.reason, r.count);
    }
  }

  // Add new reasons
  const newReasons = [
    { reason: 'too_short', count: eventValidation.rejected_too_short },
    { reason: 'placeholder', count: eventValidation.rejected_placeholder },
    { reason: 'no_structure', count: eventValidation.rejected_no_structure },
    { reason: 'high_prose', count: eventValidation.rejected_high_prose },
  ];

  for (const { reason, count } of newReasons) {
    if (count > 0) {
      reasonMap.set(reason, (reasonMap.get(reason) || 0) + count);
    }
  }

  return {
    total_extracted: total,
    passed: passed,
    pass_rate: total > 0 ? (passed / total) * 100 : 100,
    rejection_reasons: Array.from(reasonMap.entries())
      .map(([reason, count]) => ({ reason, count }))
      .filter(r => r.count > 0)
      .sort((a, b) => b.count - a.count),
  };
}

// ============================================================================
// Context
// ============================================================================

const SSEContext = createContext<SSEContextType | null>(null);

// ============================================================================
// Transform Backend Event
// ============================================================================

function transformBackendEvent(raw: BackendSSEEvent): SSEEvent | null {
  const { data } = raw;

  // Skip heartbeat events (they don't have job_id)
  if (data.type === 'heartbeat' || !data.job_id) {
    return null;
  }

  // Extract source info for job_started events
  let sourceType: 'url' | 'file' | 'directory' | undefined;
  let sourceValue: string | undefined;

  if (data.source) {
    if ('url' in data.source && data.source.url) {
      sourceType = 'url';
      sourceValue = data.source.url;
    } else if ('path' in data.source && data.source.path) {
      sourceType = 'file';
      sourceValue = data.source.path;
    }
  }

  return {
    event_type: data.type,
    job_id: data.job_id,
    timestamp: raw.timestamp,
    data: {
      processed: data.processed,
      total: data.total,
      percentage: data.percentage,
      rate: data.rate,
      current_item: data.current_item,
      item_path: data.item_path,
      chunks_created: data.chunks_created,
      embeddings_generated: data.embeddings_generated,
      duration_ms: data.duration_ms,
      error_message: data.error_message,
      recoverable: data.recoverable,
      source_type: sourceType,
      source: sourceValue,
      total_items: data.total_items,
      result: data.total_processed !== undefined ? {
        items_processed: data.total_processed,
        items_failed: 0,
        duration_secs: (data.duration_ms || 0) / 1000,
        total_chunks: data.total_chunks,
      } : undefined,
      // Item discovered fields
      discovered_at: data.discovered_at,
      parent_url: data.parent_url,
      depth: data.depth,
      // Item started fields
      started_at: data.started_at,
      // Code extraction fields
      url: data.url,
      blocks_found: data.blocks_found,
      patterns_matched: data.patterns_matched,
      languages: data.languages,
      validation_stats: data.validation_stats,
    },
  };
}

// ============================================================================
// Provider
// ============================================================================

export function SSEProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();

  // Connection state
  const [isConnected, setIsConnected] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [connectionError, setConnectionError] = useState<Event | null>(null);
  const [reconnectAttempts, setReconnectAttempts] = useState(0);

  // Job state
  const [jobs, setJobs] = useState<Record<string, EnhancedLiveJobData>>({});
  const lastRateSampleTime = useRef<{ [jobId: string]: number }>({});

  // Refs
  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Config
  const maxReconnectAttempts = 10;
  const reconnectInterval = 3000;

  // SSE URL - in development, connect directly to backend
  const sseUrl = import.meta.env.DEV
    ? 'http://localhost:3001/api/indexing/sse'
    : '/api/indexing/sse';

  // Process an SSE event
  const processEvent = useCallback((event: SSEEvent) => {
    const jobId = event.job_id;
    const now = Date.now();

    setJobs((prev) => {
      // Initialize job if not exists
      const jobState = prev[jobId] || {
        processed: 0,
        total: 0,
        percentage: 0,
        rate: 0,
        pages: {},
        errors: [],
        rateHistory: [],
        totalChunks: 0,
        totalEmbeddings: 0,
      };

      let updated = { ...jobState };

      switch (event.event_type) {
        case 'job_started': {
          updated.source = event.data.source;
          updated.sourceType = event.data.source_type;
          if (event.data.total_items) {
            updated.total = event.data.total_items;
          }
          break;
        }

        case 'progress': {
          updated.processed = event.data.processed ?? updated.processed;
          updated.total = event.data.total ?? updated.total;
          updated.percentage = event.data.percentage ?? updated.percentage;
          updated.rate = event.data.rate ?? updated.rate;
          updated.currentItem = event.data.current_item;

          // Calculate ETA from rate and remaining items (backend may not provide eta_seconds)
          if (event.data.eta_seconds != null) {
            updated.etaSeconds = event.data.eta_seconds;
          } else if (updated.rate > 0 && updated.total > 0) {
            const remaining = updated.total - updated.processed;
            updated.etaSeconds = remaining > 0 ? remaining / updated.rate : 0;
          }

          // Sample rate for sparkline (every 2 seconds)
          const lastSample = lastRateSampleTime.current[jobId] || 0;
          if (now - lastSample >= 2000 && event.data.rate !== undefined) {
            updated.rateHistory = [
              ...updated.rateHistory.slice(-MAX_RATE_SAMPLES + 1),
              event.data.rate,
            ];
            lastRateSampleTime.current[jobId] = now;
          }
          break;
        }

        case 'item_discovered': {
          const itemPath = event.data.item_path || 'Unknown';
          // Only add if not already tracked (avoid duplicates)
          if (!updated.pages[itemPath]) {
            const pageState: PageState = {
              url: itemPath,
              status: 'pending',
              discoveredAt: event.data.discovered_at || event.timestamp,
              parentUrl: event.data.parent_url,
              depth: event.data.depth,
            };
            updated.pages = { ...updated.pages, [itemPath]: pageState };
          }
          break;
        }

        case 'item_started': {
          const itemPath = event.data.item_path || 'Unknown';
          const existingPage = updated.pages[itemPath];
          const pageState: PageState = {
            url: itemPath,
            status: 'running',
            discoveredAt: existingPage?.discoveredAt || event.timestamp,
            parentUrl: existingPage?.parentUrl,
            depth: existingPage?.depth,
            startedAt: event.data.started_at || event.timestamp,
          };
          updated.pages = { ...updated.pages, [itemPath]: pageState };
          break;
        }

        case 'item_processed': {
          const itemPath = event.data.item_path || event.data.current_item || 'Unknown';
          const existingPage = updated.pages[itemPath];
          const pageState: PageState = {
            url: itemPath,
            status: 'completed',
            discoveredAt: existingPage?.discoveredAt || event.timestamp,
            parentUrl: existingPage?.parentUrl,
            depth: existingPage?.depth,
            startedAt: existingPage?.startedAt,
            completedAt: event.timestamp,
            chunksCreated: event.data.chunks_created,
            embeddingsGenerated: event.data.embeddings_generated,
            durationMs: event.data.duration_ms,
          };
          updated.pages = { ...updated.pages, [itemPath]: pageState };

          if (event.data.chunks_created) {
            updated.totalChunks += event.data.chunks_created;
          }
          if (event.data.embeddings_generated) {
            updated.totalEmbeddings += event.data.embeddings_generated;
          }
          break;
        }

        case 'error': {
          const itemPath = event.data.item_path || event.data.current_item || 'Unknown';
          const existingPage = updated.pages[itemPath];

          // Update page state to error
          const pageState: PageState = {
            url: itemPath,
            status: 'error',
            discoveredAt: existingPage?.discoveredAt || event.timestamp,
            parentUrl: existingPage?.parentUrl,
            depth: existingPage?.depth,
            startedAt: existingPage?.startedAt,
            completedAt: event.timestamp,
            errorMessage: event.data.error_message || 'Unknown error',
            recoverable: event.data.recoverable,
          };
          updated.pages = { ...updated.pages, [itemPath]: pageState };

          // Also keep in errors array for quick error summary
          const errorEntry: JobLogEntry = {
            id: generateLogEntryId(),
            type: 'error',
            timestamp: event.timestamp,
            item_path: itemPath,
            error_message: event.data.error_message || 'Unknown error',
            recoverable: event.data.recoverable,
          };
          updated.errors = [...updated.errors, errorEntry];
          break;
        }

        case 'code_extraction': {
          // Aggregate code extraction stats for the job
          const existingExtraction = updated.codeExtraction;
          const eventLanguages = event.data.languages || [];
          const eventPatterns = event.data.patterns_matched || [];
          const eventValidation = event.data.validation_stats;

          // Initialize or update code extraction stats
          const newExtraction: CodeExtractionStats = {
            job_id: jobId,
            total_pages: (existingExtraction?.total_pages || 0) + 1,
            pages_with_code: (existingExtraction?.pages_with_code || 0) + (event.data.blocks_found && event.data.blocks_found > 0 ? 1 : 0),
            total_code_blocks: (existingExtraction?.total_code_blocks || 0) + (event.data.blocks_found || 0),
            languages: mergeLanguageStats(existingExtraction?.languages || [], eventLanguages),
            patterns: mergePatternStats(existingExtraction?.patterns || [], eventPatterns),
            validation: mergeValidationStats(existingExtraction?.validation, eventValidation),
          };

          updated.codeExtraction = newExtraction;
          break;
        }

        case 'job_completed':
        case 'job_cancelled': {
          // Handle cleanup after a delay to show final state
          setTimeout(() => {
            setJobs((prev) => {
              const { [jobId]: _, ...rest } = prev;
              return rest;
            });
            delete lastRateSampleTime.current[jobId];
          }, 2000);
          break;
        }

        default:
          return prev;
      }

      return { ...prev, [jobId]: updated };
    });

    // Invalidate queries on job completion
    if (event.event_type === 'job_completed' || event.event_type === 'job_cancelled') {
      queryClient.invalidateQueries({ queryKey: ['jobs'] });
    }

    // Invalidate job history when a job is persisted to history
    if (event.event_type === 'job_history_updated') {
      console.log('[SSE] Job history updated, refreshing history');
      queryClient.invalidateQueries({ queryKey: ['job-history'] });
    }
  }, [queryClient]);

  // Disconnect function
  const disconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
    }
    setIsConnected(false);
    setIsConnecting(false);
  }, []);

  // Connect function
  const connect = useCallback(() => {
    disconnect();
    setIsConnecting(true);
    setConnectionError(null);

    const eventSource = new EventSource(sseUrl);
    eventSourceRef.current = eventSource;

    eventSource.onopen = () => {
      setIsConnected(true);
      setIsConnecting(false);
      setReconnectAttempts(0);
      console.log('[SSE] Connected to', sseUrl);
    };

    // Handler for processing SSE event data
    const handleEventData = (eventType: string, event: MessageEvent) => {
      try {
        const rawData = JSON.parse(event.data) as BackendSSEEvent;
        if (import.meta.env.DEV) {
          console.log('[SSE] Raw event:', eventType, rawData);
        }
        const transformedEvent = transformBackendEvent(rawData);
        if (transformedEvent) {
          processEvent(transformedEvent);
        }
      } catch (e) {
        console.error('Failed to parse SSE event:', e, event.data);
      }
    };

    // Listen for named SSE events
    const eventTypes: SSEEventType[] = [
      'job_started',
      'progress',
      'item_processed',
      'item_discovered',
      'item_started',
      'error',
      'job_completed',
      'job_cancelled',
      'job_history_updated',
      'code_extraction',
      'heartbeat',
    ];

    eventTypes.forEach((eventType) => {
      eventSource.addEventListener(eventType, (event) => {
        handleEventData(eventType, event as MessageEvent);
      });
    });

    // Fallback for unnamed messages
    eventSource.onmessage = (event) => {
      handleEventData('unnamed', event);
    };

    eventSource.onerror = (event) => {
      setConnectionError(event);
      setIsConnected(false);
      setIsConnecting(false);
      console.warn('[SSE] Connection error, will retry...');

      // Attempt reconnection
      if (reconnectAttempts < maxReconnectAttempts) {
        reconnectTimeoutRef.current = setTimeout(() => {
          setReconnectAttempts((prev) => prev + 1);
          connect();
        }, reconnectInterval);
      }
    };
  }, [sseUrl, disconnect, processEvent, reconnectAttempts]);

  // Manual reconnect
  const reconnect = useCallback(() => {
    setReconnectAttempts(0);
    connect();
  }, [connect]);

  // Clear pages for a job
  const clearJobLog = useCallback((jobId: string) => {
    setJobs((prev) => {
      if (!prev[jobId]) return prev;
      return {
        ...prev,
        [jobId]: { ...prev[jobId], pages: {}, errors: [] },
      };
    });
  }, []);

  // Clear all jobs
  const clearAllJobs = useCallback(() => {
    setJobs({});
    lastRateSampleTime.current = {};
  }, []);

  // Connect on mount
  useEffect(() => {
    connect();
    return () => disconnect();
  }, []);

  const value: SSEContextType = {
    isConnected,
    isConnecting,
    connectionError,
    reconnectAttempts,
    maxReconnectAttempts,
    reconnect,
    jobs,
    clearJobLog,
    clearAllJobs,
  };

  return <SSEContext.Provider value={value}>{children}</SSEContext.Provider>;
}

// ============================================================================
// Hook
// ============================================================================

export function useSSEContext(): SSEContextType {
  const context = useContext(SSEContext);
  if (!context) {
    throw new Error('useSSEContext must be used within an SSEProvider');
  }
  return context;
}
