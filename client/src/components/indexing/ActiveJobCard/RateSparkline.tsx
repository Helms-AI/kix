import { memo, useMemo, useId } from 'react';

interface RateSparklineProps {
  data: number[];
  width?: number;
  height?: number;
  color?: string;
  className?: string;
}

export const RateSparkline = memo(function RateSparkline({
  data,
  width = 200,
  height = 40,
  color = '#22d3ee',
  className = '',
}: RateSparklineProps) {
  const gradientId = useId();

  const pathData = useMemo(() => {
    if (data.length < 2) return null;

    const padding = 4;
    const chartWidth = width - padding * 2;
    const chartHeight = height - padding * 2;

    // Normalize data with some padding for visual appeal
    const maxValue = Math.max(...data, 0.1);
    const minValue = Math.min(...data, 0);
    const valueRange = maxValue - minValue || 1;

    // Calculate point positions
    const points = data.map((value, index) => {
      const x = padding + (index / (data.length - 1)) * chartWidth;
      const y = padding + chartHeight - ((value - minValue) / valueRange) * chartHeight;
      return { x, y, value };
    });

    // Create smooth bezier curve path
    let linePath = `M ${points[0].x} ${points[0].y}`;
    for (let i = 1; i < points.length; i++) {
      const prev = points[i - 1];
      const curr = points[i];
      const cpx = (prev.x + curr.x) / 2;
      linePath += ` Q ${cpx} ${prev.y} ${(cpx + curr.x) / 2} ${(prev.y + curr.y) / 2}`;
    }
    // Final point
    const last = points[points.length - 1];
    linePath += ` L ${last.x} ${last.y}`;

    // Area fill path (same as line but closed at bottom)
    const fillPath = `
      ${linePath}
      L ${last.x} ${height - padding}
      L ${padding} ${height - padding}
      Z
    `;

    return { linePath, fillPath, points, lastPoint: last };
  }, [data, width, height]);

  if (!pathData) {
    return (
      <div
        className={`flex items-center justify-center text-slate-600 text-xs font-mono ${className}`}
        style={{ width, height }}
      >
        <span className="opacity-60">No data</span>
      </div>
    );
  }

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={`overflow-visible ${className}`}
      preserveAspectRatio="none"
    >
      <defs>
        {/* Gradient fill for area under curve */}
        <linearGradient id={gradientId} x1="0%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stopColor={color} stopOpacity="0.25" />
          <stop offset="100%" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>

      {/* Area fill */}
      <path
        d={pathData.fillPath}
        fill={`url(#${gradientId})`}
        className="transition-all duration-300"
      />

      {/* Line stroke */}
      <path
        d={pathData.linePath}
        fill="none"
        stroke={color}
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
        className="transition-all duration-300"
      />

      {/* Latest point with pulse effect */}
      <g>
        {/* Outer pulse ring */}
        <circle
          cx={pathData.lastPoint.x}
          cy={pathData.lastPoint.y}
          r={6}
          fill={color}
          className="animate-ping opacity-30"
        />
        {/* Inner dot */}
        <circle
          cx={pathData.lastPoint.x}
          cy={pathData.lastPoint.y}
          r={3}
          fill={color}
          className="drop-shadow-[0_0_4px_rgba(34,211,238,0.5)]"
        />
      </g>
    </svg>
  );
});
