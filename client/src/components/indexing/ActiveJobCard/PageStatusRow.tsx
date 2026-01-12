import { memo } from 'react';
import { Clock, Loader2, CheckCircle, XCircle } from 'lucide-react';
import type { PageState } from '../../../api/indexingClient';

interface PageStatusRowProps {
  page: PageState;
  style?: React.CSSProperties;
}

function formatDuration(ms: number | undefined): string {
  if (ms === undefined || ms === 0) return '0ms';
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function truncateUrl(url: string | undefined, maxLength: number = 45): string {
  if (!url) return 'Unknown URL';
  if (url.length <= maxLength) return url;

  try {
    const parsed = new URL(url);
    const domain = parsed.hostname;
    const pathPart = parsed.pathname;

    if (domain.length + 10 < maxLength) {
      const remainingSpace = maxLength - domain.length - 4;
      return domain + '/...' + pathPart.slice(-remainingSpace);
    }
    return domain + '/...';
  } catch {
    const halfLen = Math.floor((maxLength - 3) / 2);
    return url.slice(0, halfLen) + '...' + url.slice(-halfLen);
  }
}

const statusConfig = {
  pending: {
    Icon: Clock,
    iconClass: 'text-yellow-500',
    bgClass: '',
    textClass: 'text-slate-400',
    borderClass: 'border-transparent',
  },
  running: {
    Icon: Loader2,
    iconClass: 'text-blue-400 animate-spin',
    bgClass: 'bg-blue-950/20',
    textClass: 'text-blue-200',
    borderClass: 'border-blue-500/50',
  },
  completed: {
    Icon: CheckCircle,
    iconClass: 'text-green-500',
    bgClass: '',
    textClass: 'text-slate-300',
    borderClass: 'border-transparent',
  },
  error: {
    Icon: XCircle,
    iconClass: 'text-red-500',
    bgClass: 'bg-red-950/20',
    textClass: 'text-red-300',
    borderClass: 'border-red-500/50',
  },
};

export const PageStatusRow = memo(function PageStatusRow({
  page,
  style,
}: PageStatusRowProps) {
  const config = statusConfig[page.status];
  const { Icon, iconClass, bgClass, textClass, borderClass } = config;

  return (
    <div
      style={style}
      className={`
        group flex items-center gap-3 py-2 px-3 font-mono text-xs
        transition-colors duration-200 border-l-2
        ${bgClass} ${borderClass}
        hover:bg-slate-800/60
      `}
    >
      {/* Status icon */}
      <Icon className={`w-3.5 h-3.5 flex-shrink-0 ${iconClass}`} />

      {/* URL */}
      <span
        className={`flex-1 truncate ${textClass}`}
        title={page.url}
      >
        {truncateUrl(page.url)}
      </span>

      {/* Metadata based on status */}
      <div className="flex-shrink-0 text-right flex items-center gap-3">
        {page.status === 'completed' && (
          <>
            {page.chunksCreated !== undefined && (
              <span className="text-slate-500">
                <span className="text-teal-400/80">{page.chunksCreated}</span>
                {' chunks'}
              </span>
            )}
            <span className="text-slate-600 tabular-nums w-12 text-right">
              {formatDuration(page.durationMs)}
            </span>
          </>
        )}

        {page.status === 'error' && page.errorMessage && (
          <span
            className="text-red-400/80 max-w-[180px] truncate"
            title={page.errorMessage}
          >
            {page.errorMessage.length > 30
              ? page.errorMessage.slice(0, 30) + '...'
              : page.errorMessage}
          </span>
        )}

        {page.status === 'running' && (
          <span className="text-blue-400/70 text-[10px]">Processing...</span>
        )}

        {page.status === 'pending' && page.depth !== undefined && (
          <span className="text-slate-600 text-[10px]">
            depth {page.depth}
          </span>
        )}
      </div>
    </div>
  );
});
