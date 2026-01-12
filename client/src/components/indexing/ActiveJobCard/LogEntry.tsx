import { memo } from 'react';
import { CheckCircle, XCircle, AlertTriangle } from 'lucide-react';
import type { JobLogEntry } from '../../../api/indexingClient';

interface LogEntryProps {
  entry: JobLogEntry;
  isNew?: boolean;
}

function formatTimestamp(timestamp: string): string {
  try {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('en-US', {
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  } catch {
    return '--:--:--';
  }
}

function formatDuration(ms: number | undefined): string {
  if (ms === undefined || ms === 0) return '0ms';
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function truncatePath(path: string, maxLength: number = 50): string {
  if (path.length <= maxLength) return path;

  // For URLs, try to show domain + end of path
  try {
    const url = new URL(path);
    const domain = url.hostname;
    const pathPart = url.pathname;

    if (domain.length + 10 < maxLength) {
      const remainingSpace = maxLength - domain.length - 4; // ".../" takes 4 chars
      return domain + '/...' + pathPart.slice(-remainingSpace);
    }
    return domain + '/...';
  } catch {
    // Not a URL, truncate from middle
    const halfLen = Math.floor((maxLength - 3) / 2);
    return path.slice(0, halfLen) + '...' + path.slice(-halfLen);
  }
}

export const LogEntry = memo(function LogEntry({
  entry,
  isNew = false,
}: LogEntryProps) {
  const isError = entry.type === 'error';
  const isWarning = isError && entry.recoverable;

  // Choose icon based on status
  const StatusIcon = isError
    ? isWarning
      ? AlertTriangle
      : XCircle
    : CheckCircle;

  const iconColor = isError
    ? isWarning
      ? 'text-amber-500'
      : 'text-red-500'
    : 'text-cyan-500';

  return (
    <div
      className={`
        group flex items-center gap-3 py-2 px-3 font-mono text-xs
        transition-colors duration-200
        ${isError
          ? 'bg-red-950/30 border-l-2 border-red-500/70 hover:bg-red-950/40'
          : 'hover:bg-slate-800/60 border-l-2 border-transparent'
        }
        ${isNew ? 'animate-[slideIn_0.2s_ease-out]' : ''}
      `}
    >
      {/* Status icon */}
      <StatusIcon className={`w-3.5 h-3.5 flex-shrink-0 ${iconColor}`} />

      {/* Timestamp */}
      <span className="flex-shrink-0 text-slate-500 w-16 tabular-nums">
        {formatTimestamp(entry.timestamp)}
      </span>

      {/* Path/URL */}
      <span
        className={`
          flex-1 truncate
          ${isError ? 'text-red-300/90' : 'text-slate-300'}
        `}
        title={entry.item_path}
      >
        {truncatePath(entry.item_path)}
      </span>

      {/* Stats or Error message */}
      <div className="flex-shrink-0 text-right flex items-center gap-3">
        {isError ? (
          <span
            className="text-red-400/80 max-w-[180px] truncate"
            title={entry.error_message}
          >
            {entry.error_message && entry.error_message.length > 30
              ? entry.error_message.slice(0, 30) + '...'
              : entry.error_message || 'Error'}
          </span>
        ) : (
          <>
            {entry.chunks_created !== undefined && (
              <span className="text-slate-500">
                <span className="text-teal-400/80">{entry.chunks_created}</span>
                {' chunks'}
              </span>
            )}
            <span className="text-slate-600 tabular-nums w-12 text-right">
              {formatDuration(entry.duration_ms)}
            </span>
          </>
        )}
      </div>
    </div>
  );
});
