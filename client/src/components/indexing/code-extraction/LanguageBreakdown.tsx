import React from 'react';
import { LanguageStats } from '../../../api/indexingClient';

interface Props {
  languages: LanguageStats[];
}

// Language colors for visual distinction (GitHub-style colors)
const languageColors: Record<string, string> = {
  Rust: '#dea584',
  Python: '#3572A5',
  JavaScript: '#f1e05a',
  TypeScript: '#3178c6',
  Go: '#00ADD8',
  Java: '#b07219',
  'C++': '#f34b7d',
  C: '#555555',
  'C#': '#178600',
  Ruby: '#701516',
  PHP: '#4F5D95',
  Swift: '#F05138',
  Kotlin: '#A97BFF',
  HTML: '#e34c26',
  CSS: '#563d7c',
  SQL: '#e38c00',
  Shell: '#89e051',
  JSON: '#292929',
  YAML: '#cb171e',
  TOML: '#9c4221',
  Markdown: '#083fa1',
  Other: '#6b7280',
};

export const LanguageBreakdown: React.FC<Props> = ({ languages }) => {
  const sortedLanguages = [...languages].sort(
    (a, b) => b.block_count - a.block_count
  );
  const maxCount = sortedLanguages[0]?.block_count || 1;

  if (sortedLanguages.length === 0) {
    return (
      <div className="text-sm text-slate-500 italic">
        No languages detected
      </div>
    );
  }

  return (
    <div>
      <h4 className="text-sm font-medium text-slate-300 mb-3 flex items-center gap-2">
        <span className="text-lg">{'</>'}</span>
        Languages Detected
      </h4>

      <div className="space-y-2">
        {sortedLanguages.slice(0, 10).map((lang) => (
          <div key={lang.language} className="flex items-center gap-3">
            {/* Color dot */}
            <div
              className="w-3 h-3 rounded-full flex-shrink-0"
              style={{
                backgroundColor:
                  languageColors[lang.language] || languageColors.Other,
              }}
              aria-hidden="true"
            />

            {/* Language name */}
            <span className="w-24 text-sm text-slate-300 truncate font-medium">
              {lang.language}
            </span>

            {/* Progress bar */}
            <div className="flex-1 h-2 bg-slate-700/50 rounded-full overflow-hidden">
              <div
                className="h-full rounded-full transition-all duration-500 ease-out"
                style={{
                  width: `${(lang.block_count / maxCount) * 100}%`,
                  backgroundColor:
                    languageColors[lang.language] || languageColors.Other,
                }}
                role="progressbar"
                aria-valuenow={lang.block_count}
                aria-valuemax={maxCount}
              />
            </div>

            {/* Count */}
            <span className="w-20 text-sm text-slate-400 text-right tabular-nums">
              {lang.block_count} {lang.block_count === 1 ? 'block' : 'blocks'}
            </span>

            {/* Percentage */}
            <span className="w-12 text-xs text-slate-500 text-right tabular-nums">
              {lang.percentage.toFixed(1)}%
            </span>
          </div>
        ))}
      </div>

      {sortedLanguages.length > 10 && (
        <p className="mt-3 text-xs text-slate-500">
          +{sortedLanguages.length - 10} more languages
        </p>
      )}
    </div>
  );
};

export default LanguageBreakdown;
