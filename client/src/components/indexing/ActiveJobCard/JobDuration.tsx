import { memo } from 'react';
import { Clock, CheckCircle } from 'lucide-react';

interface JobDurationProps {
  durationMs: number;
  className?: string;
}

function formatDuration(ms: number): string {
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  if (seconds < 3600) {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return secs > 0 ? `${mins}m ${secs}s` : `${mins}m`;
  }
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
}

export const JobDuration = memo(function JobDuration({
  durationMs,
  className = '',
}: JobDurationProps) {
  return (
    <div className={`space-y-2.5 ${className}`}>
      {/* Duration text row */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Clock className="w-4 h-4 text-slate-500" />
          <span className="text-sm text-slate-400">
            Completed in{' '}
            <span className="text-white font-medium font-mono">
              {formatDuration(durationMs)}
            </span>
          </span>
        </div>

        <div className="flex items-center gap-1.5 text-emerald-400">
          <CheckCircle className="w-4 h-4" />
          <span className="text-sm font-medium font-mono">100%</span>
        </div>
      </div>

      {/* Completed progress bar */}
      <div className="relative h-2 bg-slate-800 rounded-full overflow-hidden">
        {/* Track with subtle inset */}
        <div className="absolute inset-0 rounded-full shadow-inner opacity-50" />

        {/* Full progress fill with success gradient */}
        <div className="absolute inset-y-0 left-0 right-0 rounded-full bg-gradient-to-r from-emerald-500 via-teal-400 to-emerald-500" />
      </div>
    </div>
  );
});
