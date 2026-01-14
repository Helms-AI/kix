import React from 'react';
import { ValidationSummary } from '../../../api/indexingClient';

interface Props {
  validation: ValidationSummary;
}

export const ValidationStats: React.FC<Props> = ({ validation }) => {
  const rejected = validation.total_extracted - validation.passed;
  const passRate = validation.pass_rate;

  // Color based on pass rate
  const getPassRateColor = (rate: number): string => {
    if (rate >= 90) return 'text-emerald-400';
    if (rate >= 70) return 'text-amber-400';
    return 'text-red-400';
  };

  const getProgressColor = (rate: number): string => {
    if (rate >= 90) return '#34d399'; // emerald-400
    if (rate >= 70) return '#fbbf24'; // amber-400
    return '#f87171'; // red-400
  };

  return (
    <div className="bg-slate-900/50 rounded-lg p-4">
      <h4 className="text-sm font-medium text-slate-300 mb-4 flex items-center gap-2">
        <span className="text-lg text-cyan-400">✓</span>
        Code Validation
      </h4>

      <div className="flex items-center gap-6">
        {/* Pass rate gauge */}
        <div className="relative w-24 h-24 flex-shrink-0">
          <svg
            className="w-full h-full transform -rotate-90"
            viewBox="0 0 100 100"
            aria-label={`Pass rate: ${passRate.toFixed(0)}%`}
          >
            {/* Background circle */}
            <circle
              cx="50"
              cy="50"
              r="40"
              fill="none"
              stroke="#334155"
              strokeWidth="8"
            />
            {/* Progress circle */}
            <circle
              cx="50"
              cy="50"
              r="40"
              fill="none"
              stroke={getProgressColor(passRate)}
              strokeWidth="8"
              strokeLinecap="round"
              strokeDasharray={`${passRate * 2.51} 251`}
              className="transition-all duration-700 ease-out"
            />
          </svg>
          <div className="absolute inset-0 flex items-center justify-center">
            <span className={`text-xl font-bold ${getPassRateColor(passRate)}`}>
              {passRate.toFixed(0)}%
            </span>
          </div>
        </div>

        {/* Stats */}
        <div className="flex-1 grid grid-cols-2 gap-4">
          <div>
            <div className="text-2xl font-semibold text-slate-100 tabular-nums">
              {validation.passed.toLocaleString()}
            </div>
            <div className="text-sm text-slate-400">Passed validation</div>
          </div>
          <div>
            <div className="text-2xl font-semibold text-slate-500 tabular-nums">
              {rejected.toLocaleString()}
            </div>
            <div className="text-sm text-slate-400">Rejected</div>
          </div>
        </div>
      </div>

      {/* Rejection reasons */}
      {validation.rejection_reasons.length > 0 && (
        <div className="border-t border-slate-700/50 mt-4 pt-4">
          <h5 className="text-xs font-medium text-slate-500 uppercase tracking-wide mb-2">
            Rejection Reasons
          </h5>
          <div className="flex flex-wrap gap-2">
            {validation.rejection_reasons.map((reason) => (
              <span
                key={reason.reason}
                className="inline-flex items-center px-2.5 py-1 rounded-full text-xs bg-slate-700/50 text-slate-300 font-medium"
              >
                {formatRejectionReason(reason.reason)}: {reason.count.toLocaleString()}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

function formatRejectionReason(reason: string): string {
  const labels: Record<string, string> = {
    too_short: 'Too short',
    high_prose: 'Prose content',
    placeholder: 'Placeholder',
    no_structure: 'No structure',
    duplicate: 'Duplicate',
  };
  return labels[reason] || reason.replace(/_/g, ' ');
}

export default ValidationStats;
