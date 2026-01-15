import { useState, useMemo, useEffect, useCallback } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import {
  ChevronRight,
  ChevronDown,
  Grid,
  List,
  Globe,
  Clock,
  FileText,
  Code,
  Hash,
  Database,
  Search,
  X,
  Sparkles,
  Zap,
  Layers,
  Box,
  FileType,
  RefreshCw,
  Tag,
  FolderTree,
} from 'lucide-react';
import clsx from 'clsx';
import { api } from '../api/client';
import { useEntriesFilters, type ViewMode, type SearchMode, type EntryType, type GroupByDimension } from '../hooks/useEntriesFilters';
import { useEntryGroups } from '../hooks/useEntryGroups';
import { GroupedEntryView } from '../components/GroupedEntryView';
import type { Entry, SearchResult } from '../types';

// Helper to format dates nicely
function formatDate(dateStr?: string): string {
  if (!dateStr) return '';
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return 'Today';
  if (diffDays === 1) return 'Yesterday';
  if (diffDays < 7) return `${diffDays} days ago`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)} weeks ago`;
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
}

// Helper to get entry type icon and color
function getEntryTypeStyle(type: string) {
  const styles: Record<string, { bg: string; text: string; border: string; icon: React.ReactNode }> = {
    document: { bg: 'bg-blue-500/15', text: 'text-blue-400', border: 'border-blue-500/30', icon: <FileText className="w-3.5 h-3.5" /> },
    pdf: { bg: 'bg-rose-500/15', text: 'text-rose-400', border: 'border-rose-500/30', icon: <FileType className="w-3.5 h-3.5" /> },
    article: { bg: 'bg-emerald-500/15', text: 'text-emerald-400', border: 'border-emerald-500/30', icon: <FileText className="w-3.5 h-3.5" /> },
    code: { bg: 'bg-amber-500/15', text: 'text-amber-400', border: 'border-amber-500/30', icon: <Code className="w-3.5 h-3.5" /> },
    other: { bg: 'bg-slate-500/15', text: 'text-slate-400', border: 'border-slate-500/30', icon: <FileText className="w-3.5 h-3.5" /> },
  };
  return styles[type.toLowerCase()] || styles.other;
}

// Helper to get source type style
function getSourceTypeStyle(type?: string) {
  if (!type) return { bg: 'bg-slate-700/50', text: 'text-slate-400' };
  const styles: Record<string, { bg: string; text: string }> = {
    html: { bg: 'bg-orange-500/10', text: 'text-orange-300' },
    pdf: { bg: 'bg-rose-500/10', text: 'text-rose-300' },
    docx: { bg: 'bg-blue-500/10', text: 'text-blue-300' },
    markdown: { bg: 'bg-purple-500/10', text: 'text-purple-300' },
    url: { bg: 'bg-cyan-500/10', text: 'text-cyan-300' },
    source_code: { bg: 'bg-green-500/10', text: 'text-green-300' },
  };
  return styles[type.toLowerCase()] || { bg: 'bg-slate-700/50', text: 'text-slate-400' };
}

// Type pill configuration
const TYPE_PILLS: { value: EntryType; label: string; icon: React.ReactNode; colors: { bg: string; text: string; border: string } }[] = [
  { value: 'all', label: 'All', icon: <Box className="w-3.5 h-3.5" />, colors: { bg: 'bg-cyan-500/15', text: 'text-cyan-400', border: 'border-cyan-500/30' } },
  { value: 'document', label: 'Document', icon: <FileText className="w-3.5 h-3.5" />, colors: { bg: 'bg-blue-500/15', text: 'text-blue-400', border: 'border-blue-500/30' } },
  { value: 'article', label: 'Article', icon: <FileText className="w-3.5 h-3.5" />, colors: { bg: 'bg-emerald-500/15', text: 'text-emerald-400', border: 'border-emerald-500/30' } },
  { value: 'pdf', label: 'PDF', icon: <FileType className="w-3.5 h-3.5" />, colors: { bg: 'bg-rose-500/15', text: 'text-rose-400', border: 'border-rose-500/30' } },
  { value: 'code', label: 'Code', icon: <Code className="w-3.5 h-3.5" />, colors: { bg: 'bg-amber-500/15', text: 'text-amber-400', border: 'border-amber-500/30' } },
];

// Group By pill configuration
const GROUP_BY_PILLS: { value: GroupByDimension; label: string; icon: React.ReactNode; colors: { bg: string; text: string; border: string } }[] = [
  { value: 'source_domain', label: 'Source', icon: <Globe className="w-3.5 h-3.5" />, colors: { bg: 'bg-teal-500/15', text: 'text-teal-400', border: 'border-teal-500/30' } },
  { value: 'entry_type', label: 'Type', icon: <FileText className="w-3.5 h-3.5" />, colors: { bg: 'bg-blue-500/15', text: 'text-blue-400', border: 'border-blue-500/30' } },
  { value: 'source_type', label: 'Format', icon: <FileType className="w-3.5 h-3.5" />, colors: { bg: 'bg-purple-500/15', text: 'text-purple-400', border: 'border-purple-500/30' } },
  { value: 'tag', label: 'Tag', icon: <Tag className="w-3.5 h-3.5" />, colors: { bg: 'bg-amber-500/15', text: 'text-amber-400', border: 'border-amber-500/30' } },
  { value: 'none', label: 'None', icon: <List className="w-3.5 h-3.5" />, colors: { bg: 'bg-slate-500/15', text: 'text-slate-400', border: 'border-slate-500/30' } },
];

// Domain badge component
function DomainBadge({ domain }: { domain?: string }) {
  if (!domain) return null;
  return (
    <div className="flex items-center gap-1.5 px-2 py-1 rounded-md bg-slate-800/80 border border-slate-700/50">
      <Globe className="w-3 h-3 text-cyan-400" />
      <span className="text-xs font-mono text-slate-300 truncate max-w-[140px]">{domain}</span>
    </div>
  );
}

// Entry card component - Grid view
function EntryCardGrid({ entry }: { entry: Entry }) {
  const typeStyle = getEntryTypeStyle(entry.entry_type);
  const sourceStyle = getSourceTypeStyle(entry.source_type);

  return (
    <Link
      to={`/entries/${encodeURIComponent(entry.id)}`}
      className="group relative flex flex-col bg-slate-900/60 backdrop-blur-sm rounded-xl border border-slate-800/80 overflow-hidden hover:border-cyan-500/40 hover:shadow-lg hover:shadow-cyan-500/5 transition-all duration-300"
    >
      {/* Top accent bar */}
      <div className={clsx('h-1 w-full', typeStyle.bg.replace('/15', '/40'))} />

      <div className="p-5 flex flex-col flex-1">
        {/* Header with type badge */}
        <div className="flex items-start justify-between gap-3 mb-3">
          <div className={clsx('flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium', typeStyle.bg, typeStyle.text)}>
            {typeStyle.icon}
            <span className="capitalize">{entry.entry_type}</span>
          </div>
          {entry.source_type && (
            <span className={clsx('px-2 py-0.5 text-[10px] font-mono rounded uppercase tracking-wider', sourceStyle.bg, sourceStyle.text)}>
              {entry.source_type}
            </span>
          )}
        </div>

        {/* Title */}
        <h3 className="font-semibold text-white text-lg leading-tight group-hover:text-cyan-400 transition-colors line-clamp-2 mb-2">
          {entry.title}
        </h3>

        {/* Description */}
        <p className="text-sm text-slate-400 line-clamp-2 flex-1 mb-4">{entry.description}</p>

        {/* Source domain */}
        {entry.source_domain && <DomainBadge domain={entry.source_domain} />}

        {/* Tags */}
        {entry.tags && entry.tags.length > 0 && (
          <div className="flex flex-wrap gap-1.5 mt-3">
            {entry.tags.slice(0, 3).map((tag: string) => (
              <span key={tag} className="px-2 py-0.5 text-xs bg-slate-800/80 text-slate-400 rounded-md border border-slate-700/50">
                {tag}
              </span>
            ))}
            {entry.tags.length > 3 && (
              <span className="px-2 py-0.5 text-xs bg-slate-800/50 text-slate-500 rounded-md">+{entry.tags.length - 3}</span>
            )}
          </div>
        )}

        {/* Footer with timestamp */}
        {entry.updated_at && (
          <div className="flex items-center gap-1.5 mt-4 pt-3 border-t border-slate-800/50">
            <Clock className="w-3 h-3 text-slate-500" />
            <span className="text-xs text-slate-500">{formatDate(entry.updated_at)}</span>
          </div>
        )}
      </div>

      {/* Hover indicator */}
      <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-gradient-to-r from-cyan-500 to-teal-500 scale-x-0 group-hover:scale-x-100 transition-transform duration-300" />
    </Link>
  );
}

// Entry card component - List view
function EntryCardList({ entry }: { entry: Entry }) {
  const typeStyle = getEntryTypeStyle(entry.entry_type);
  const sourceStyle = getSourceTypeStyle(entry.source_type);

  return (
    <Link
      to={`/entries/${encodeURIComponent(entry.id)}`}
      className="group flex items-center gap-4 p-4 bg-slate-900/60 backdrop-blur-sm rounded-xl border border-slate-800/80 hover:border-cyan-500/40 hover:shadow-lg hover:shadow-cyan-500/5 transition-all duration-300"
    >
      {/* Type indicator */}
      <div className={clsx('flex-shrink-0 w-10 h-10 rounded-lg flex items-center justify-center', typeStyle.bg)}>
        <div className={typeStyle.text}>{typeStyle.icon}</div>
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-3 mb-1">
          <h3 className="font-semibold text-white group-hover:text-cyan-400 transition-colors truncate">{entry.title}</h3>
          <div className={clsx('flex-shrink-0 px-2 py-0.5 rounded text-xs font-medium', typeStyle.bg, typeStyle.text)}>
            {entry.entry_type}
          </div>
        </div>
        <p className="text-sm text-slate-400 line-clamp-1">{entry.description}</p>
        <div className="flex items-center gap-3 mt-2">
          {entry.source_domain && (
            <div className="flex items-center gap-1">
              <Globe className="w-3 h-3 text-slate-500" />
              <span className="text-xs text-slate-500 font-mono truncate max-w-[120px]">{entry.source_domain}</span>
            </div>
          )}
          {entry.source_type && (
            <span className={clsx('px-1.5 py-0.5 text-[10px] font-mono rounded uppercase', sourceStyle.bg, sourceStyle.text)}>
              {entry.source_type}
            </span>
          )}
          {entry.updated_at && (
            <div className="flex items-center gap-1">
              <Clock className="w-3 h-3 text-slate-500" />
              <span className="text-xs text-slate-500">{formatDate(entry.updated_at)}</span>
            </div>
          )}
        </div>
      </div>

      {/* Tags */}
      <div className="flex-shrink-0 flex items-center gap-2 max-w-[200px]">
        {entry.tags?.slice(0, 2).map((tag: string) => (
          <span key={tag} className="px-2 py-0.5 text-xs bg-slate-800 text-slate-400 rounded truncate max-w-[80px]">
            {tag}
          </span>
        ))}
      </div>

      <ChevronRight className="w-5 h-5 text-slate-600 group-hover:text-cyan-400 flex-shrink-0 transition-colors" />
    </Link>
  );
}

// Entry card component - Compact view
function EntryCardCompact({ entry }: { entry: Entry }) {
  const typeStyle = getEntryTypeStyle(entry.entry_type);

  return (
    <Link
      to={`/entries/${encodeURIComponent(entry.id)}`}
      className="group flex items-center gap-3 px-3 py-2.5 bg-slate-900/40 rounded-lg border border-slate-800/60 hover:border-cyan-500/30 hover:bg-slate-900/60 transition-all"
    >
      <div className={clsx('w-2 h-2 rounded-full flex-shrink-0', typeStyle.bg.replace('/15', '/60'))} />
      <span className="font-medium text-slate-200 group-hover:text-cyan-400 transition-colors truncate flex-1">{entry.title}</span>
      {entry.source_domain && <span className="text-[10px] text-slate-500 font-mono truncate max-w-[100px]">{entry.source_domain}</span>}
      <span className={clsx('text-[10px] px-1.5 py-0.5 rounded', typeStyle.bg, typeStyle.text)}>{entry.entry_type}</span>
    </Link>
  );
}

// Unified entry card component
function EntryCard({ entry, viewMode }: { entry: Entry; viewMode: ViewMode }) {
  switch (viewMode) {
    case 'list':
      return <EntryCardList entry={entry} />;
    case 'compact':
      return <EntryCardCompact entry={entry} />;
    default:
      return <EntryCardGrid entry={entry} />;
  }
}

// Search result card component
function SearchResultCard({ result }: { result: SearchResult }) {
  const typeStyle = getEntryTypeStyle(result.entry_type);
  const scorePercent = Math.round(result.score * 100);
  const scoreColor = scorePercent >= 80 ? 'text-emerald-400' : scorePercent >= 60 ? 'text-cyan-400' : 'text-amber-400';

  return (
    <Link
      to={`/entries/${encodeURIComponent(result.entry_id)}`}
      className="group relative flex flex-col bg-slate-900/60 backdrop-blur-sm rounded-xl border border-slate-800/80 overflow-hidden hover:border-cyan-500/40 hover:shadow-lg hover:shadow-cyan-500/5 transition-all duration-300"
    >
      {/* Top accent bar */}
      <div className={clsx('h-1 w-full', typeStyle.bg.replace('/15', '/40'))} />

      <div className="p-5 flex flex-col flex-1">
        {/* Header with type badge and score */}
        <div className="flex items-start justify-between gap-3 mb-3">
          <div className={clsx('flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium', typeStyle.bg, typeStyle.text)}>
            {typeStyle.icon}
            <span className="capitalize">{result.entry_type}</span>
          </div>
          <div className="flex items-center gap-2">
            {result.chunk_type && (
              <span className="px-2 py-0.5 text-[10px] font-mono rounded bg-slate-800/80 text-slate-400 uppercase tracking-wider">
                {result.chunk_type}
              </span>
            )}
            <div className={clsx('flex items-center gap-1 px-2 py-0.5 rounded-md bg-slate-800/80', scoreColor)}>
              <Sparkles className="w-3 h-3" />
              <span className="text-xs font-bold">{scorePercent}%</span>
            </div>
          </div>
        </div>

        {/* Title */}
        <h3 className="font-semibold text-white text-lg leading-tight group-hover:text-cyan-400 transition-colors line-clamp-2 mb-2">
          {result.entry_title}
        </h3>

        {/* Matched text snippet */}
        <p className="text-sm text-slate-400 line-clamp-3 flex-1 mb-4 italic border-l-2 border-cyan-500/30 pl-3">
          "{result.text}"
        </p>

        {/* Source domain */}
        {result.source_domain && <DomainBadge domain={result.source_domain} />}

        {/* Tags */}
        {result.tags && result.tags.length > 0 && (
          <div className="flex flex-wrap gap-1.5 mt-3">
            {result.tags.slice(0, 3).map((tag: string) => (
              <span key={tag} className="px-2 py-0.5 text-xs bg-slate-800/80 text-slate-400 rounded-md border border-slate-700/50">
                {tag}
              </span>
            ))}
            {result.tags.length > 3 && (
              <span className="px-2 py-0.5 text-xs bg-slate-800/50 text-slate-500 rounded-md">+{result.tags.length - 3}</span>
            )}
          </div>
        )}
      </div>

      {/* Hover indicator */}
      <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-gradient-to-r from-cyan-500 to-teal-500 scale-x-0 group-hover:scale-x-100 transition-transform duration-300" />
    </Link>
  );
}

// Search mode toggle component
function SearchModeToggle({ mode, onChange }: { mode: SearchMode; onChange: (mode: SearchMode) => void }) {
  const modes: { value: SearchMode; label: string; icon: React.ReactNode }[] = [
    { value: 'fts', label: 'FTS', icon: <Search className="w-3.5 h-3.5" /> },
    { value: 'vector', label: 'Vector', icon: <Sparkles className="w-3.5 h-3.5" /> },
    { value: 'hybrid', label: 'Hybrid', icon: <Layers className="w-3.5 h-3.5" /> },
  ];

  return (
    <div className="flex items-center bg-slate-800/80 rounded-lg p-1 border border-slate-700/50">
      {modes.map((m) => (
        <button
          key={m.value}
          onClick={() => onChange(m.value)}
          className={clsx(
            'flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium transition-all',
            mode === m.value
              ? 'bg-cyan-500/20 text-cyan-400'
              : 'text-slate-500 hover:text-white'
          )}
        >
          {m.icon}
          <span className="hidden sm:inline">{m.label}</span>
        </button>
      ))}
    </div>
  );
}

// Type pill component
function TypePill({
  pill,
  isActive,
  count,
  onClick,
}: {
  pill: typeof TYPE_PILLS[0];
  isActive: boolean;
  count: number;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={clsx(
        'flex items-center gap-2 px-4 py-2 rounded-full text-sm font-medium transition-all whitespace-nowrap border',
        isActive
          ? `${pill.colors.bg} ${pill.colors.text} ${pill.colors.border}`
          : 'bg-slate-800/60 text-slate-400 border-slate-700/50 hover:text-white hover:bg-slate-800'
      )}
    >
      {pill.icon}
      <span>{pill.label}</span>
      <span className={clsx('text-xs', isActive ? 'opacity-80' : 'text-slate-500')}>({count})</span>
    </button>
  );
}

// Group By pill component
function GroupByPill({
  pill,
  isActive,
  onClick,
}: {
  pill: typeof GROUP_BY_PILLS[0];
  isActive: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={clsx(
        'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all whitespace-nowrap border',
        isActive
          ? `${pill.colors.bg} ${pill.colors.text} ${pill.colors.border}`
          : 'bg-slate-800/40 text-slate-500 border-slate-700/30 hover:text-slate-300 hover:bg-slate-800/60 hover:border-slate-700/50'
      )}
    >
      {pill.icon}
      <span>{pill.label}</span>
    </button>
  );
}

// View mode toggle component
function ViewModeToggle({ mode, onChange }: { mode: ViewMode; onChange: (mode: ViewMode) => void }) {
  return (
    <div className="flex items-center bg-slate-800/80 rounded-lg p-1 border border-slate-700/50">
      <button
        onClick={() => onChange('grid')}
        className={clsx('p-1.5 rounded transition-all', mode === 'grid' ? 'bg-cyan-500/20 text-cyan-400' : 'text-slate-500 hover:text-white')}
        title="Grid view"
      >
        <Grid className="w-4 h-4" />
      </button>
      <button
        onClick={() => onChange('list')}
        className={clsx('p-1.5 rounded transition-all', mode === 'list' ? 'bg-cyan-500/20 text-cyan-400' : 'text-slate-500 hover:text-white')}
        title="List view"
      >
        <List className="w-4 h-4" />
      </button>
      <button
        onClick={() => onChange('compact')}
        className={clsx('p-1.5 rounded transition-all', mode === 'compact' ? 'bg-cyan-500/20 text-cyan-400' : 'text-slate-500 hover:text-white')}
        title="Compact view"
      >
        <Hash className="w-4 h-4" />
      </button>
    </div>
  );
}

// Stats bar component
function StatsBar({ total, filtered, domains }: { total: number; filtered: number; domains: number }) {
  return (
    <div className="flex items-center gap-6 text-sm">
      <div className="flex items-center gap-2">
        <Database className="w-4 h-4 text-cyan-500" />
        <span className="text-slate-400">
          <span className="text-white font-semibold">{filtered}</span>
          {filtered !== total && <span className="text-slate-500"> of {total}</span>} entries
        </span>
      </div>
      <div className="flex items-center gap-2">
        <Globe className="w-4 h-4 text-teal-500" />
        <span className="text-slate-400">
          <span className="text-white font-semibold">{domains}</span> sources
        </span>
      </div>
    </div>
  );
}

export default function EntryBrowser() {
  const { filters, setFilter, clearFilters, hasActiveFilters, activeFilterCount } = useEntriesFilters();
  const [searchInput, setSearchInput] = useState(filters.q);
  const [filtersExpanded, setFiltersExpanded] = useState(false);
  const [isSearchMode, setIsSearchMode] = useState(false);
  const [isReindexing, setIsReindexing] = useState(false);
  const [reindexResult, setReindexResult] = useState<{ success: boolean; message: string } | null>(null);
  const queryClient = useQueryClient();

  // Handle bulk re-index by domain
  const handleReindexDomain = async () => {
    if (!filters.domain) return;
    setIsReindexing(true);
    setReindexResult(null);
    try {
      const result = await api.reindexByDomain(filters.domain, { skip_render: false });
      setReindexResult({
        success: true,
        message: `Queued ${result.entries_queued} entries for re-indexing`,
      });
      // Invalidate queries to refresh the list
      queryClient.invalidateQueries({ queryKey: ['entries'] });
      queryClient.invalidateQueries({ queryKey: ['entries-all'] });
    } catch (err) {
      setReindexResult({
        success: false,
        message: err instanceof Error ? err.message : 'Re-index failed',
      });
    } finally {
      setIsReindexing(false);
    }
  };

  // Sync search input with URL params
  useEffect(() => {
    setSearchInput(filters.q);
    // Enable search mode if there's a query
    setIsSearchMode(!!filters.q);
  }, [filters.q]);

  // Fetch all entries to get counts
  const { data: allEntriesData } = useQuery({
    queryKey: ['entries-all'],
    queryFn: () => api.getEntries({}),
  });

  // Fetch filtered entries (browse mode)
  const { data: entriesData, isLoading: entriesLoading } = useQuery({
    queryKey: ['entries', filters.type, filters.chunk_type],
    queryFn: () =>
      api.getEntries({
        // When chunk_type is set, use it; otherwise fall back to entry_type
        entry_type: filters.chunk_type ? undefined : (filters.type === 'all' ? undefined : filters.type),
        chunk_type: filters.chunk_type || undefined,
      }),
    enabled: !isSearchMode,
  });

  // Semantic search query (search mode)
  const { data: searchData, isLoading: searchLoading } = useQuery({
    queryKey: ['search', filters.q, filters.type, filters.domain],
    queryFn: () =>
      api.search({
        q: filters.q,
        entry_type: filters.type === 'all' ? undefined : filters.type,
        source_domain: filters.domain || undefined,
        limit: 50,
      }),
    enabled: isSearchMode && !!filters.q,
  });

  const isLoading = isSearchMode ? searchLoading : entriesLoading;
  const allEntries = allEntriesData?.entries || [];
  const filteredEntries = entriesData?.entries || [];
  const searchResults = searchData?.results || [];

  // Calculate type counts from all entries
  const typeCounts = useMemo(() => {
    const counts: Record<string, number> = { all: allEntries.length };
    allEntries.forEach((entry) => {
      const type = entry.entry_type.toLowerCase();
      counts[type] = (counts[type] || 0) + 1;
    });
    return counts;
  }, [allEntries]);

  // Extract unique domains from filtered entries
  const domains = useMemo(() => {
    const domainSet = new Set<string>();
    filteredEntries.forEach((e) => {
      if (e.source_domain) domainSet.add(e.source_domain);
    });
    return Array.from(domainSet).sort();
  }, [filteredEntries]);

  // Apply domain filtering for browse mode
  const entries = useMemo(() => {
    let result = filteredEntries;

    // Filter by domain
    if (filters.domain) {
      result = result.filter((e) => e.source_domain === filters.domain);
    }

    return result;
  }, [filteredEntries, filters.domain]);

  // Compute groups based on selected groupBy dimension
  const entryGroups = useEntryGroups(entries, filters.groupBy);

  // Handle clicking on a group to filter to that group
  const handleGroupClick = useCallback((groupKey: string) => {
    // Set the appropriate filter based on current groupBy dimension
    switch (filters.groupBy) {
      case 'source_domain':
        setFilter('domain', groupKey);
        break;
      case 'entry_type':
        setFilter('type', groupKey.toLowerCase() as EntryType);
        break;
      case 'tag':
        setFilter('tag', groupKey);
        break;
      // source_type filtering would need backend support
      default:
        break;
    }
  }, [filters.groupBy, setFilter]);

  // Render entry callback for grouped view
  const renderEntry = useCallback((entry: Entry, viewMode: ViewMode) => {
    return <EntryCard entry={entry} viewMode={viewMode} />;
  }, []);

  // Handle search submit - triggers semantic search
  const handleSearchSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (searchInput.trim()) {
      setIsSearchMode(true);
      setFilter('q', searchInput.trim());
    }
  };

  // Handle search clear
  const handleSearchClear = () => {
    setSearchInput('');
    setFilter('q', '');
    setIsSearchMode(false);
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold text-white tracking-tight">Knowledge Base</h1>
          <div className="mt-2">
            <StatsBar total={allEntriesData?.total || 0} filtered={isSearchMode ? searchResults.length : entries.length} domains={domains.length} />
          </div>
        </div>
      </div>

      {/* Search Bar */}
      <form onSubmit={handleSearchSubmit} className="bg-slate-900/60 backdrop-blur-sm rounded-xl border border-slate-800/80 p-4">
        <div className="flex items-center gap-3">
          <div className="relative flex-1">
            <Search className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-slate-500" />
            <input
              type="text"
              value={searchInput}
              onChange={(e) => setSearchInput(e.target.value)}
              placeholder="Describe what you're looking for... (press Enter for semantic search)"
              className="input w-full pl-12 pr-10 py-3 bg-slate-800/80 border-slate-700/80 text-white placeholder-slate-500"
            />
            {searchInput && (
              <button
                type="button"
                onClick={handleSearchClear}
                className="absolute right-4 top-1/2 -translate-y-1/2 text-slate-500 hover:text-white transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            )}
          </div>
          <SearchModeToggle mode={filters.mode} onChange={(mode) => setFilter('mode', mode)} />
        </div>
      </form>

      {/* Type Pills */}
      <div className="flex items-center gap-2 overflow-x-auto pb-2 scrollbar-thin scrollbar-thumb-slate-700 scrollbar-track-transparent">
        {TYPE_PILLS.map((pill) => (
          <TypePill
            key={pill.value}
            pill={pill}
            isActive={filters.type === pill.value}
            count={typeCounts[pill.value] || 0}
            onClick={() => setFilter('type', pill.value)}
          />
        ))}
      </div>

      {/* Group By Pills */}
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-2">
          <FolderTree className="w-4 h-4 text-slate-500" />
          <span className="text-xs font-medium text-slate-500 uppercase tracking-wider">Group by</span>
        </div>
        <div className="flex items-center gap-1.5">
          {GROUP_BY_PILLS.map((pill) => (
            <GroupByPill
              key={pill.value}
              pill={pill}
              isActive={filters.groupBy === pill.value}
              onClick={() => setFilter('groupBy', pill.value)}
            />
          ))}
        </div>
      </div>

      {/* Collapsible Filter Panel */}
      <div className="bg-slate-900/40 rounded-xl border border-slate-800/60 overflow-hidden">
        <div className="flex items-center justify-between p-3">
          <button
            onClick={() => setFiltersExpanded(!filtersExpanded)}
            className={clsx(
              'flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium transition-all',
              hasActiveFilters
                ? 'bg-cyan-500/15 text-cyan-400 border border-cyan-500/30'
                : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
            )}
          >
            <Zap className="w-4 h-4" />
            <span>Filters{activeFilterCount > 0 && ` (${activeFilterCount})`}</span>
            <ChevronDown
              className={clsx('w-4 h-4 transition-transform duration-200', filtersExpanded && 'rotate-180')}
            />
          </button>

          <ViewModeToggle mode={filters.view} onChange={(mode) => setFilter('view', mode)} />
        </div>

        {/* Expanded filters */}
        <div
          className={clsx(
            'overflow-hidden transition-all duration-300 ease-in-out',
            filtersExpanded ? 'max-h-60 opacity-100' : 'max-h-0 opacity-0'
          )}
        >
          <div className="p-4 pt-0 border-t border-slate-800/50 space-y-4">
            {/* Domain filter with re-index button */}
            <div className="flex items-center gap-3">
              <label className="text-xs font-medium text-slate-500 uppercase tracking-wider w-20">Source</label>
              <select
                value={filters.domain}
                onChange={(e) => {
                  setFilter('domain', e.target.value);
                  setReindexResult(null); // Clear any previous result
                }}
                className="input flex-1 bg-slate-800/80 border-slate-700/80 text-sm"
              >
                <option value="">All Sources</option>
                {domains.map((domain) => (
                  <option key={domain} value={domain}>
                    {domain}
                  </option>
                ))}
              </select>
              {filters.domain && (
                <button
                  onClick={handleReindexDomain}
                  disabled={isReindexing}
                  className={clsx(
                    "flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-medium transition-all whitespace-nowrap",
                    isReindexing
                      ? "bg-slate-700 text-slate-400 cursor-not-allowed"
                      : "bg-cyan-600 hover:bg-cyan-500 text-white"
                  )}
                  title="Re-index all entries from this domain with JS rendering"
                >
                  <RefreshCw className={clsx("w-4 h-4", isReindexing && "animate-spin")} />
                  {isReindexing ? 'Re-indexing...' : 'Re-index Domain'}
                </button>
              )}
            </div>
            {/* Re-index result message */}
            {reindexResult && (
              <div className={clsx(
                "flex items-center gap-2 px-3 py-2 rounded-lg text-sm",
                reindexResult.success
                  ? "bg-emerald-500/15 text-emerald-400 border border-emerald-500/30"
                  : "bg-red-500/15 text-red-400 border border-red-500/30"
              )}>
                {reindexResult.success ? (
                  <RefreshCw className="w-4 h-4" />
                ) : (
                  <X className="w-4 h-4" />
                )}
                {reindexResult.message}
              </div>
            )}

            {/* Active filter chips */}
            {hasActiveFilters && (
              <div className="flex items-center gap-2 flex-wrap">
                {filters.q && (
                  <div className="flex items-center gap-2 px-3 py-1.5 bg-cyan-500/15 border border-cyan-500/30 rounded-lg">
                    <Search className="w-3.5 h-3.5 text-cyan-400" />
                    <span className="text-cyan-400 text-sm">"{filters.q}"</span>
                    <button onClick={() => setFilter('q', '')} className="text-cyan-400/60 hover:text-cyan-400 ml-1">
                      <X className="w-3.5 h-3.5" />
                    </button>
                  </div>
                )}
                {filters.type !== 'all' && (
                  <div className="flex items-center gap-2 px-3 py-1.5 bg-blue-500/15 border border-blue-500/30 rounded-lg">
                    <FileText className="w-3.5 h-3.5 text-blue-400" />
                    <span className="text-blue-400 text-sm capitalize">{filters.type}</span>
                    <button onClick={() => setFilter('type', 'all')} className="text-blue-400/60 hover:text-blue-400 ml-1">
                      <X className="w-3.5 h-3.5" />
                    </button>
                  </div>
                )}
                {filters.domain && (
                  <div className="flex items-center gap-2 px-3 py-1.5 bg-teal-500/15 border border-teal-500/30 rounded-lg">
                    <Globe className="w-3.5 h-3.5 text-teal-400" />
                    <span className="text-teal-400 text-sm font-mono">{filters.domain}</span>
                    <button onClick={() => setFilter('domain', '')} className="text-teal-400/60 hover:text-teal-400 ml-1">
                      <X className="w-3.5 h-3.5" />
                    </button>
                  </div>
                )}
                <button
                  onClick={clearFilters}
                  className="text-xs text-slate-500 hover:text-white transition-colors px-2"
                >
                  Clear all
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Results */}
      <div className="min-w-0">
        {isLoading ? (
          <div className="flex items-center justify-center py-20">
            <div className="flex flex-col items-center gap-4">
              <div className="w-10 h-10 border-3 border-cyan-500 border-t-transparent rounded-full animate-spin" />
              <p className="text-slate-500 text-sm">{isSearchMode ? 'Searching...' : 'Loading entries...'}</p>
            </div>
          </div>
        ) : isSearchMode ? (
          // Search results view
          searchResults.length === 0 ? (
            <div className="bg-slate-900/40 rounded-xl border border-slate-800/60 p-12 text-center">
              <Search className="w-12 h-12 text-slate-700 mx-auto mb-4" />
              <p className="text-slate-400 text-lg">No results found</p>
              <p className="text-slate-600 text-sm mt-1">Try a different search query or adjust your filters</p>
              <button
                onClick={handleSearchClear}
                className="mt-4 px-4 py-2 bg-cyan-500/10 text-cyan-400 rounded-lg border border-cyan-500/30 hover:bg-cyan-500/20 transition-colors text-sm"
              >
                Clear search
              </button>
            </div>
          ) : (
            <div className="space-y-4">
              <div className="flex items-center gap-2 text-sm text-slate-400">
                <Sparkles className="w-4 h-4 text-cyan-400" />
                <span>
                  Found <span className="text-white font-semibold">{searchResults.length}</span> semantic matches for "
                  <span className="text-cyan-400">{filters.q}</span>"
                </span>
              </div>
              <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
                {searchResults.map((result) => (
                  <SearchResultCard key={result.id} result={result} />
                ))}
              </div>
            </div>
          )
        ) : entries.length === 0 ? (
          <div className="bg-slate-900/40 rounded-xl border border-slate-800/60 p-12 text-center">
            <Database className="w-12 h-12 text-slate-700 mx-auto mb-4" />
            <p className="text-slate-400 text-lg">No entries found</p>
            <p className="text-slate-600 text-sm mt-1">Try adjusting your filters or add some content</p>
            {hasActiveFilters && (
              <button
                onClick={clearFilters}
                className="mt-4 px-4 py-2 bg-cyan-500/10 text-cyan-400 rounded-lg border border-cyan-500/30 hover:bg-cyan-500/20 transition-colors text-sm"
              >
                Clear all filters
              </button>
            )}
          </div>
        ) : (
          <GroupedEntryView
            groups={entryGroups}
            groupBy={filters.groupBy}
            viewMode={filters.view}
            onGroupClick={handleGroupClick}
            renderEntry={renderEntry}
          />
        )}
      </div>
    </div>
  );
}
