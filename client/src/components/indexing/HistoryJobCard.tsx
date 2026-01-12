import { memo, useState, useCallback } from 'react';
import {
  Globe,
  FileText,
  ChevronDown,
  ChevronUp,
  CheckCircle,
  XCircle,
  Loader2,
  AlertTriangle,
  Zap,
} from 'lucide-react';
import type {
  PersistedJob,
  PersistedJobItem,
  PageState,
} from '../../api/indexingClient';
import {
  indexingApi,
  convertPersistedItemsToPages,
} from '../../api/indexingClient';
import { ProgressRing } from './ActiveJobCard/ProgressRing';
import { JobMetrics } from './ActiveJobCard/JobMetrics';
import { JobDuration } from './ActiveJobCard/JobDuration';
import { PageStatusList } from './ActiveJobCard/PageStatusList';

interface HistoryJobCardProps {
  job: PersistedJob;
  isExpanded: boolean;
  onToggleExpand: () => void;
}

// Status configuration for history jobs
const statusConfig: Record<PersistedJob['status'], {
  icon: typeof CheckCircle;
  label: string;
  className: string;
}> = {
  completed: {
    icon: CheckCircle,
    label: 'Completed',
    className: 'bg-emerald-900/50 text-emerald-400',
  },
  failed: {
    icon: XCircle,
    label: 'Failed',
    className: 'bg-red-900/50 text-red-400',
  },
  cancelled: {
    icon: XCircle,
    label: 'Cancelled',
    className: 'bg-slate-700/60 text-slate-400',
  },
};

function truncateSource(source: string | null, maxLength: number = 35): string {
  if (!source) return 'Unknown source';

  if (source.length <= maxLength) return source;

  // For URLs, show domain + truncated path
  try {
    const url = new URL(source);
    const domain = url.hostname;
    const path = url.pathname;
    const available = maxLength - domain.length - 4;
    if (available > 5) {
      return `${domain}${path.slice(0, available)}...`;
    }
    return `${domain}/...`;
  } catch {
    return source.slice(0, maxLength - 3) + '...';
  }
}

function formatRelativeTime(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) return 'Just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays === 1) return 'Yesterday';
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

