import { useEffect, useRef, useCallback, useState } from 'react';

export type SSEEventType =
  | 'job_started'
  | 'progress'
  | 'item_processed'
  | 'item_discovered'
  | 'item_started'
  | 'error'
  | 'job_completed'
  | 'job_cancelled'
  | 'heartbeat';

// Backend sends this structure
interface BackendSSEEvent {
  id: string;
  timestamp: string;
  data: {
    type: SSEEventType;
    job_id: string;
    // Progress fields
    processed?: number;
    total?: number;
    percentage?: number;
    rate?: number;
    current_item?: string;
    // Item processed fields
    item_path?: string;
    chunks_created?: number;
    embeddings_generated?: number;
    duration_ms?: number;
    // Error fields
    error_message?: string;
    recoverable?: boolean;
    // Job started fields
    source?: { url?: string; depth?: number; path?: string };
    total_items?: number;
    // Job completed fields
    total_processed?: number;
    total_chunks?: number;
    errors?: string[];
    // Job cancelled fields
    reason?: string;
    // Item discovered fields
    discovered_at?: string;
    parent_url?: string;
    depth?: number;
    // Item started fields
    started_at?: string;
  };
}

// Frontend normalized structure
export interface SSEEvent {
  event_type: SSEEventType;
  job_id: string;
  timestamp: string;
  data: {
    // Progress fields
    processed?: number;
    total?: number;
    percentage?: number;
    rate?: number;
    current_item?: string;
    eta_seconds?: number;

    // Item processed fields
    item_path?: string;
    chunks_created?: number;
    embeddings_generated?: number;
    duration_ms?: number;

    // Error fields
    error_message?: string;
    recoverable?: boolean;

    // Job started fields
    source_type?: 'url' | 'file' | 'directory';
    source?: string;
    total_items?: number;

    // Legacy fields
    message?: string;
    error?: string;

    // Job completed fields
    result?: {
      items_processed: number;
      items_failed: number;
      duration_secs: number;
      total_chunks?: number;
    };

    // Item discovered fields
    discovered_at?: string;
    parent_url?: string;
    depth?: number;

    // Item started fields
    started_at?: string;
  };
}

// Transform backend event to frontend format
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
      // Progress fields
      processed: data.processed,
      total: data.total,
      percentage: data.percentage,
      rate: data.rate,
      current_item: data.current_item,
      // Item processed fields
      item_path: data.item_path,
      chunks_created: data.chunks_created,
      embeddings_generated: data.embeddings_generated,
      duration_ms: data.duration_ms,
      // Error fields
      error_message: data.error_message,
      recoverable: data.recoverable,
      // Job started fields
      source_type: sourceType,
      source: sourceValue,
      total_items: data.total_items,
      // Job completed fields
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
    },
  };
}

type TimeoutId = ReturnType<typeof setTimeout>;

interface UseSSEOptions {
  url: string;
  onEvent?: (event: SSEEvent) => void;
  onError?: (error: Event) => void;
  onOpen?: () => void;
  enabled?: boolean;
  reconnectInterval?: number;
  maxReconnectAttempts?: number;
}

interface UseSSEReturn {
  isConnected: boolean;
  isConnecting: boolean;
  error: Event | null;
  reconnectAttempts: number;
  disconnect: () => void;
  reconnect: () => void;
}

export function useSSE({
  url,
  onEvent,
  onError,
  onOpen,
  enabled = true,
  reconnectInterval = 3000,
  maxReconnectAttempts = 10,
}: UseSSEOptions): UseSSEReturn {
  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectTimeoutRef = useRef<TimeoutId | null>(null);
  const [isConnected, setIsConnected] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<Event | null>(null);
  const [reconnectAttempts, setReconnectAttempts] = useState(0);

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

  const connect = useCallback(() => {
    if (!enabled) return;

    disconnect();
    setIsConnecting(true);
    setError(null);

    const eventSource = new EventSource(url);
    eventSourceRef.current = eventSource;

    eventSource.onopen = () => {
      setIsConnected(true);
      setIsConnecting(false);
      setReconnectAttempts(0);
      onOpen?.();
    };

    // Handler for processing SSE event data
    const handleEventData = (eventType: string, event: MessageEvent) => {
      try {
        const rawData = JSON.parse(event.data) as BackendSSEEvent;
        // Debug: log raw events in development
        if (import.meta.env.DEV) {
          console.log('[SSE] Raw event:', eventType, rawData);
        }
        // Transform backend format to frontend format
        const transformedEvent = transformBackendEvent(rawData);
        // Only call onEvent for non-null events (skip heartbeats)
        if (transformedEvent) {
          onEvent?.(transformedEvent);
        }
      } catch (e) {
        console.error('Failed to parse SSE event:', e, event.data);
      }
    };

    // Listen for named SSE events (backend sends event types like 'progress', 'job_started', etc.)
    const eventTypes: SSEEventType[] = [
      'job_started',
      'progress',
      'item_processed',
      'item_discovered',
      'item_started',
      'error',
      'job_completed',
      'job_cancelled',
      'heartbeat',
    ];

    eventTypes.forEach((eventType) => {
      eventSource.addEventListener(eventType, (event) => {
        handleEventData(eventType, event as MessageEvent);
      });
    });

    // Also listen for unnamed messages (fallback)
    eventSource.onmessage = (event) => {
      handleEventData('unnamed', event);
    };

    eventSource.onerror = (event) => {
      setError(event);
      setIsConnected(false);
      setIsConnecting(false);
      onError?.(event);

      // Attempt reconnection
      if (reconnectAttempts < maxReconnectAttempts) {
        reconnectTimeoutRef.current = setTimeout(() => {
          setReconnectAttempts((prev) => prev + 1);
          connect();
        }, reconnectInterval);
      }
    };
  }, [url, enabled, onEvent, onError, onOpen, disconnect, reconnectAttempts, reconnectInterval, maxReconnectAttempts]);

  const reconnect = useCallback(() => {
    setReconnectAttempts(0);
    connect();
  }, [connect]);

  useEffect(() => {
    if (enabled) {
      connect();
    } else {
      disconnect();
    }

    return () => {
      disconnect();
    };
  }, [enabled, url]);

  return {
    isConnected,
    isConnecting,
    error,
    reconnectAttempts,
    disconnect,
    reconnect,
  };
}

// Hook for subscribing to specific job events
export function useJobSSE(jobId: string | null, onEvent?: (event: SSEEvent) => void) {
  return useSSE({
    url: jobId ? `/api/indexing/sse/${jobId}` : '/api/indexing/sse',
    onEvent,
    enabled: true,
  });
}
