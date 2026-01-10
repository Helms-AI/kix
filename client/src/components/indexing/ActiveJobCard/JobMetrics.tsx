import React from 'react';
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
  if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`;
  if (num >= 1000) return `${(num / 1000).toFixed(1)}K`;
  return num.toLocaleString();
}

interface MetricCardProps {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  subValue?: string;
  variant?: 'default' | 'error' | 'success';
}

function MetricCard({ icon, label, value, subValue, variant = 'default' }: MetricCardProps) {
  const bgColor = {
    default: 'bg-slate-800/50',
    error: 'bg-red-900/20',
    success: 'bg-emerald-900/20',
  }[variant];

  const textColor = {
    default: 'text-white',
    error: 'text-red-400',
    success: 'text-emerald-400',
  }[variant];

  return (
    <div className={`${bgColor} rounded-lg p-3`}>
      <div className="flex items-center gap-2 mb-1">
        {icon}
        <span className="text-xs text-slate-400 uppercase tracking-wide">{label}</span>
      </div>
      <div className={`text-lg font-semibold ${textColor}`}>
        {typeof value === 'number' ? formatNumber(value) : value}
      </div>
      {subValue && (
        <div className="text-xs text-slate-500 mt-0.5">{subValue}</div>
      )}
    </div>
  );
}

export function JobMetrics({
  processed,
  total,
  totalChunks,
  totalEmbeddings,
  errorCount,
  rate,
  rateHistory,
  className = '',
}: JobMetricsProps) {
  return (
    <div className={`grid grid-cols-2 md:grid-cols-3 gap-3 ${className}`}>
      {/* Items processed */}
      <MetricCard
        icon={<FileText className="w-4 h-4 text-cyan-500" />}
        label="Items"
        value={`${formatNumber(processed)}${total > 0 ? ` / ${formatNumber(total)}` : ''}`}
        subValue={total > 0 ? `${((processed / total) * 100).toFixed(1)}% complete` : undefined}
      />

      {/* Chunks created */}
      <MetricCard
        icon={<Layers className="w-4 h-4 text-teal-500" />}
        label="Chunks"
        value={totalChunks}
        subValue={processed > 0 ? `${(totalChunks / processed).toFixed(1)} avg/item` : undefined}
      />

      {/* Embeddings */}
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
        subValue={processed > 0 && errorCount > 0 ? `${((errorCount / processed) * 100).toFixed(1)}% fail rate` : undefined}
      />

      {/* Processing rate with sparkline */}
      <div className="col-span-2 bg-slate-800/50 rounded-lg p-3">
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-2">
            <Zap className="w-4 h-4 text-amber-500" />
            <span className="text-xs text-slate-400 uppercase tracking-wide">Rate</span>
          </div>
          <span className="text-lg font-semibold text-white">
            {rate.toFixed(1)}<span className="text-sm text-slate-400 ml-1">/sec</span>
          </span>
        </div>
        <RateSparkline
          data={rateHistory}
          width={260}
          height={40}
          className="w-full"
        />
      </div>
    </div>
  );
}
