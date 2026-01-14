# Phase 6: UI Updates

**Duration**: 3-4 days
**Dependencies**: Phase 5
**Status**: Not Started

---

## Objective

Update the React dashboard to display code extraction metrics, language breakdowns, and enhanced job progress visibility.

---

## Implementation Guidelines

### Required: Use UX/UI Agents for Client-Side Code

**All client-side code in this phase MUST be implemented using specialized UX/UI agents (ux-designer or frontend-design or any other available) .** This ensures high design quality, accessibility compliance, and consistent user experience.

#### Available Agents

| Agent | Command | Use For |
|-------|---------|---------|
| **ux-designer** | `Task` with `subagent_type: "ux-product:ux-designer"` | UX research, design systems, component architecture |
| **interaction-designer** | `Task` with `subagent_type: "ux-product:interaction-designer"` | Micro-interactions, animations, user flows |
| **accessibility-expert** | `Task` with `subagent_type: "ux-product:accessibility-expert"` | WCAG compliance, a11y audits, inclusive design |
| **frontend-design** | `/frontend-design` skill | Production-grade UI components with high design quality |

#### Implementation Process

1. **Design Review**: Before implementing any component, use `ux-designer` agent to review the design approach
2. **Component Implementation**: Use `/frontend-design` skill for creating React components
3. **Interaction Design**: Use `interaction-designer` agent for animations and micro-interactions
4. **Accessibility Audit**: Use `accessibility-expert` agent to verify WCAG 2.1 AA compliance

#### Example Usage

```bash
# Design review for CodeExtractionPanel
Task(subagent_type: "ux-product:ux-designer", prompt: "Review the CodeExtractionPanel component design...")

# Implement component with high design quality
/frontend-design CodeExtractionPanel component with stats display, language breakdown chart...

# Verify accessibility
Task(subagent_type: "ux-product:accessibility-expert", prompt: "Audit CodeExtractionPanel for WCAG compliance...")
```

#### Design Quality Standards

- **No generic AI aesthetics** - Components must have distinctive, polished styling
- **Consistent design system** - Follow existing Tailwind patterns in the codebase
- **Responsive design** - Mobile-first approach, test at 320px, 768px, 1024px, 1440px
- **Dark mode support** - Ensure components work with dark mode (if applicable)
- **Animation polish** - Subtle, purposeful animations using Framer Motion or CSS transitions

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    UI Component Architecture                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  IndexingDashboard.tsx                                          │
│         │                                                        │
│         ├── JobList.tsx                                          │
│         │      └── JobCard.tsx (with code extraction badge)     │
│         │                                                        │
│         ├── JobDetail.tsx                                        │
│         │      ├── ProgressStages.tsx                           │
│         │      ├── CodeExtractionPanel.tsx (NEW)                │
│         │      │      ├── LanguageBreakdown.tsx (NEW)           │
│         │      │      ├── PatternMatches.tsx (NEW)              │
│         │      │      └── ValidationStats.tsx (NEW)             │
│         │      └── EventLog.tsx                                  │
│         │                                                        │
│         └── CodeBlockBrowser.tsx (NEW)                          │
│                ├── CodeBlockList.tsx (NEW)                      │
│                └── CodeBlockViewer.tsx (NEW)                    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Tasks

### 6.1 Update TypeScript Types

**File**: `client/src/types/indexing.ts` (MODIFY)

