import { memo, useId } from 'react';
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

export const ETACountdown = memo(function ETACountdown({
  etaSeconds,
  percentage,
  className = '',
}: ETACountdownProps) {
  const shimmerId = useId();
  const hasETA = etaSeconds != null && etaSeconds > 0;
  const normalizedPercentage = Math.min(100, Math.max(0, percentage));
  const isActive = normalizedPercentage > 0 && normalizedPercentage < 100;

  return (
    <div className={`space-y-2.5 ${className}`}>
      {/* ETA text row */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Clock className="w-4 h-4 text-slate-500" />
          <span className="text-sm text-slate-400">
            {hasETA ? (
              <>
                ETA:{' '}
                <span className="text-white font-medium font-mono">
                  {formatETA(etaSeconds)}
                </span>
                {' remaining'}
              </>
            ) : (
              <span className="text-slate-500 animate-pulse">Calculating...</span>
            )}
          </span>
        </div>

        <span className="text-sm font-medium font-mono text-cyan-400">
          {normalizedPercentage.toFixed(1)}%
        </span>
      </div>

      {/* Progress bar */}
      <div className="relative h-2 bg-slate-800 rounded-full overflow-hidden">
        {/* Track with subtle inset */}
        <div className="absolute inset-0 rounded-full shadow-inner opacity-50" />

        {/* Progress fill */}
        <div
          className="absolute inset-y-0 left-0 rounded-full bg-gradient-to-r from-cyan-500 via-teal-400 to-cyan-500 transition-all duration-500 ease-out"
          style={{ width: `${normalizedPercentage}%` }}
        >
          {/* Shimmer effect for active progress */}
          {isActive && (
            <div className="absolute inset-0 overflow-hidden rounded-full">
              <svg
                className="absolute inset-0 w-full h-full"
                preserveAspectRatio="none"
              >
                <defs>
                  <linearGradient id={shimmerId} x1="0%" y1="0%" x2="100%" y2="0%">
                    <stop offset="0%" stopColor="white" stopOpacity="0" />
                    <stop offset="50%" stopColor="white" stopOpacity="0.3" />
                    <stop offset="100%" stopColor="white" stopOpacity="0" />
                    <animate
                      attributeName="x1"
                      from="-100%"
                      to="100%"
                      dur="1.5s"
                      repeatCount="indefinite"
                    />
                    <animate
                      attributeName="x2"
                      from="0%"
                      to="200%"
                      dur="1.5s"
                      repeatCount="indefinite"
                    />
                  </linearGradient>
                </defs>
                <rect
                  x="0"
                  y="0"
                  width="100%"
                  height="100%"
                  fill={`url(#${shimmerId})`}
                />
              </svg>
            </div>
          )}
        </div>

        {/* Glow at the edge of progress */}
        {isActive && normalizedPercentage > 0 && (
          <div
            className="absolute top-1/2 -translate-y-1/2 w-4 h-4 rounded-full bg-cyan-400 blur-sm opacity-60"
            style={{ left: `calc(${normalizedPercentage}% - 8px)` }}
          />
        )}
      </div>
    </div>
  );
});
