import { Clock } from 'lucide-react';

interface ETACountdownProps {
  etaSeconds: number | null | undefined;
  percentage: number;
  className?: string;
}

function formatETA(seconds: number): string {
  if (seconds <= 0) return 'Almost done';
  if (seconds < 60) return `${Math.ceil(seconds)}s`;
  if (seconds < 3600) {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return secs > 0 ? `${mins}m ${secs}s` : `${mins}m`;
  }
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
}

export function ETACountdown({
  etaSeconds,
  percentage,
  className = '',
}: ETACountdownProps) {
  const hasETA = etaSeconds !== null && etaSeconds !== undefined && etaSeconds > 0;
  const displayPercentage = Math.min(100, Math.max(0, percentage));

  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      {/* ETA text */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-sm">
          <Clock className="w-4 h-4 text-slate-500" />
          <span className="text-slate-400">
            {hasETA ? (
              <>
                ETA:{' '}
                <span className="text-white font-medium">
                  {formatETA(etaSeconds)}
                </span>
                {' remaining'}
              </>
            ) : (
              <span className="text-slate-500">Calculating...</span>
            )}
          </span>
        </div>

        <span className="text-sm font-medium text-cyan-400">
          {displayPercentage.toFixed(1)}%
        </span>
      </div>

      {/* Progress bar */}
      <div className="relative h-2 bg-slate-800 rounded-full overflow-hidden">
        {/* Progress fill */}
        <div
          className="absolute inset-y-0 left-0 bg-gradient-to-r from-cyan-500 via-teal-400 to-cyan-500 rounded-full transition-all duration-300"
          style={{ width: `${displayPercentage}%` }}
        />

        {/* Shimmer effect for active progress */}
        {displayPercentage > 0 && displayPercentage < 100 && (
          <div
            className="absolute inset-y-0 left-0 overflow-hidden rounded-full"
            style={{ width: `${displayPercentage}%` }}
          >
            <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/20 to-transparent animate-shimmer" />
          </div>
        )}
      </div>
    </div>
  );
}