```typescript
// Existing types...

/** Code extraction statistics */
export interface CodeExtractionStats {
  jobId: string;
  totalPages: number;
  pagesWithCode: number;
  totalCodeBlocks: number;
  languages: LanguageStats[];
  patterns: PatternStats[];
  validation: ValidationSummary;
}

/** Language statistics */
export interface LanguageStats {
  language: string;
  blockCount: number;
  totalLines: number;
  percentage: number;
}

/** Pattern statistics */
export interface PatternStats {
  pattern: string;
  matchCount: number;
  percentage: number;
}

/** Validation summary */
export interface ValidationSummary {
  totalExtracted: number;
  passed: number;
  passRate: number;
  rejectionReasons: RejectionReason[];
}

export interface RejectionReason {
  reason: string;
  count: number;
}

/** Code block */
export interface CodeBlock {
  id: string;
  content: string;
  language: string;
  pattern: string;
  lineCount: number;
  sourceUrl: string;
  validated: boolean;
}

/** Pattern info */
export interface PatternInfo {
  name: string;
  cssSelector: string;
  description: string;
  exampleSites: string[];
}

/** Language info */
export interface LanguageInfo {
  name: string;
  aliases: string[];
  extensions: string[];
  treeSitterSupport: boolean;
}

/** SSE Events */
export type IndexingEvent =
  | JobStartedEvent
  | DiscoveryEvent
  | PageCrawledEvent
  | CodeExtractionEvent
  | ProgressEvent
  | ChunkCreatedEvent
  | EmbeddingGeneratedEvent
  | PageStoredEvent
  | JobCompletedEvent
  | JobFailedEvent;

export interface CodeExtractionEvent {
  type: 'code_extraction';
  jobId: string;
  url: string;
  blocksFound: number;
  patternsMatched: string[];
  languages: { language: string; count: number }[];
  validationStats: {
    totalExtracted: number;
    passedValidation: number;
    rejectedTooShort: number;
    rejectedProse: number;
    rejectedDuplicates: number;
  };
}

// ... other event types
```

---

### 6.2 Update API Client

**File**: `client/src/api/indexingClient.ts` (MODIFY)

```typescript
import {
  CodeExtractionStats,
  CodeBlock,
  PatternInfo,
  LanguageInfo,
} from '../types/indexing';

const API_BASE = '/api/indexing';

/** Get code extraction stats for a job */
export async function getCodeExtractionStats(
  jobId: string
): Promise<CodeExtractionStats> {
  const response = await fetch(`${API_BASE}/jobs/${jobId}/code-stats`);
  if (!response.ok) {
    throw new Error(`Failed to get code stats: ${response.statusText}`);
  }
  return response.json();
}

/** List code blocks for a job */
export async function listCodeBlocks(
  jobId: string,
  options?: {
    language?: string;
    pattern?: string;
    offset?: number;
    limit?: number;
  }
): Promise<CodeBlock[]> {
  const params = new URLSearchParams();
  if (options?.language) params.set('language', options.language);
  if (options?.pattern) params.set('pattern', options.pattern);
  if (options?.offset) params.set('offset', options.offset.toString());
  if (options?.limit) params.set('limit', options.limit.toString());

  const url = `${API_BASE}/jobs/${jobId}/code-blocks?${params}`;
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to list code blocks: ${response.statusText}`);
  }
  return response.json();
}

/** Get all supported patterns */
export async function getPatterns(): Promise<PatternInfo[]> {
  const response = await fetch(`${API_BASE}/patterns`);
  if (!response.ok) {
    throw new Error(`Failed to get patterns: ${response.statusText}`);
  }
  return response.json();
}

/** Get all supported languages */
export async function getLanguages(): Promise<LanguageInfo[]> {
  const response = await fetch(`${API_BASE}/languages`);
  if (!response.ok) {
    throw new Error(`Failed to get languages: ${response.statusText}`);
  }
  return response.json();
}
```

---

### 6.3 Create CodeExtractionPanel Component

**File**: `client/src/components/indexing/CodeExtractionPanel.tsx` (NEW)

```tsx
import React from 'react';
import { CodeExtractionStats } from '../../types/indexing';
import { LanguageBreakdown } from './LanguageBreakdown';
import { PatternMatches } from './PatternMatches';
import { ValidationStats } from './ValidationStats';

interface Props {
  stats: CodeExtractionStats;
  isLoading?: boolean;
}

