import { memo, type ReactNode } from 'react';
import { FileText, Layers, Brain, AlertTriangle, Zap } from 'lucide-react';
import { RateSparkline } from './RateSparkline';

interface JobMetricsProps {
  processed: number;
  total: number;
  totalChunks: number;
  totalEmbeddings: number;
  errorCount: number;
  rate: number;
  rateHistory: number[];
  className?: string;
}

function formatNumber(num: number): string {
  if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M`;
  if (num >= 1_000) return `${(num / 1_000).toFixed(1)}K`;
  return num.toLocaleString();
}

interface MetricCardProps {
  icon: ReactNode;
  label: string;
  value: string | number;
  subValue?: string;
  variant?: 'default' | 'error' | 'success';
  className?: string;
}

const MetricCard = memo(function MetricCard({
  icon,
  label,
  value,
  subValue,
  variant = 'default',
  className = '',
}: MetricCardProps) {
  const bgStyles = {
    default: 'bg-slate-800/50',
    error: 'bg-red-950/40',
    success: 'bg-emerald-950/30',
  }[variant];

  const textStyles = {
    default: 'text-white',
    error: 'text-red-400',
    success: 'text-emerald-400',
  }[variant];

  return (
    <div className={`rounded-lg p-3 ${bgStyles} ${className}`}>
      <div className="flex items-center gap-2 mb-1.5">
        {icon}
        <span className="text-[10px] font-semibold text-slate-500 uppercase tracking-wider">
          {label}
        </span>
      </div>
      <div className={`text-lg font-bold font-mono tracking-tight ${textStyles}`}>
        {typeof value === 'number' ? formatNumber(value) : value}
      </div>
      {subValue && (
        <div className="text-[11px] text-slate-500 mt-0.5 font-medium">
          {subValue}
        </div>
      )}
    </div>
  );
});

export const JobMetrics = memo(function JobMetrics({
  processed,
  total,
  totalChunks,
  totalEmbeddings,
  errorCount,
  rate,
  rateHistory,
  className = '',
}: JobMetricsProps) {
  // Calculate derived values
  const percentage = total > 0 ? ((processed / total) * 100).toFixed(1) : '0.0';
  const avgChunksPerItem = processed > 0 ? (totalChunks / processed).toFixed(1) : '0.0';
  const failRate = processed > 0 && errorCount > 0
    ? `${((errorCount / processed) * 100).toFixed(1)}% fail rate`
    : undefined;

  return (
    <div className={`grid grid-cols-2 md:grid-cols-3 gap-2.5 ${className}`}>
      {/* Items processed */}
      <MetricCard
        icon={<FileText className="w-4 h-4 text-cyan-500" />}
        label="Items"
        value={`${formatNumber(processed)}${total > 0 ? ` / ${formatNumber(total)}` : ''}`}
        subValue={total > 0 ? `${percentage}% complete` : undefined}
      />

      {/* Chunks created */}
      <MetricCard
        icon={<Layers className="w-4 h-4 text-teal-500" />}
        label="Chunks"
        value={totalChunks}
        subValue={processed > 0 ? `${avgChunksPerItem} avg/item` : undefined}
      />

      {/* Embeddings generated */}
      <MetricCard
        icon={<Brain className="w-4 h-4 text-purple-500" />}
        label="Embeddings"
        value={totalEmbeddings}
      />

      {/* Errors */}
      <MetricCard
        icon={<AlertTriangle className="w-4 h-4 text-red-500" />}
        label="Errors"
        value={errorCount}
        variant={errorCount > 0 ? 'error' : 'default'}
        subValue={failRate}
      />

      {/* Rate with sparkline - spans 2 columns */}
      <div className="col-span-2 bg-slate-800/50 rounded-lg p-3">
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-2">
            <Zap className="w-4 h-4 text-amber-500" />
            <span className="text-[10px] font-semibold text-slate-500 uppercase tracking-wider">
              Rate
            </span>
          </div>
          <div className="text-lg font-bold font-mono tracking-tight text-white">
            {rate.toFixed(1)}
            <span className="text-sm text-slate-400 ml-0.5 font-normal">/sec</span>
          </div>
        </div>
        <RateSparkline
          data={rateHistory}
          width={260}
          height={36}
          className="w-full"
        />
      </div>
    </div>
  );
});
