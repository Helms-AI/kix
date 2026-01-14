import React, { useState } from 'react';
import { PatternStats } from '../../../api/indexingClient';

interface Props {
  patterns: PatternStats[];
}

export const PatternMatches: React.FC<Props> = ({ patterns }) => {
  const [expanded, setExpanded] = useState(false);

  const sortedPatterns = [...patterns].sort(
    (a, b) => b.match_count - a.match_count
  );
  const displayPatterns = expanded
    ? sortedPatterns
    : sortedPatterns.slice(0, 5);
  const maxCount = sortedPatterns[0]?.match_count || 1;

  if (sortedPatterns.length === 0) {
    return (
      <div className="text-sm text-slate-500 italic">
        No patterns matched
      </div>
    );
  }

  return (
    <div>
      <h4 className="text-sm font-medium text-slate-300 mb-3 flex items-center gap-2">
        <span className="text-lg">{'{ }'}</span>
        Extraction Patterns Used
      </h4>

      <div className="space-y-2">
        {displayPatterns.map((pattern) => (
          <div key={pattern.pattern} className="flex items-center gap-3">
            {/* Pattern icon */}
            <span className="text-cyan-400 text-sm flex-shrink-0" aria-hidden="true">
              #
            </span>

            {/* Pattern name */}
            <span
              className="w-36 text-sm text-slate-300 truncate font-medium"
              title={pattern.pattern}
            >
              {formatPatternName(pattern.pattern)}
            </span>

            {/* Progress bar */}
            <div className="flex-1 h-2 bg-slate-700/50 rounded-full overflow-hidden">
              <div
                className="h-full bg-cyan-500 rounded-full transition-all duration-500 ease-out"
                style={{
                  width: `${(pattern.match_count / maxCount) * 100}%`,
                }}
                role="progressbar"
                aria-valuenow={pattern.match_count}
                aria-valuemax={maxCount}
              />
            </div>

            {/* Count */}
            <span className="w-16 text-sm text-slate-400 text-right tabular-nums">
              {pattern.match_count}
            </span>

            {/* Percentage */}
            <span className="w-12 text-xs text-slate-500 text-right tabular-nums">
              {pattern.percentage.toFixed(1)}%
            </span>
          </div>
        ))}
      </div>

      {sortedPatterns.length > 5 && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="mt-3 text-sm text-cyan-400 hover:text-cyan-300 focus:outline-none focus:underline transition-colors"
        >
          {expanded ? 'Show less' : `Show ${sortedPatterns.length - 5} more`}
        </button>
      )}
    </div>
  );
};

/** Format pattern name for display */
function formatPatternName(name: string): string {
  // Convert DocusaurusCodeBlock -> Docusaurus
  return name
    .replace(/CodeBlock$/, '')
    .replace(/Code$/, '')
    .replace(/([a-z])([A-Z])/g, '$1 $2');
}

export default PatternMatches;
