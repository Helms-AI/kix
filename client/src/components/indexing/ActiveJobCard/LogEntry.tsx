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
  if (ms === undefined) return '';
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function truncatePath(path: string, maxLength: number = 45): string {
  if (path.length <= maxLength) return path;

  // Try to show the last meaningful part
  const parts = path.split('/');
  if (parts.length > 2) {
    const lastTwo = parts.slice(-2).join('/');
    if (lastTwo.length <= maxLength - 3) {
      return `.../${lastTwo}`;
    }
  }

  return path.slice(0, maxLength - 3) + '...';
}

export function LogEntry({ entry, isNew = false }: LogEntryProps) {
  const isError = entry.type === 'error';

  return (
    <div
      className={`
        flex items-center gap-3 py-1.5 px-2 font-mono text-xs
        ${isError ? 'bg-red-900/20 border-l-2 border-red-500' : 'hover:bg-slate-800/50'}
        ${isNew ? 'log-entry-new' : ''}
      `}
    >
      {/* Status icon */}
      <div className="flex-shrink-0">
        {isError ? (
          entry.recoverable ? (
            <AlertTriangle className="w-3.5 h-3.5 text-amber-500" />
          ) : (
            <XCircle className="w-3.5 h-3.5 text-red-500" />
          )
        ) : (
          <CheckCircle className="w-3.5 h-3.5 text-cyan-500" />
        )}
      </div>

      {/* Timestamp */}
      <span className="flex-shrink-0 text-slate-500 w-16">
        {formatTimestamp(entry.timestamp)}
      </span>

      {/* Path/URL */}
      <span
        className={`flex-1 truncate ${isError ? 'text-red-300' : 'text-slate-300'}`}
        title={entry.item_path}
      >
        {truncatePath(entry.item_path)}
      </span>

      {/* Stats or Error message */}
      <div className="flex-shrink-0 text-right min-w-[100px]">
        {isError ? (
          <span className="text-red-400 truncate max-w-[150px]" title={entry.error_message}>
            {entry.error_message && entry.error_message.length > 20
              ? entry.error_message.slice(0, 20) + '...'
              : entry.error_message}
          </span>
        ) : (
          <span className="text-slate-500">
            {entry.chunks_created !== undefined && (
              <span className="mr-2">{entry.chunks_created} chunks</span>
            )}
            {entry.duration_ms !== undefined && (
              <span>{formatDuration(entry.duration_ms)}</span>
            )}
          </span>
        )}
      </div>
    </div>
  );
}
