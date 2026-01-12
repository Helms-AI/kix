import { memo, useId } from 'react';

interface ProgressRingProps {
  percentage: number;
  size?: number;
  strokeWidth?: number;
  animated?: boolean;
  className?: string;
}

export const ProgressRing = memo(function ProgressRing({
  percentage,
  size = 56,
  strokeWidth = 4,
  animated = true,
  className = '',
}: ProgressRingProps) {
  const gradientId = useId();
  const glowId = useId();

  const radius = (size - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  const normalizedPercentage = Math.min(100, Math.max(0, percentage));
  const strokeDashoffset = circumference - (normalizedPercentage / 100) * circumference;
  const center = size / 2;
  const displayPercentage = Math.round(normalizedPercentage);

  const isActive = animated && normalizedPercentage > 0 && normalizedPercentage < 100;

  return (
    <div
      className={`relative flex-shrink-0 ${className}`}
      style={{ width: size, height: size }}
    >
      {/* Glow effect for active state */}
      {isActive && (
        <div
          className="absolute inset-0 rounded-full animate-pulse opacity-40"
          style={{
            background: 'radial-gradient(circle, rgba(34, 211, 238, 0.3) 0%, transparent 70%)',
          }}
        />
      )}

      <svg
        width={size}
        height={size}
        viewBox={`0 0 ${size} ${size}`}
        className="transform -rotate-90"
      >
        <defs>
          {/* Progress gradient */}
          <linearGradient id={gradientId} x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#22d3ee" />
            <stop offset="50%" stopColor="#2dd4bf" />
            <stop offset="100%" stopColor="#22d3ee" />
          </linearGradient>

          {/* Glow filter */}
          <filter id={glowId} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur stdDeviation="2" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* Background track */}
        <circle
          cx={center}
          cy={center}
          r={radius}
          fill="none"
          stroke="#1e293b"
          strokeWidth={strokeWidth}
          className="opacity-80"
        />

        {/* Subtle inner ring */}
        <circle
          cx={center}
          cy={center}
          r={radius - strokeWidth}
          fill="none"
          stroke="#0f172a"
          strokeWidth={1}
          className="opacity-50"
        />

        {/* Progress arc */}
        {normalizedPercentage > 0 && (
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="none"
            stroke={`url(#${gradientId})`}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={strokeDashoffset}
            filter={isActive ? `url(#${glowId})` : undefined}
            className="transition-all duration-500 ease-out"
          />
        )}
      </svg>

      {/* Center percentage text */}
      <div className="absolute inset-0 flex items-center justify-center">
        <span
          className={`
            font-mono font-semibold tracking-tight
            ${size >= 56 ? 'text-sm' : 'text-xs'}
            ${isActive ? 'text-white' : 'text-slate-300'}
          `}
        >
          {displayPercentage}%
        </span>
      </div>
    </div>
  );
});
