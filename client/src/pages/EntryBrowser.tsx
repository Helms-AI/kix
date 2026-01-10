import { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import {
  ChevronRight,
  Filter,
  Grid,
  List,
  Tag,
  Globe,
  Clock,
  FileText,
  Code,
  Hash,
  Layers,
  Database,
} from 'lucide-react';
import clsx from 'clsx';
import { api } from '../api/client';
import type { Entry, CategoryInfo } from '../types';

type ViewMode = 'grid' | 'list' | 'compact';

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
  const styles: Record<string, { bg: string; text: string; icon: React.ReactNode }> = {
    document: { bg: 'bg-blue-500/15', text: 'text-blue-400', icon: <FileText className="w-3.5 h-3.5" /> },
    pdf: { bg: 'bg-rose-500/15', text: 'text-rose-400', icon: <FileText className="w-3.5 h-3.5" /> },
    article: { bg: 'bg-emerald-500/15', text: 'text-emerald-400', icon: <FileText className="w-3.5 h-3.5" /> },
    code: { bg: 'bg-amber-500/15', text: 'text-amber-400', icon: <Code className="w-3.5 h-3.5" /> },
    messaging: { bg: 'bg-violet-500/15', text: 'text-violet-400', icon: <Layers className="w-3.5 h-3.5" /> },
    conversation: { bg: 'bg-orange-500/15', text: 'text-orange-400', icon: <Layers className="w-3.5 h-3.5" /> },
    other: { bg: 'bg-slate-500/15', text: 'text-slate-400', icon: <FileText className="w-3.5 h-3.5" /> },
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

// Category sidebar with domain filtering
function CategorySidebar({
  categories,
  selectedCategory,
  onSelectCategory,
  domains,
  selectedDomain,
  onSelectDomain,
}: {
  categories: CategoryInfo[];
  selectedCategory: string | null;
  onSelectCategory: (cat: string | null) => void;
  domains: string[];
  selectedDomain: string | null;
  onSelectDomain: (domain: string | null) => void;
}) {
  const messagingCats = categories.filter((c) => c.entry_type === 'messaging');
  const conversationCats = categories.filter((c) => c.entry_type === 'conversation');

  return (
    <div className="w-72 flex-shrink-0 hidden lg:block space-y-4">
      {/* Domains filter */}
      {domains.length > 0 && (
        <div className="bg-slate-900/60 backdrop-blur-sm rounded-xl border border-slate-800/80 p-4">
          <h3 className="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-3 flex items-center gap-2">
            <Globe className="w-3.5 h-3.5" />
            Sources
          </h3>
          <div className="space-y-1">
            <button
              onClick={() => onSelectDomain(null)}
              className={clsx(
                'w-full text-left px-3 py-2 rounded-lg text-sm transition-all flex items-center justify-between',
                selectedDomain === null ? 'bg-cyan-500/15 text-cyan-400 border border-cyan-500/30' : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
              )}
            >
              <span>All Sources</span>
              <span className="text-xs text-slate-600">{domains.length}</span>
            </button>
            {domains.slice(0, 8).map((domain) => (
              <button
                key={domain}
                onClick={() => onSelectDomain(domain)}
                className={clsx(
                  'w-full text-left px-3 py-2 rounded-lg text-sm transition-all flex items-center gap-2 group',
                  selectedDomain === domain ? 'bg-cyan-500/15 text-cyan-400 border border-cyan-500/30' : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
                )}
              >
                <Globe className={clsx('w-3 h-3 flex-shrink-0', selectedDomain === domain ? 'text-cyan-400' : 'text-slate-600 group-hover:text-slate-400')} />
                <span className="truncate font-mono text-xs">{domain}</span>
              </button>
            ))}
            {domains.length > 8 && <p className="text-xs text-slate-600 px-3 py-1">+{domains.length - 8} more</p>}
          </div>
        </div>
      )}

      {/* Categories filter */}
      <div className="bg-slate-900/60 backdrop-blur-sm rounded-xl border border-slate-800/80 p-4 sticky top-8">
        <h3 className="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-3 flex items-center gap-2">
          <Tag className="w-3.5 h-3.5" />
          Categories
        </h3>
        <button
          onClick={() => onSelectCategory(null)}
          className={clsx(
            'w-full text-left px-3 py-2 rounded-lg text-sm transition-all mb-2',
            selectedCategory === null ? 'bg-cyan-500/15 text-cyan-400 border border-cyan-500/30' : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
          )}
        >
          All Entries
        </button>

        {messagingCats.length > 0 && (
          <>
            <p className="text-[10px] text-slate-600 font-mono uppercase mt-4 mb-2 px-3 flex items-center gap-1.5">
              <div className="w-1.5 h-1.5 rounded-full bg-violet-500/60" />
              Messaging
            </p>
            {messagingCats.map((cat) => (
              <button
                key={cat.name}
                onClick={() => onSelectCategory(cat.name)}
                className={clsx(
                  'w-full text-left px-3 py-2 rounded-lg text-sm transition-all flex items-center justify-between',
                  selectedCategory === cat.name ? 'bg-violet-500/15 text-violet-400 border border-violet-500/30' : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
                )}
              >
                <span className="truncate">{cat.name}</span>
                <span className="text-xs text-slate-600">{cat.entry_count}</span>
              </button>
            ))}
          </>
        )}

        {conversationCats.length > 0 && (
          <>
            <p className="text-[10px] text-slate-600 font-mono uppercase mt-4 mb-2 px-3 flex items-center gap-1.5">
              <div className="w-1.5 h-1.5 rounded-full bg-amber-500/60" />
              Conversation
            </p>
            {conversationCats.map((cat) => (
              <button
                key={cat.name}
                onClick={() => onSelectCategory(cat.name)}
                className={clsx(
                  'w-full text-left px-3 py-2 rounded-lg text-sm transition-all flex items-center justify-between',
                  selectedCategory === cat.name ? 'bg-amber-500/15 text-amber-400 border border-amber-500/30' : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
                )}
              >
                <span className="truncate">{cat.name}</span>
                <span className="text-xs text-slate-600">{cat.entry_count}</span>
              </button>
            ))}
          </>
        )}
      </div>
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
  const [viewMode, setViewMode] = useState<ViewMode>('grid');
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [selectedType, setSelectedType] = useState<string>('all');
  const [selectedDomain, setSelectedDomain] = useState<string | null>(null);

  const { data: categoriesData } = useQuery({
    queryKey: ['categories'],
    queryFn: api.getCategories,
  });

  const { data: entriesData, isLoading } = useQuery({
    queryKey: ['entries', selectedCategory, selectedType],
    queryFn: () =>
      api.getEntries({
        category: selectedCategory || undefined,
        entry_type: selectedType === 'all' ? undefined : selectedType,
      }),
  });

  const categories = categoriesData?.categories || [];
  const allEntries = entriesData?.entries || [];

  // Extract unique domains from entries
  const domains = useMemo(() => {
    const domainSet = new Set<string>();
    allEntries.forEach((p) => {
      if (p.source_domain) domainSet.add(p.source_domain);
    });
    return Array.from(domainSet).sort();
  }, [allEntries]);

  // Filter entries by domain
  const entries = useMemo(() => {
    if (!selectedDomain) return allEntries;
    return allEntries.filter((e) => e.source_domain === selectedDomain);
  }, [allEntries, selectedDomain]);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold text-white tracking-tight">Knowledge Base</h1>
          <div className="mt-2">
            <StatsBar total={entriesData?.total || 0} filtered={entries.length} domains={domains.length} />
          </div>
        </div>

        <div className="flex items-center gap-3">
          {/* Type Filter */}
          <div className="flex items-center gap-2">
            <Filter className="w-4 h-4 text-slate-500" />
            <select value={selectedType} onChange={(e) => setSelectedType(e.target.value)} className="input text-sm py-1.5 pr-8 bg-slate-800/80 border-slate-700/80">
              <option value="all">All Types</option>
              <option value="document">Document</option>
              <option value="article">Article</option>
              <option value="pdf">PDF</option>
              <option value="code">Code</option>
              <option value="messaging">Messaging</option>
              <option value="conversation">Conversation</option>
            </select>
          </div>

          {/* View Toggle */}
          <div className="flex items-center bg-slate-800/80 rounded-lg p-1 border border-slate-700/50">
            <button
              onClick={() => setViewMode('grid')}
              className={clsx('p-1.5 rounded transition-all', viewMode === 'grid' ? 'bg-cyan-500/20 text-cyan-400' : 'text-slate-500 hover:text-white')}
              title="Grid view"
            >
              <Grid className="w-4 h-4" />
            </button>
            <button
              onClick={() => setViewMode('list')}
              className={clsx('p-1.5 rounded transition-all', viewMode === 'list' ? 'bg-cyan-500/20 text-cyan-400' : 'text-slate-500 hover:text-white')}
              title="List view"
            >
              <List className="w-4 h-4" />
            </button>
            <button
              onClick={() => setViewMode('compact')}
              className={clsx('p-1.5 rounded transition-all', viewMode === 'compact' ? 'bg-cyan-500/20 text-cyan-400' : 'text-slate-500 hover:text-white')}
              title="Compact view"
            >
              <Hash className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      {/* Mobile Filters */}
      <div className="lg:hidden space-y-3">
        <select
          value={selectedDomain || ''}
          onChange={(e) => setSelectedDomain(e.target.value || null)}
          className="input w-full bg-slate-800/80 border-slate-700/80"
        >
          <option value="">All Sources</option>
          {domains.map((domain) => (
            <option key={domain} value={domain}>
              {domain}
            </option>
          ))}
        </select>
        <select
          value={selectedCategory || ''}
          onChange={(e) => setSelectedCategory(e.target.value || null)}
          className="input w-full bg-slate-800/80 border-slate-700/80"
        >
          <option value="">All Categories</option>
          {categories.map((cat) => (
            <option key={cat.name} value={cat.name}>
              {cat.name} ({cat.entry_count})
            </option>
          ))}
        </select>
      </div>

      {/* Main Content */}
      <div className="flex gap-6">
        <CategorySidebar
          categories={categories}
          selectedCategory={selectedCategory}
          onSelectCategory={setSelectedCategory}
          domains={domains}
          selectedDomain={selectedDomain}
          onSelectDomain={setSelectedDomain}
        />

        <div className="flex-1 min-w-0">
          {/* Active Filters */}
          {(selectedCategory || selectedDomain) && (
            <div className="mb-4 flex items-center gap-2 flex-wrap">
              {selectedCategory && (
                <div className="flex items-center gap-2 px-3 py-1.5 bg-violet-500/15 border border-violet-500/30 rounded-lg">
                  <Tag className="w-3.5 h-3.5 text-violet-400" />
                  <span className="text-violet-400 text-sm font-medium">{selectedCategory}</span>
                  <button onClick={() => setSelectedCategory(null)} className="text-violet-400/60 hover:text-violet-400 ml-1 text-lg leading-none">
                    ×
                  </button>
                </div>
              )}
              {selectedDomain && (
                <div className="flex items-center gap-2 px-3 py-1.5 bg-cyan-500/15 border border-cyan-500/30 rounded-lg">
                  <Globe className="w-3.5 h-3.5 text-cyan-400" />
                  <span className="text-cyan-400 text-sm font-mono">{selectedDomain}</span>
                  <button onClick={() => setSelectedDomain(null)} className="text-cyan-400/60 hover:text-cyan-400 ml-1 text-lg leading-none">
                    ×
                  </button>
                </div>
              )}
              <button
                onClick={() => {
                  setSelectedCategory(null);
                  setSelectedDomain(null);
                }}
                className="text-xs text-slate-500 hover:text-white transition-colors px-2"
              >
                Clear all
              </button>
            </div>
          )}

          {isLoading ? (
            <div className="flex items-center justify-center py-20">
              <div className="flex flex-col items-center gap-4">
                <div className="w-10 h-10 border-3 border-cyan-500 border-t-transparent rounded-full animate-spin" />
                <p className="text-slate-500 text-sm">Loading entries...</p>
              </div>
            </div>
          ) : entries.length === 0 ? (
            <div className="bg-slate-900/40 rounded-xl border border-slate-800/60 p-12 text-center">
              <Database className="w-12 h-12 text-slate-700 mx-auto mb-4" />
              <p className="text-slate-400 text-lg">No entries found</p>
              <p className="text-slate-600 text-sm mt-1">Try adjusting your filters or add some content</p>
            </div>
          ) : viewMode === 'grid' ? (
            <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
              {entries.map((entry) => (
                <EntryCard key={entry.id} entry={entry} viewMode="grid" />
              ))}
            </div>
          ) : viewMode === 'list' ? (
            <div className="space-y-3">
              {entries.map((entry) => (
                <EntryCard key={entry.id} entry={entry} viewMode="list" />
              ))}
            </div>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
              {entries.map((entry) => (
                <EntryCard key={entry.id} entry={entry} viewMode="compact" />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