export const CodeExtractionPanel: React.FC<Props> = ({ stats, isLoading }) => {
  if (isLoading) {
    return (
      <div className="bg-white rounded-lg shadow p-6">
        <div className="animate-pulse">
          <div className="h-6 bg-gray-200 rounded w-1/3 mb-4"></div>
          <div className="space-y-3">
            <div className="h-4 bg-gray-200 rounded w-full"></div>
            <div className="h-4 bg-gray-200 rounded w-2/3"></div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-white rounded-lg shadow">
      {/* Header */}
      <div className="px-6 py-4 border-b border-gray-200">
        <h3 className="text-lg font-medium text-gray-900">
          Code Extraction Results
        </h3>
        <p className="mt-1 text-sm text-gray-500">
          {stats.totalCodeBlocks} code blocks from {stats.pagesWithCode} pages
        </p>
      </div>

      {/* Summary Stats */}
      <div className="px-6 py-4 grid grid-cols-4 gap-4 border-b border-gray-200">
        <StatCard
          label="Total Blocks"
          value={stats.totalCodeBlocks}
          icon="code"
        />
        <StatCard
          label="Pages with Code"
          value={stats.pagesWithCode}
          subtext={`of ${stats.totalPages} pages`}
          icon="file"
        />
        <StatCard
          label="Languages"
          value={stats.languages.length}
          icon="globe"
        />
        <StatCard
          label="Pass Rate"
          value={`${stats.validation.passRate.toFixed(1)}%`}
          icon="check"
        />
      </div>

      {/* Detailed Breakdowns */}
      <div className="p-6 grid grid-cols-2 gap-6">
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

interface StatCardProps {
  label: string;
  value: string | number;
  subtext?: string;
  icon: string;
}

const StatCard: React.FC<StatCardProps> = ({ label, value, subtext, icon }) => (
  <div className="bg-gray-50 rounded-lg p-4">
    <div className="flex items-center gap-2">
      <span className="text-gray-400">{getIcon(icon)}</span>
      <span className="text-sm font-medium text-gray-500">{label}</span>
    </div>
    <div className="mt-2">
      <span className="text-2xl font-semibold text-gray-900">{value}</span>
      {subtext && (
        <span className="ml-2 text-sm text-gray-500">{subtext}</span>
      )}
    </div>
  </div>
);

function getIcon(name: string): React.ReactNode {
  // Return appropriate icon based on name
  const icons: Record<string, string> = {
    code: '< />',
    file: '📄',
    globe: '🌐',
    check: '✓',
  };
  return icons[name] || '•';
}
```

---

### 6.4 Create LanguageBreakdown Component

**File**: `client/src/components/indexing/LanguageBreakdown.tsx` (NEW)

```tsx
import React from 'react';
import { LanguageStats } from '../../types/indexing';

interface Props {
  languages: LanguageStats[];
}

// Language colors for visual distinction
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
  Scala: '#c22d40',
  HTML: '#e34c26',
  CSS: '#563d7c',
  SQL: '#e38c00',
  Bash: '#89e051',
  JSON: '#292929',
  YAML: '#cb171e',
};

export const LanguageBreakdown: React.FC<Props> = ({ languages }) => {
  const sortedLanguages = [...languages].sort(
    (a, b) => b.blockCount - a.blockCount
  );
  const maxCount = sortedLanguages[0]?.blockCount || 1;

  return (
    <div>
      <h4 className="text-sm font-medium text-gray-700 mb-3">
        Languages Detected
      </h4>

      <div className="space-y-2">
        {sortedLanguages.slice(0, 10).map((lang) => (
          <div key={lang.language} className="flex items-center gap-3">
            {/* Color dot */}
            <div
              className="w-3 h-3 rounded-full"
              style={{
                backgroundColor:
                  languageColors[lang.language] || '#6b7280',
              }}
            />

            {/* Language name */}
            <span className="w-24 text-sm text-gray-700 truncate">
              {lang.language}
            </span>

            {/* Progress bar */}
            <div className="flex-1 h-2 bg-gray-100 rounded-full overflow-hidden">
              <div
                className="h-full rounded-full transition-all duration-300"
                style={{
                  width: `${(lang.blockCount / maxCount) * 100}%`,
                  backgroundColor:
                    languageColors[lang.language] || '#6b7280',
                }}
              />
            </div>

            {/* Count */}
            <span className="w-16 text-sm text-gray-500 text-right">
              {lang.blockCount} blocks
            </span>

            {/* Percentage */}
            <span className="w-12 text-xs text-gray-400 text-right">
              {lang.percentage.toFixed(1)}%
            </span>
          </div>
        ))}
      </div>

      {sortedLanguages.length > 10 && (
        <p className="mt-2 text-xs text-gray-500">
          +{sortedLanguages.length - 10} more languages
        </p>
      )}
    </div>
  );
};
```

---

### 6.5 Create PatternMatches Component

**File**: `client/src/components/indexing/PatternMatches.tsx` (NEW)

```tsx
import React, { useState } from 'react';
import { PatternStats } from '../../types/indexing';

interface Props {
  patterns: PatternStats[];
}

export const PatternMatches: React.FC<Props> = ({ patterns }) => {
  const [expanded, setExpanded] = useState(false);
  const sortedPatterns = [...patterns].sort(
    (a, b) => b.matchCount - a.matchCount
  );
  const displayPatterns = expanded
    ? sortedPatterns
    : sortedPatterns.slice(0, 5);
  const maxCount = sortedPatterns[0]?.matchCount || 1;

  return (
    <div>
      <h4 className="text-sm font-medium text-gray-700 mb-3">
        Extraction Patterns Used
      </h4>

      <div className="space-y-2">
        {displayPatterns.map((pattern) => (
          <div key={pattern.pattern} className="flex items-center gap-3">
            {/* Pattern icon */}
            <span className="text-gray-400 text-sm">🎯</span>

            {/* Pattern name */}
            <span className="w-40 text-sm text-gray-700 truncate" title={pattern.pattern}>
              {formatPatternName(pattern.pattern)}
            </span>

            {/* Progress bar */}
            <div className="flex-1 h-2 bg-gray-100 rounded-full overflow-hidden">
              <div
                className="h-full bg-blue-500 rounded-full transition-all duration-300"
                style={{
                  width: `${(pattern.matchCount / maxCount) * 100}%`,
                }}
              />
            </div>

            {/* Count */}
            <span className="w-16 text-sm text-gray-500 text-right">
              {pattern.matchCount}
            </span>

            {/* Percentage */}
            <span className="w-12 text-xs text-gray-400 text-right">
              {pattern.percentage.toFixed(1)}%
            </span>
          </div>
        ))}
      </div>

      {sortedPatterns.length > 5 && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="mt-2 text-sm text-blue-600 hover:text-blue-800"
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
```

---

### 6.6 Create ValidationStats Component

**File**: `client/src/components/indexing/ValidationStats.tsx` (NEW)

```tsx
import React from 'react';
import { ValidationSummary } from '../../types/indexing';

interface Props {
  validation: ValidationSummary;
}

export const ValidationStats: React.FC<Props> = ({ validation }) => {
  const rejected = validation.totalExtracted - validation.passed;
  const passRate = validation.passRate;

  // Color based on pass rate
  const getPassRateColor = (rate: number) => {
    if (rate >= 90) return 'text-green-600';
    if (rate >= 70) return 'text-yellow-600';
    return 'text-red-600';
  };

  return (
    <div className="bg-gray-50 rounded-lg p-4">
      <h4 className="text-sm font-medium text-gray-700 mb-3">
        Code Validation
      </h4>

      <div className="flex items-center gap-6 mb-4">
        {/* Pass rate gauge */}
        <div className="relative w-24 h-24">
          <svg className="w-full h-full" viewBox="0 0 100 100">
            {/* Background circle */}
            <circle
              cx="50"
              cy="50"
              r="40"
              fill="none"
              stroke="#e5e7eb"
              strokeWidth="8"
            />
            {/* Progress circle */}
            <circle
              cx="50"
              cy="50"
              r="40"
              fill="none"
              stroke={passRate >= 90 ? '#22c55e' : passRate >= 70 ? '#eab308' : '#ef4444'}
              strokeWidth="8"
              strokeLinecap="round"
              strokeDasharray={`${passRate * 2.51} 251`}
              transform="rotate(-90 50 50)"
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
            <div className="text-2xl font-semibold text-gray-900">
              {validation.passed}
            </div>
            <div className="text-sm text-gray-500">Passed validation</div>
          </div>
          <div>
            <div className="text-2xl font-semibold text-gray-400">
              {rejected}
            </div>
            <div className="text-sm text-gray-500">Rejected</div>
          </div>
        </div>
      </div>

      {/* Rejection reasons */}
      {validation.rejectionReasons.length > 0 && (
        <div className="border-t border-gray-200 pt-3">
          <h5 className="text-xs font-medium text-gray-500 mb-2">
            REJECTION REASONS
          </h5>
          <div className="flex flex-wrap gap-2">
            {validation.rejectionReasons.map((reason) => (
              <span
                key={reason.reason}
                className="inline-flex items-center px-2 py-1 rounded-full text-xs bg-gray-200 text-gray-700"
              >
                {formatRejectionReason(reason.reason)}: {reason.count}
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
    prose_content: 'Prose content',
    duplicate: 'Duplicate',
    invalid_structure: 'Invalid structure',
  };
  return labels[reason] || reason;
}
```

---

### 6.7 Create CodeBlockBrowser Component

**File**: `client/src/components/indexing/CodeBlockBrowser.tsx` (NEW)

```tsx
import React, { useState, useEffect } from 'react';
import { listCodeBlocks, getLanguages } from '../../api/indexingClient';
import { CodeBlock, LanguageInfo } from '../../types/indexing';
import { CodeBlockViewer } from './CodeBlockViewer';
import Prism from 'prismjs';
import 'prismjs/themes/prism-tomorrow.css';

interface Props {
  jobId: string;
}

export const CodeBlockBrowser: React.FC<Props> = ({ jobId }) => {
  const [blocks, setBlocks] = useState<CodeBlock[]>([]);
  const [languages, setLanguages] = useState<LanguageInfo[]>([]);
  const [selectedLanguage, setSelectedLanguage] = useState<string>('');
  const [selectedBlock, setSelectedBlock] = useState<CodeBlock | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(true);

  const LIMIT = 20;

  // Load languages on mount
  useEffect(() => {
    getLanguages().then(setLanguages);
  }, []);

  // Load blocks when filter changes
  useEffect(() => {
    setIsLoading(true);
    setOffset(0);

    listCodeBlocks(jobId, {
      language: selectedLanguage || undefined,
      limit: LIMIT,
    })
      .then((data) => {
        setBlocks(data);
        setHasMore(data.length === LIMIT);
      })
      .finally(() => setIsLoading(false));
  }, [jobId, selectedLanguage]);

  const loadMore = async () => {
    const newOffset = offset + LIMIT;
    const moreBlocks = await listCodeBlocks(jobId, {
      language: selectedLanguage || undefined,
      offset: newOffset,
      limit: LIMIT,
    });
    setBlocks([...blocks, ...moreBlocks]);
    setOffset(newOffset);
    setHasMore(moreBlocks.length === LIMIT);
  };

  return (
    <div className="bg-white rounded-lg shadow">
      {/* Header */}
      <div className="px-6 py-4 border-b border-gray-200 flex items-center justify-between">
        <h3 className="text-lg font-medium text-gray-900">Code Blocks</h3>

        {/* Language filter */}
        <select
          value={selectedLanguage}
          onChange={(e) => setSelectedLanguage(e.target.value)}
          className="block w-48 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 text-sm"
        >
          <option value="">All languages</option>
          {languages.map((lang) => (
            <option key={lang.name} value={lang.name}>
              {lang.name}
            </option>
          ))}
        </select>
      </div>

      {/* Code block list */}
      <div className="divide-y divide-gray-200 max-h-96 overflow-y-auto">
        {isLoading ? (
          <div className="p-6 text-center text-gray-500">Loading...</div>
        ) : blocks.length === 0 ? (
          <div className="p-6 text-center text-gray-500">
            No code blocks found
          </div>
        ) : (
          blocks.map((block) => (
            <div
              key={block.id}
              className="p-4 hover:bg-gray-50 cursor-pointer"
              onClick={() => setSelectedBlock(block)}
            >
              <div className="flex items-center justify-between mb-2">
                <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-blue-100 text-blue-800">
                  {block.language}
                </span>
                <span className="text-xs text-gray-500">
                  {block.lineCount} lines
                </span>
              </div>
              <pre className="text-xs text-gray-600 overflow-hidden max-h-16 font-mono">
                {block.content.slice(0, 200)}
                {block.content.length > 200 && '...'}
              </pre>
              <div className="mt-1 text-xs text-gray-400 truncate">
                {block.sourceUrl}
              </div>
            </div>
          ))
        )}
      </div>

      {/* Load more */}
      {hasMore && !isLoading && (
        <div className="px-6 py-4 border-t border-gray-200">
          <button
            onClick={loadMore}
            className="w-full py-2 text-sm text-blue-600 hover:text-blue-800"
          >
            Load more
          </button>
        </div>
      )}

      {/* Code viewer modal */}
      {selectedBlock && (
        <CodeBlockViewer
          block={selectedBlock}
          onClose={() => setSelectedBlock(null)}
        />
      )}
    </div>
  );
};
```

---

### 6.8 Create CodeBlockViewer Component

**File**: `client/src/components/indexing/CodeBlockViewer.tsx` (NEW)

```tsx
import React, { useEffect, useRef } from 'react';
import { CodeBlock } from '../../types/indexing';
import Prism from 'prismjs';
import 'prismjs/components/prism-rust';
import 'prismjs/components/prism-python';
import 'prismjs/components/prism-javascript';
import 'prismjs/components/prism-typescript';
import 'prismjs/components/prism-go';
import 'prismjs/components/prism-java';
import 'prismjs/components/prism-c';
import 'prismjs/components/prism-cpp';
import 'prismjs/components/prism-csharp';
import 'prismjs/components/prism-ruby';
import 'prismjs/components/prism-php';

interface Props {
  block: CodeBlock;
  onClose: () => void;
}

export const CodeBlockViewer: React.FC<Props> = ({ block, onClose }) => {
  const codeRef = useRef<HTMLElement>(null);

  useEffect(() => {
    if (codeRef.current) {
      Prism.highlightElement(codeRef.current);
    }
  }, [block.content]);

  // Map language names to Prism language identifiers
  const getPrismLanguage = (lang: string): string => {
    const mapping: Record<string, string> = {
      'C++': 'cpp',
      'C#': 'csharp',
      JavaScript: 'javascript',
      TypeScript: 'typescript',
      Python: 'python',
      Rust: 'rust',
      Go: 'go',
      Java: 'java',
      C: 'c',
      Ruby: 'ruby',
      PHP: 'php',
    };
    return mapping[lang] || lang.toLowerCase();
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg shadow-xl w-4/5 max-w-4xl max-h-[90vh] flex flex-col">
        {/* Header */}
        <div className="px-6 py-4 border-b border-gray-200 flex items-center justify-between">
          <div>
            <h3 className="text-lg font-medium text-gray-900">
              Code Block
            </h3>
            <p className="text-sm text-gray-500 mt-1">
              {block.language} • {block.lineCount} lines • {block.pattern}
            </p>
          </div>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600"
          >
            <span className="text-2xl">&times;</span>
          </button>
        </div>

        {/* Code content */}
        <div className="flex-1 overflow-auto p-6 bg-gray-900">
          <pre className="text-sm">
            <code
              ref={codeRef}
              className={`language-${getPrismLanguage(block.language)}`}
            >
              {block.content}
            </code>
          </pre>
        </div>

        {/* Footer */}
        <div className="px-6 py-4 border-t border-gray-200 flex items-center justify-between">
          <div className="text-sm text-gray-500">
            Source:{' '}
            <a
              href={block.sourceUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="text-blue-600 hover:text-blue-800"
            >
              {block.sourceUrl}
            </a>
          </div>
          <div className="flex items-center gap-2">
            {block.validated ? (
              <span className="inline-flex items-center px-2 py-1 rounded-full text-xs bg-green-100 text-green-800">
                ✓ Validated
              </span>
            ) : (
              <span className="inline-flex items-center px-2 py-1 rounded-full text-xs bg-yellow-100 text-yellow-800">
                Unvalidated
              </span>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
```

---

### 6.9 Update IndexingDashboard

**File**: `client/src/pages/IndexingDashboard.tsx` (MODIFY)

Integrate the new components:

```tsx
import React, { useState, useEffect } from 'react';
import { getCodeExtractionStats } from '../api/indexingClient';
import { CodeExtractionPanel } from '../components/indexing/CodeExtractionPanel';
import { CodeBlockBrowser } from '../components/indexing/CodeBlockBrowser';
import { CodeExtractionStats } from '../types/indexing';
// ... other imports

export const IndexingDashboard: React.FC = () => {
  const [selectedJob, setSelectedJob] = useState<Job | null>(null);
  const [codeStats, setCodeStats] = useState<CodeExtractionStats | null>(null);
  const [showCodeBrowser, setShowCodeBrowser] = useState(false);

  // Load code stats when job selected
  useEffect(() => {
    if (selectedJob?.status === 'completed') {
      getCodeExtractionStats(selectedJob.id).then(setCodeStats);
    } else {
      setCodeStats(null);
    }
  }, [selectedJob]);

  return (
    <div className="container mx-auto px-4 py-8">
      {/* Existing job list and details */}
      {/* ... */}

      {/* Code Extraction Panel - show for completed jobs */}
      {selectedJob?.status === 'completed' && codeStats && (
        <div className="mt-6">
          <CodeExtractionPanel stats={codeStats} />

          {/* Toggle code browser */}
          <button
            onClick={() => setShowCodeBrowser(!showCodeBrowser)}
            className="mt-4 text-sm text-blue-600 hover:text-blue-800"
          >
            {showCodeBrowser ? 'Hide' : 'Browse'} extracted code blocks
          </button>

          {showCodeBrowser && (
            <div className="mt-4">
              <CodeBlockBrowser jobId={selectedJob.id} />
            </div>
          )}
        </div>
      )}
    </div>
  );
};
```

---

### 6.10 Update SSE Hook

**File**: `client/src/hooks/useIndexingSSE.ts` (MODIFY)

Handle new event types:

```typescript
import { useState, useEffect, useCallback } from 'react';
import { IndexingEvent, CodeExtractionEvent } from '../types/indexing';

interface UseIndexingSSEOptions {
  jobId?: string;
  onCodeExtraction?: (event: CodeExtractionEvent) => void;
}

export function useIndexingSSE(options: UseIndexingSSEOptions = {}) {
  const [events, setEvents] = useState<IndexingEvent[]>([]);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    const url = options.jobId
      ? `/api/indexing/events?job_id=${options.jobId}`
      : '/api/indexing/events';

    const eventSource = new EventSource(url);

    eventSource.onopen = () => setConnected(true);
    eventSource.onerror = () => setConnected(false);

    // Handle code extraction events specifically
    eventSource.addEventListener('code_extraction', (e) => {
      const event = JSON.parse(e.data) as CodeExtractionEvent;
      setEvents((prev) => [...prev, event]);
      options.onCodeExtraction?.(event);
    });

    // Handle other event types
    const eventTypes = [
      'job_started',
      'discovery',
      'page_crawled',
      'progress',
      'chunk_created',
      'embedding_generated',
      'page_stored',
      'job_completed',
      'job_failed',
    ];

    eventTypes.forEach((type) => {
      eventSource.addEventListener(type, (e) => {
        const event = JSON.parse(e.data) as IndexingEvent;
        setEvents((prev) => [...prev, event]);
      });
    });

    return () => {
      eventSource.close();
    };
  }, [options.jobId]);

  const clearEvents = useCallback(() => {
    setEvents([]);
  }, []);

  return { events, connected, clearEvents };
}
```

---

## Deliverables

| Deliverable | File | Description |
|-------------|------|-------------|
| Types | `types/indexing.ts` | TypeScript interfaces |
| API client | `api/indexingClient.ts` | API functions |
| CodeExtractionPanel | `components/indexing/CodeExtractionPanel.tsx` | Main panel |
| LanguageBreakdown | `components/indexing/LanguageBreakdown.tsx` | Language chart |
| PatternMatches | `components/indexing/PatternMatches.tsx` | Pattern list |
| ValidationStats | `components/indexing/ValidationStats.tsx` | Validation display |
| CodeBlockBrowser | `components/indexing/CodeBlockBrowser.tsx` | Code browser |
| CodeBlockViewer | `components/indexing/CodeBlockViewer.tsx` | Code modal |
| Dashboard update | `pages/IndexingDashboard.tsx` | Integration |
| SSE hook | `hooks/useIndexingSSE.ts` | Event handling |

---

## Exit Criteria

### Build & Lint
- [ ] `npm run build` passes without errors
- [ ] `npm run lint` passes
- [ ] `npx tsc --noEmit` passes (type checking)

### Functional Requirements
- [ ] Code extraction panel displays stats
- [ ] Language breakdown shows colored bars
- [ ] Pattern matches are expandable
- [ ] Validation gauge renders correctly
- [ ] Code block browser loads and filters
- [ ] Code viewer shows syntax highlighting
- [ ] SSE events update UI in real-time

### UX/Design Quality (Verified by Agents)
- [ ] Components reviewed by `ux-designer` agent
- [ ] Accessibility audit passed by `accessibility-expert` agent
- [ ] Responsive on mobile devices (320px, 768px, 1024px, 1440px)
- [ ] Dark mode support verified (if applicable)
- [ ] No generic AI aesthetics - distinctive, polished styling
- [ ] Keyboard navigation works for all interactive elements
- [ ] ARIA labels present on all interactive elements

---

## Testing Commands

```bash
# Development server
cd client && npm run dev

# Build
cd client && npm run build

# Lint
cd client && npm run lint

# Type check
cd client && npx tsc --noEmit
```

---

## Next Phase

Upon completion, proceed to [Phase 7: Testing & Documentation](./phase-7-testing-docs.md).
