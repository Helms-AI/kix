import React from 'react';
import { CodeExtractionStats } from '../../../api/indexingClient';
import { LanguageBreakdown } from './LanguageBreakdown';
import { PatternMatches } from './PatternMatches';
import { ValidationStats } from './ValidationStats';

interface Props {
  stats: CodeExtractionStats | null;
  isLoading?: boolean;
  /** Use compact layout for embedded views */
  compact?: boolean;
}

export const CodeExtractionPanel: React.FC<Props> = ({ stats, isLoading, compact = false }) => {
  if (isLoading) {
    return (
      <div className="bg-slate-800/50 rounded-lg border border-slate-700/50 p-4">
        <div className="animate-pulse">
          <div className="h-5 bg-slate-700 rounded w-1/3 mb-3"></div>
          <div className="space-y-2">
            <div className="h-3 bg-slate-700 rounded w-full"></div>
            <div className="h-3 bg-slate-700 rounded w-2/3"></div>
            <div className="h-3 bg-slate-700 rounded w-3/4"></div>
          </div>
        </div>
      </div>
    );
  }

  if (!stats || stats.total_code_blocks === 0) {
    return null;
  }

  // Compact layout for embedded views
  if (compact) {
    return (
      <div className="bg-slate-800/50 rounded-lg border border-slate-700/50 overflow-hidden">
        {/* Header */}
        <div className="px-4 py-3 border-b border-slate-700/50 bg-gradient-to-r from-slate-800 to-slate-800/50">
          <h4 className="text-sm font-semibold text-slate-200 flex items-center gap-2">
            <span className="text-cyan-400">{'</>'}</span>
            Code Extraction
          </h4>
          <p className="mt-0.5 text-xs text-slate-400">
            {stats.total_code_blocks.toLocaleString()} blocks from{' '}
            {stats.pages_with_code.toLocaleString()} pages
          </p>
        </div>

        {/* Compact Stats */}
        <div className="p-4 grid grid-cols-2 gap-3">
          <div className="flex items-center gap-2">
            <span className="text-lg font-semibold text-cyan-400 tabular-nums">
              {stats.languages.length}
            </span>
            <span className="text-xs text-slate-400">Languages</span>
          </div>
          <div className="flex items-center gap-2">
            <span className={`text-lg font-semibold tabular-nums ${getPassRateColor(stats.validation.pass_rate)}`}>
              {stats.validation.pass_rate.toFixed(0)}%
            </span>
            <span className="text-xs text-slate-400">Valid</span>
          </div>
        </div>

        {/* Top Languages */}
        {stats.languages.length > 0 && (
          <div className="px-4 pb-4">
            <div className="flex flex-wrap gap-1.5">
              {stats.languages.slice(0, 5).map((lang) => (
                <span
                  key={lang.language}
                  className="px-2 py-0.5 text-xs rounded-full bg-slate-700/50 text-slate-300 font-medium"
                >
                  {lang.language} ({lang.block_count})
                </span>
              ))}
              {stats.languages.length > 5 && (
                <span className="px-2 py-0.5 text-xs text-slate-500">
                  +{stats.languages.length - 5} more
                </span>
              )}
            </div>
          </div>
        )}
      </div>
    );
  }

  // Full layout
  return (
    <div className="bg-slate-800/50 rounded-lg border border-slate-700/50 overflow-hidden">
      {/* Header */}
      <div className="px-6 py-4 border-b border-slate-700/50 bg-gradient-to-r from-slate-800 to-slate-800/50">
        <h3 className="text-lg font-semibold text-slate-100 flex items-center gap-2">
          <span className="text-cyan-400">{'</>'}</span>
          Code Extraction Results
        </h3>
        <p className="mt-1 text-sm text-slate-400">
          {stats.total_code_blocks.toLocaleString()} code blocks from{' '}
          {stats.pages_with_code.toLocaleString()} pages
        </p>
      </div>

      {/* Summary Stats */}
      <div className="px-6 py-4 grid grid-cols-2 md:grid-cols-4 gap-4 border-b border-slate-700/50">
        <StatCard
          label="Total Blocks"
          value={stats.total_code_blocks}
          icon={<CodeIcon />}
        />
        <StatCard
          label="Pages with Code"
          value={stats.pages_with_code}
          subtext={`of ${stats.total_pages} pages`}
          icon={<FileIcon />}
        />
        <StatCard
          label="Languages"
          value={stats.languages.length}
          icon={<GlobeIcon />}
        />
        <StatCard
          label="Pass Rate"
          value={`${stats.validation.pass_rate.toFixed(1)}%`}
          icon={<CheckIcon />}
          valueColor={getPassRateColor(stats.validation.pass_rate)}
        />
      </div>

      {/* Detailed Breakdowns */}
      <div className="p-6 grid grid-cols-1 lg:grid-cols-2 gap-6">
        <LanguageBreakdown languages={stats.languages} />
        <PatternMatches patterns={stats.patterns} />
      </div>

      {/* Validation Details */}
      <div className="px-6 pb-6">
        <ValidationStats validation={stats.validation} />
      </div>
    </div>
  );
};

// Stat card component
interface StatCardProps {
  label: string;
  value: string | number;
  subtext?: string;
  icon: React.ReactNode;
  valueColor?: string;
}

const StatCard: React.FC<StatCardProps> = ({
  label,
  value,
  subtext,
  icon,
  valueColor = 'text-slate-100',
}) => (
  <div className="bg-slate-900/50 rounded-lg p-4 transition-all hover:bg-slate-900/70">
    <div className="flex items-center gap-2 mb-2">
      <span className="text-slate-500">{icon}</span>
      <span className="text-sm font-medium text-slate-400">{label}</span>
    </div>
    <div>
      <span className={`text-2xl font-semibold tabular-nums ${valueColor}`}>
        {typeof value === 'number' ? value.toLocaleString() : value}
      </span>
      {subtext && (
        <span className="ml-2 text-sm text-slate-500">{subtext}</span>
      )}
    </div>
  </div>
);

// Helper function for pass rate color
function getPassRateColor(rate: number): string {
  if (rate >= 90) return 'text-emerald-400';
  if (rate >= 70) return 'text-amber-400';
  return 'text-red-400';
}

// Simple icon components
const CodeIcon = () => (
  <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={1.5}
      d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"
    />
  </svg>
);

const FileIcon = () => (
  <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={1.5}
      d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
    />
  </svg>
);

const GlobeIcon = () => (
  <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={1.5}
      d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9"
    />
  </svg>
);

const CheckIcon = () => (
  <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={1.5}
      d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
    />
  </svg>
);

export default CodeExtractionPanel;
