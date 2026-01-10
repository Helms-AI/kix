import { useMemo } from 'react';

interface RateSparklineProps {
  data: number[];
  width?: number;
  height?: number;
  color?: string;
  className?: string;
}

export function RateSparkline({
  data,
  width = 120,
  height = 32,
  color = '#22d3ee',
  className = '',
}: RateSparklineProps) {
  const pathData = useMemo(() => {
    if (data.length === 0) return null;

    const padding = 2;
    const chartWidth = width - padding * 2;
    const chartHeight = height - padding * 2;

    // Normalize data
    const maxValue = Math.max(...data, 1); // Avoid division by zero
    const minValue = Math.min(...data, 0);
    const range = maxValue - minValue || 1;

    // Calculate points
    const points = data.map((value, index) => {
      const x = padding + (index / (data.length - 1 || 1)) * chartWidth;
      const y = padding + chartHeight - ((value - minValue) / range) * chartHeight;
      return { x, y };
    });

    // Build path for the line
    const linePath = points
      .map((point, index) => `${index === 0 ? 'M' : 'L'} ${point.x} ${point.y}`)
      .join(' ');

    // Build path for the fill (area under the line)
    const fillPath = `
      M ${padding} ${height - padding}
      ${points.map((point) => `L ${point.x} ${point.y}`).join(' ')}
      L ${width - padding} ${height - padding}
      Z
    `;

    return { linePath, fillPath, points };
  }, [data, width, height]);

  if (!pathData || data.length < 2) {
    // Show placeholder when no data
    return (
      <div
        className={`flex items-center justify-center text-slate-600 text-xs ${className}`}
        style={{ width, height }}
      >
        No data
      </div>
    );
  }

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={className}
    >
      <defs>
        <linearGradient id="sparklineFill" x1="0%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stopColor={color} stopOpacity="0.3" />
          <stop offset="100%" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>

      {/* Fill area */}
      <path
        d={pathData.fillPath}
        fill="url(#sparklineFill)"
        className="transition-all duration-300"
      />

      {/* Line */}
      <path
        d={pathData.linePath}
        fill="none"
        stroke={color}
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
        className="transition-all duration-300"
      />

      {/* Latest point highlight */}
      {pathData.points.length > 0 && (
        <circle
          cx={pathData.points[pathData.points.length - 1].x}
          cy={pathData.points[pathData.points.length - 1].y}
          r={2.5}
          fill={color}
          className="animate-pulse"
        />
      )}
    </svg>
  );
}
