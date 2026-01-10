import { useCallback, useRef } from 'react';
import type { JobLogEntry, EnhancedLiveJobData } from '../api/indexingClient';
import type { SSEEvent } from './useSSE';

const MAX_LOG_ENTRIES = 500;
const MAX_RATE_SAMPLES = 20;

export interface JobEventsState {
  [jobId: string]: EnhancedLiveJobData;
}

// Generate unique ID for log entries
let logEntryIdCounter = 0;
function generateLogEntryId(): string {
  return `log-${Date.now()}-${logEntryIdCounter++}`;
}

export function useJobEvents() {
  const stateRef = useRef<JobEventsState>({});
  const lastRateSampleTime = useRef<{ [jobId: string]: number }>({});

  const initializeJob = useCallback((jobId: string, source?: string, sourceType?: 'url' | 'file' | 'directory') => {
    if (!stateRef.current[jobId]) {
      stateRef.current[jobId] = {
        processed: 0,
        total: 0,
        percentage: 0,
        rate: 0,
        log: [],
        errors: [],
        rateHistory: [],
        totalChunks: 0,
        totalEmbeddings: 0,
        source,
        sourceType,
      };
    } else if (source) {
      stateRef.current[jobId].source = source;
      stateRef.current[jobId].sourceType = sourceType;
    }
  }, []);

  const processEvent = useCallback((event: SSEEvent): EnhancedLiveJobData | null => {
    const jobId = event.job_id;

    // Initialize job if not exists
    if (!stateRef.current[jobId]) {
      initializeJob(jobId);
    }

    const jobState = stateRef.current[jobId];
    const now = Date.now();

    switch (event.event_type) {
      case 'job_started': {
        jobState.source = event.data.source;
        jobState.sourceType = event.data.source_type;
        if (event.data.total_items) {
          jobState.total = event.data.total_items;
        }
        break;
      }

      case 'progress': {
        jobState.processed = event.data.processed ?? jobState.processed;
        jobState.total = event.data.total ?? jobState.total;
        jobState.percentage = event.data.percentage ?? jobState.percentage;
        jobState.rate = event.data.rate ?? jobState.rate;
        jobState.currentItem = event.data.current_item;
        jobState.etaSeconds = event.data.eta_seconds;

        // Sample rate for sparkline (every 2 seconds)
        const lastSample = lastRateSampleTime.current[jobId] || 0;
        if (now - lastSample >= 2000 && event.data.rate !== undefined) {
          jobState.rateHistory = [
            ...jobState.rateHistory.slice(-MAX_RATE_SAMPLES + 1),
            event.data.rate,
          ];
          lastRateSampleTime.current[jobId] = now;
        }
        break;
      }

      case 'item_processed': {
        const logEntry: JobLogEntry = {
          id: generateLogEntryId(),
          type: 'item',
          timestamp: event.timestamp,
          item_path: event.data.item_path || event.data.current_item || 'Unknown',
          chunks_created: event.data.chunks_created,
          embeddings_generated: event.data.embeddings_generated,
          duration_ms: event.data.duration_ms,
        };

        // Add to log (ring buffer)
        jobState.log = [...jobState.log.slice(-MAX_LOG_ENTRIES + 1), logEntry];

        // Update totals
        if (event.data.chunks_created) {
          jobState.totalChunks += event.data.chunks_created;
        }
        if (event.data.embeddings_generated) {
          jobState.totalEmbeddings += event.data.embeddings_generated;
        }
        break;
      }

      case 'error': {
        const errorEntry: JobLogEntry = {
          id: generateLogEntryId(),
          type: 'error',
          timestamp: event.timestamp,
          item_path: event.data.item_path || event.data.current_item || 'Unknown',
          error_message: event.data.error_message || event.data.error || 'Unknown error',
          recoverable: event.data.recoverable,
        };

        // Add to log
        jobState.log = [...jobState.log.slice(-MAX_LOG_ENTRIES + 1), errorEntry];

        // Track errors separately
        jobState.errors = [...jobState.errors, errorEntry];
        break;
      }

      case 'job_completed':
      case 'job_cancelled': {
        // Mark final state but don't remove yet (let parent handle cleanup)
        break;
      }

      case 'heartbeat':
      default:
        // No state change needed
        return null;
    }

    // Return a copy to trigger re-render
    return { ...jobState };
  }, [initializeJob]);

  const getJobState = useCallback((jobId: string): EnhancedLiveJobData | undefined => {
    return stateRef.current[jobId];
  }, []);

  const clearJob = useCallback((jobId: string) => {
    delete stateRef.current[jobId];
    delete lastRateSampleTime.current[jobId];
  }, []);

  const clearAllJobs = useCallback(() => {
    stateRef.current = {};
    lastRateSampleTime.current = {};
  }, []);

  return {
    processEvent,
    getJobState,
    initializeJob,
    clearJob,
    clearAllJobs,
  };
}
