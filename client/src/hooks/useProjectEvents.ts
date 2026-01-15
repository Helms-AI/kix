import { useEffect, useRef, useCallback, useState } from 'react';
import type { ProjectEvent, ProjectEventType } from '../types/project';

type TimeoutId = ReturnType<typeof setTimeout>;

interface UseProjectEventsOptions {
  projectId?: string;
  onEvent?: (event: ProjectEvent) => void;
  onError?: (error: Event) => void;
  onOpen?: () => void;
  enabled?: boolean;
  reconnectInterval?: number;
  maxReconnectAttempts?: number;
}

interface UseProjectEventsReturn {
  isConnected: boolean;
  isConnecting: boolean;
  error: Event | null;
  reconnectAttempts: number;
  disconnect: () => void;
  reconnect: () => void;
}

// Transform raw SSE data to ProjectEvent
function parseProjectEvent(data: string): ProjectEvent | null {
  try {
    const parsed = JSON.parse(data);

    // Skip heartbeat events
    if (parsed.event_type === 'heartbeat' || !parsed.event_type) {
      return null;
    }

    return {
      event_type: parsed.event_type as ProjectEventType,
      project_id: parsed.project_id,
      timestamp: parsed.timestamp || new Date().toISOString(),
      data: parsed.data || {},
    };
  } catch (e) {
    console.error('Failed to parse project event:', e, data);
    return null;
  }
}

export function useProjectEvents({
  projectId,
  onEvent,
  onError,
  onOpen,
  enabled = true,
  reconnectInterval = 3000,
  maxReconnectAttempts = 10,
}: UseProjectEventsOptions): UseProjectEventsReturn {
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

    // Build URL - use project-specific endpoint if projectId provided
    const url = projectId
      ? `/api/projects/events/${encodeURIComponent(projectId)}`
      : '/api/projects/events';

    const eventSource = new EventSource(url);
    eventSourceRef.current = eventSource;

    eventSource.onopen = () => {
      setIsConnected(true);
      setIsConnecting(false);
      setReconnectAttempts(0);
      onOpen?.();
    };

    // Handler for processing SSE event data
    const handleEventData = (event: MessageEvent) => {
      const parsedEvent = parseProjectEvent(event.data);
      if (parsedEvent) {
        onEvent?.(parsedEvent);
      }
    };

    // Listen for all project event types
    const eventTypes: ProjectEventType[] = [
      'project_created',
      'project_updated',
      'project_deleted',
      'work_item_created',
      'work_item_updated',
      'work_item_deleted',
      'entry_linked',
      'entry_unlinked',
      'heartbeat',
    ];

    eventTypes.forEach((eventType) => {
      eventSource.addEventListener(eventType, handleEventData);
    });

    // Also listen for unnamed messages (fallback)
    eventSource.onmessage = handleEventData;

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
  }, [
    projectId,
    enabled,
    onEvent,
    onError,
    onOpen,
    disconnect,
    reconnectAttempts,
    reconnectInterval,
    maxReconnectAttempts,
  ]);

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
  }, [enabled, projectId]);

  return {
    isConnected,
    isConnecting,
    error,
    reconnectAttempts,
    disconnect,
    reconnect,
  };
}

// Hook for auto-refetching queries when relevant events occur
export function useProjectEventRefetch(
  projectId: string | undefined,
  refetchFns: {
    refetchProjects?: () => void;
    refetchProject?: () => void;
    refetchWorkItems?: () => void;
    refetchEntries?: () => void;
  }
) {
  useProjectEvents({
    projectId,
    enabled: true,
    onEvent: (event) => {
      switch (event.event_type) {
        case 'project_created':
        case 'project_updated':
        case 'project_deleted':
          refetchFns.refetchProjects?.();
          refetchFns.refetchProject?.();
          break;
        case 'work_item_created':
        case 'work_item_updated':
        case 'work_item_deleted':
          refetchFns.refetchWorkItems?.();
          refetchFns.refetchProject?.(); // Refresh stats
          break;
        case 'entry_linked':
        case 'entry_unlinked':
          refetchFns.refetchEntries?.();
          refetchFns.refetchProject?.(); // Refresh stats
          break;
      }
    },
  });
}
