import {
  Globe,
  FileText,
  Clock,
  Zap,
  AlertTriangle,
  ChevronDown,
  ChevronUp,
  X,
  Loader2,
  CheckCircle,
  XCircle,
} from 'lucide-react';
import { ProgressRing } from './ProgressRing';
import type { Job, EnhancedLiveJobData } from '../../../api/indexingClient';

interface CompactJobViewProps {
  job: Job;
  liveData?: EnhancedLiveJobData;
  isExpanded: boolean;
  onToggle: () => void;
  onCancel: () => void;
}

function formatETA(seconds: number | undefined): string {
  if (!seconds || seconds <= 0) return '';
  if (seconds < 60) return `${Math.ceil(seconds)}s`;
  if (seconds < 3600) {
    const mins = Math.floor(seconds / 60);
    return `${mins}m`;
  }
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  return `${hours}h ${mins}m`;
}

function truncateSource(source: string | { url?: string; path?: string } | undefined, maxLength: number = 40): string {
  if (!source) return 'Unknown source';

  // Handle object source (from backend)
  let sourceStr: string;
  if (typeof source === 'object') {
    sourceStr = source.url || source.path || 'Unknown source';
  } else {
    sourceStr = source;
  }

  if (sourceStr.length <= maxLength) return sourceStr;

  // Try to extract domain for URLs
  try {
    const url = new URL(sourceStr);
    const domain = url.hostname;
    const path = url.pathname;
    if (domain.length + path.length <= maxLength) {
      return domain + path;
    }
    return domain + (path.length > 10 ? path.slice(0, 10) + '...' : path);
  } catch {
    return sourceStr.slice(0, maxLength - 3) + '...';
  }
}

function getStatusBadge(status: Job['status']) {
  const configs: Record<Job['status'], {
    icon: typeof Clock;
    label: string;
    className: string;
    animate?: boolean;
  }> = {
    pending: {
      icon: Clock,
      label: 'Pending',
      className: 'bg-slate-700 text-slate-300',
    },
    queued: {
      icon: Clock,
      label: 'Queued',
      className: 'bg-amber-900/50 text-amber-400 border border-amber-800/50',
    },
    running: {
      icon: Loader2,
      label: 'Running',
      className: 'bg-cyan-900/50 text-cyan-400 border border-cyan-800/50',
      animate: true,
    },
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
      className: 'bg-slate-700 text-slate-400',
    },
  };

  const config = configs[status] || configs.pending;
  const IconComponent = config.icon;

  return (
    <div className={`flex items-center gap-1.5 px-2 py-1 rounded-full text-xs font-medium ${config.className}`}>
      <IconComponent className={`w-3 h-3 ${config.animate ? 'animate-spin' : ''}`} />
      {config.label}
    </div>
  );
}

export function CompactJobView({
  job,
  liveData,
  isExpanded,
  onToggle,
  onCancel,
}: CompactJobViewProps) {
  const isActive = job.status === 'running' || job.status === 'queued';
  const percentage = liveData?.percentage ?? job.progress?.percentage ?? 0;
  const processed = liveData?.processed ?? job.progress?.processed ?? 0;
  const total = liveData?.total ?? job.progress?.total ?? 0;
  const rate = liveData?.rate ?? job.progress?.rate ?? 0;
  const errorCount = liveData?.errors?.length ?? 0;
  const source = liveData?.source ?? (job.job_type === 'url' ? 'URL' : 'File');
  const etaSeconds = liveData?.etaSeconds;

  const JobIcon = job.job_type === 'url' ? Globe : FileText;

  return (
    <div
      className="flex items-center gap-4 p-4 cursor-pointer select-none"
      onClick={onToggle}
    >
      {/* Progress Ring */}
      <ProgressRing
        percentage={percentage}
        size={56}
        strokeWidth={4}
        animated={job.status === 'running'}
      />

      {/* Job Info */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-1">
          <JobIcon className="w-4 h-4 text-cyan-500 flex-shrink-0" />
          <span className="text-sm font-medium text-white truncate" title={source}>
            {truncateSource(source)}
          </span>
        </div>
        <div className="text-xs text-slate-500">
          {job.job_type === 'url' ? 'URL crawl' : job.job_type === 'file_upload' ? 'File upload' : 'Reindex'}
        </div>
      </div>

      {/* Stats */}
      <div className="flex items-center gap-4 text-sm">
        {/* Processed count */}
        <div className="text-slate-400">
          <span className="text-white font-medium">{processed.toLocaleString()}</span>
          {total > 0 && (
            <span className="text-slate-500">/{total.toLocaleString()}</span>
          )}
        </div>

        {/* Rate */}
        {isActive && rate > 0 && (
          <div className="flex items-center gap-1 text-slate-400">
            <Zap className="w-3.5 h-3.5 text-amber-500" />
            <span>{rate.toFixed(1)}/s</span>
          </div>
        )}

        {/* ETA */}
        {isActive && etaSeconds && etaSeconds > 0 && (
          <div className="flex items-center gap-1 text-slate-400">
            <Clock className="w-3.5 h-3.5" />
            <span>{formatETA(etaSeconds)}</span>
          </div>
        )}

        {/* Error count */}
        {errorCount > 0 && (
          <div className="flex items-center gap-1 text-red-400">
            <AlertTriangle className="w-3.5 h-3.5" />
            <span>{errorCount}</span>
          </div>
        )}
      </div>

      {/* Status Badge */}
      {getStatusBadge(job.status)}

      {/* Cancel Button */}
      {isActive && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            onCancel();
          }}
          className="p-1.5 rounded-lg text-slate-400 hover:text-red-400 hover:bg-red-900/20 transition-colors"
          title="Cancel job"
        >
          <X className="w-4 h-4" />
        </button>
      )}

      {/* Expand/Collapse Chevron */}
      <div className="text-slate-500">
        {isExpanded ? (
          <ChevronUp className="w-5 h-5" />
        ) : (
          <ChevronDown className="w-5 h-5" />
        )}
      </div>
    </div>
  );
}