// Compact view for history job
const HistoryCompactView = memo(function HistoryCompactView({
  job,
  isExpanded,
  onToggle,
}: {
  job: PersistedJob;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  const JobIcon = job.job_type === 'url' ? Globe : FileText;
  const config = statusConfig[job.status];
  const StatusIcon = config.icon;

  const jobTypeLabel = {
    url: 'URL crawl',
    file_upload: 'File upload',
    reindex: 'Reindex',
  }[job.job_type];

  return (
    <div
      className="flex items-center gap-4 p-4 cursor-pointer select-none group"
      onClick={onToggle}
    >
      {/* Completed Progress Ring */}
      <ProgressRing
        percentage={100}
        size={56}
        strokeWidth={4}
        animated={false}
      />

      {/* Job Info */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-0.5">
          <JobIcon className="w-4 h-4 text-cyan-500 flex-shrink-0" />
          <span
            className="text-sm font-medium text-white truncate"
            title={job.source_url || undefined}
          >
            {truncateSource(job.source_url)}
          </span>
        </div>
        <div className="flex items-center gap-2 text-xs text-slate-500 font-medium">
          <span>{jobTypeLabel}</span>
          <span className="text-slate-600">·</span>
          <span>{formatRelativeTime(job.completed_at)}</span>
        </div>
      </div>

      {/* Inline Stats */}
      <div className="flex items-center gap-4 text-sm">
        {/* Items processed */}
        <div className="text-slate-400 font-mono">
          <span className="text-white font-semibold">
            {job.stats.items_processed.toLocaleString()}
          </span>
          {job.stats.items_discovered > 0 && (
            <span className="text-slate-500">
              /{job.stats.items_discovered.toLocaleString()}
            </span>
          )}
        </div>

        {/* Average rate */}
        {job.stats.rate > 0 && (
          <div className="hidden sm:flex items-center gap-1 text-slate-400">
            <Zap className="w-3.5 h-3.5 text-amber-500" />
            <span className="font-mono">{job.stats.rate.toFixed(1)}/s</span>
          </div>
        )}

        {/* Error count */}
        {job.stats.error_count > 0 && (
          <div className="flex items-center gap-1 text-red-400">
            <AlertTriangle className="w-3.5 h-3.5" />
            <span className="font-mono">{job.stats.error_count}</span>
          </div>
        )}
      </div>

      {/* Status Badge */}
      <div
        className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-semibold ${config.className}`}
      >
        <StatusIcon className="w-3 h-3" />
        {config.label}
      </div>

      {/* Expand/Collapse Chevron */}
      <div className="text-slate-500 group-hover:text-slate-400 transition-colors">
        {isExpanded ? (
          <ChevronUp className="w-5 h-5" />
        ) : (
          <ChevronDown className="w-5 h-5" />
        )}
      </div>
    </div>
  );
});

// Expanded view for history job
const HistoryExpandedView = memo(function HistoryExpandedView({
  job,
  items,
  isLoading,
  error,
}: {
  job: PersistedJob;
  items: PersistedJobItem[] | null;
  isLoading: boolean;
  error: string | null;
}) {
  // Convert persisted items to PageState format
  const pages: Record<string, PageState> = items
    ? convertPersistedItemsToPages(items)
    : {};

  return (
    <div className="px-4 pb-4 space-y-4 border-t border-slate-700/50">
      {/* Metrics Grid */}
      <JobMetrics
        processed={job.stats.items_processed}
        total={job.stats.items_discovered}
        totalChunks={job.stats.chunks_created}
        totalEmbeddings={job.stats.embeddings_generated}
        errorCount={job.stats.error_count}
        rate={job.stats.rate}
        rateHistory={[]} // No history for completed jobs
        className="pt-4"
      />

      {/* Job Duration */}
      <JobDuration durationMs={job.stats.duration_ms} />

      {/* Page Status List or Loading State */}
      {isLoading ? (
        <div className="flex items-center justify-center py-8 text-slate-500">
          <Loader2 className="w-5 h-5 animate-spin mr-2" />
          <span>Loading job details...</span>
        </div>
      ) : error ? (
        <div className="flex items-center justify-center py-8 text-red-400">
          <AlertTriangle className="w-5 h-5 mr-2" />
          <span>{error}</span>
        </div>
      ) : (
        <PageStatusList pages={pages} maxHeight={200} />
      )}
    </div>
  );
});

export const HistoryJobCard = memo(function HistoryJobCard({
  job,
  isExpanded,
  onToggleExpand,
}: HistoryJobCardProps) {
  const [items, setItems] = useState<PersistedJobItem[] | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleToggle = useCallback(async () => {
    // If expanding and we haven't loaded items yet, fetch them
    if (!isExpanded && !items && !isLoading) {
      setIsLoading(true);
      setError(null);
      try {
        const detail = await indexingApi.getJobDetail(job.job_id);
        setItems(detail.items);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load details');
      } finally {
        setIsLoading(false);
      }
    }
    onToggleExpand();
  }, [isExpanded, items, isLoading, job.job_id, onToggleExpand]);

  return (
    <div
      className={`
        bg-slate-800/80 backdrop-blur-sm rounded-xl border overflow-hidden
        transition-all duration-300 ease-out
        border-slate-700/50
        ${isExpanded
          ? 'ring-1 ring-cyan-500/20'
          : 'hover:border-cyan-700/40 hover:shadow-md hover:shadow-cyan-950/20'
        }
      `}
    >
      {/* Compact view - always visible */}
      <HistoryCompactView
        job={job}
        isExpanded={isExpanded}
        onToggle={handleToggle}
      />

      {/* Expanded view - animated visibility */}
      <div
        className={`
          transition-all duration-300 ease-out overflow-hidden
          ${isExpanded
            ? 'max-h-[600px] opacity-100'
            : 'max-h-0 opacity-0'
          }
        `}
      >
        <HistoryExpandedView
          job={job}
          items={items}
          isLoading={isLoading}
          error={error}
        />
      </div>
    </div>
  );
});
