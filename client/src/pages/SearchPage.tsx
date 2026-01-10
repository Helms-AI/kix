import { useState, useCallback } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { Search, Filter, Sparkles, ArrowRight, Clock, Globe, Code, FileText, BookOpen, Hash } from 'lucide-react';
import clsx from 'clsx';
import { api } from '../api/client';
import type { SearchResult, ChunkType } from '../types';

const EXAMPLE_QUERIES = [
  'How to split a large message into smaller parts',
  'Route messages based on content',
  'Combine multiple messages into one',
  'Filter unwanted messages',
  'Transform message format',
  'Handle request-reply patterns',
];

// Helper to get chunk type styling
function getChunkTypeStyle(type?: ChunkType) {
  const styles: Record<string, { bg: string; text: string; border: string; icon: React.ReactNode; label: string }> = {
    summary: {
      bg: 'bg-violet-500/10',
      text: 'text-violet-400',
      border: 'border-violet-500/30',
      icon: <BookOpen className="w-3.5 h-3.5" />,
      label: 'Summary',
    },
    content: {
      bg: 'bg-blue-500/10',
      text: 'text-blue-400',
      border: 'border-blue-500/30',
      icon: <FileText className="w-3.5 h-3.5" />,
      label: 'Content',
    },
    code: {
      bg: 'bg-amber-500/10',
      text: 'text-amber-400',
      border: 'border-amber-500/30',
      icon: <Code className="w-3.5 h-3.5" />,
      label: 'Code',
    },
    header: {
      bg: 'bg-emerald-500/10',
      text: 'text-emerald-400',
      border: 'border-emerald-500/30',
      icon: <Hash className="w-3.5 h-3.5" />,
      label: 'Header',
    },
  };
  return styles[type || 'content'] || styles.content;
}

// Helper to get entry type styling
function getEntryTypeStyle(type: string) {
  const styles: Record<string, { bg: string; text: string }> = {
    messaging: { bg: 'bg-violet-500/15', text: 'text-violet-400' },
    conversation: { bg: 'bg-amber-500/15', text: 'text-amber-400' },
    document: { bg: 'bg-blue-500/15', text: 'text-blue-400' },
    pdf: { bg: 'bg-rose-500/15', text: 'text-rose-400' },
    article: { bg: 'bg-emerald-500/15', text: 'text-emerald-400' },
    code: { bg: 'bg-amber-500/15', text: 'text-amber-400' },
  };
  return styles[type.toLowerCase()] || { bg: 'bg-slate-500/15', text: 'text-slate-400' };
}

// Code block renderer for code chunks
function CodeBlockDisplay({ text }: { text: string }) {
  // Detect if this is a code block (starts with ```)
  const isCodeFenced = text.startsWith('```');
  const lines = text.split('\n');

  if (isCodeFenced && lines.length > 2) {
    const language = lines[0].replace('```', '').trim() || 'code';
    const codeContent = lines.slice(1, -1).join('\n');

    return (
      <div className="mt-3 rounded-lg overflow-hidden border border-slate-700/50">
        <div className="flex items-center justify-between px-3 py-1.5 bg-slate-800/80 border-b border-slate-700/50">
          <span className="text-[10px] font-mono text-slate-400 uppercase tracking-wider">{language}</span>
          <Code className="w-3.5 h-3.5 text-amber-400/60" />
        </div>
        <pre className="p-3 text-sm bg-slate-900/80 overflow-x-auto scrollbar-thin">
          <code className="text-slate-300 font-mono text-xs leading-relaxed">{codeContent}</code>
        </pre>
      </div>
    );
  }

  // Regular text display
  return <p className="text-sm text-slate-400 mt-2 line-clamp-4 leading-relaxed">{text}</p>;
}

function SearchResultCard({ result, rank }: { result: SearchResult; rank: number }) {
  const scorePercent = Math.round(result.score * 100);
  const chunkStyle = getChunkTypeStyle(result.chunk_type);
  const entryStyle = getEntryTypeStyle(result.entry_type);
  const isCodeChunk = result.chunk_type === 'code';

  return (
    <Link
      to={`/entries/${encodeURIComponent(result.entry_id || result.id)}`}
      className={clsx(
        'group block bg-slate-900/60 backdrop-blur-sm rounded-xl border overflow-hidden hover:shadow-lg transition-all duration-300',
        isCodeChunk ? 'border-amber-500/20 hover:border-amber-500/40 hover:shadow-amber-500/5' : 'border-slate-800/80 hover:border-cyan-500/40 hover:shadow-cyan-500/5'
      )}
    >
      {/* Top accent bar based on chunk type */}
      <div className={clsx('h-1 w-full', chunkStyle.bg.replace('/10', '/40'))} />

      <div className="p-5">
        <div className="flex items-start gap-4">
          {/* Rank indicator */}
          <div
            className={clsx(
              'flex-shrink-0 w-9 h-9 rounded-lg flex items-center justify-center text-sm font-bold',
              rank <= 3 ? 'bg-gradient-to-br from-cyan-500 to-teal-500 text-white' : 'bg-slate-800 text-slate-400'
            )}
          >
            {rank}
          </div>

          <div className="flex-1 min-w-0">
            {/* Header row */}
            <div className="flex items-start justify-between gap-3 mb-2">
              <div className="flex-1 min-w-0">
                <h3 className="font-semibold text-white group-hover:text-cyan-400 transition-colors truncate">{result.entry_title}</h3>
                <div className="flex items-center gap-2 mt-1.5 flex-wrap">
                  {/* Entry type badge */}
                  <span className={clsx('px-2 py-0.5 text-xs font-medium rounded', entryStyle.bg, entryStyle.text)}>{result.entry_type}</span>

                  {/* Chunk type badge */}
                  <div className={clsx('flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium', chunkStyle.bg, chunkStyle.text)}>
                    {chunkStyle.icon}
                    <span>{chunkStyle.label}</span>
                  </div>

                  {/* Source domain */}
                  {result.source_domain && (
                    <div className="flex items-center gap-1 text-xs text-slate-500">
                      <Globe className="w-3 h-3" />
                      <span className="font-mono truncate max-w-[120px]">{result.source_domain}</span>
                    </div>
                  )}
                </div>
              </div>

              {/* Score indicator */}
              <div className="flex flex-col items-end gap-1 flex-shrink-0">
                <div className="flex items-center gap-2">
                  <div className="w-12 h-1.5 bg-slate-800 rounded-full overflow-hidden">
                    <div
                      className={clsx('h-full rounded-full', scorePercent >= 70 ? 'bg-gradient-to-r from-cyan-500 to-teal-500' : 'bg-slate-600')}
                      style={{ width: `${scorePercent}%` }}
                    />
                  </div>
                  <span className={clsx('text-xs font-mono', scorePercent >= 70 ? 'text-cyan-400' : 'text-slate-500')}>{scorePercent}%</span>
                </div>
              </div>
            </div>

            {/* Content based on chunk type */}
            {isCodeChunk ? <CodeBlockDisplay text={result.text} /> : <p className="text-sm text-slate-400 mt-2 line-clamp-3 leading-relaxed">{result.text}</p>}

            {/* Tags */}
            {result.tags && result.tags.length > 0 && (
              <div className="flex flex-wrap gap-1.5 mt-3">
                {result.tags.slice(0, 3).map((tag) => (
                  <span key={tag} className="px-2 py-0.5 text-xs bg-slate-800/80 text-slate-400 rounded-md border border-slate-700/50">
                    {tag}
                  </span>
                ))}
                {result.tags.length > 3 && <span className="px-2 py-0.5 text-xs bg-slate-800/50 text-slate-500 rounded-md">+{result.tags.length - 3}</span>}
              </div>
            )}
          </div>

          <ArrowRight className="w-5 h-5 text-slate-600 group-hover:text-cyan-400 flex-shrink-0 transition-colors mt-1" />
        </div>
      </div>
    </Link>
  );
}

export default function SearchPage() {
  const [query, setQuery] = useState('');
  const [submittedQuery, setSubmittedQuery] = useState('');
  const [entryType, setEntryType] = useState<string>('all');
  const [sourceDomain, setSourceDomain] = useState<string>('');
  const [recentSearches, setRecentSearches] = useState<string[]>(() => {
    try {
      return JSON.parse(localStorage.getItem('kix-recent-searches') || '[]');
    } catch {
      return [];
    }
  });

  const {
    data: searchData,
    isLoading,
    isFetching,
  } = useQuery({
    queryKey: ['search', submittedQuery, entryType, sourceDomain],
    queryFn: () =>
      api.search({
        q: submittedQuery,
        entry_type: entryType === 'all' ? undefined : entryType,
        source_domain: sourceDomain || undefined,
        limit: 20,
      }),
    enabled: submittedQuery.length > 0,
  });

  const handleSearch = useCallback(
    (searchQuery: string) => {
      const trimmed = searchQuery.trim();
      if (!trimmed) return;

      setSubmittedQuery(trimmed);

      // Update recent searches
      const updated = [trimmed, ...recentSearches.filter((s) => s !== trimmed)].slice(0, 5);
      setRecentSearches(updated);
      localStorage.setItem('kix-recent-searches', JSON.stringify(updated));
    },
    [recentSearches]
  );

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    handleSearch(query);
  };

  const results = searchData?.results || [];

  // Group results by chunk type for stats
  const chunkStats = results.reduce(
    (acc, r) => {
      const type = r.chunk_type || 'content';
      acc[type] = (acc[type] || 0) + 1;
      return acc;
    },
    {} as Record<string, number>
  );

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-3xl font-bold text-white tracking-tight">Semantic Search</h1>
        <p className="text-slate-400 mt-1">Search your knowledge base using natural language</p>
      </div>

      {/* Search Form */}
      <form onSubmit={handleSubmit} className="bg-slate-900/60 backdrop-blur-sm rounded-xl border border-slate-800/80 p-6">
        <div className="flex flex-col lg:flex-row gap-3">
          <div className="flex-1 relative">
            <Search className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-slate-500" />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Describe what you're looking for..."
              className="input w-full pl-12 pr-4 py-3 text-lg bg-slate-800/80 border-slate-700/80"
              autoFocus
            />
            {(isLoading || isFetching) && (
              <div className="absolute right-4 top-1/2 -translate-y-1/2">
                <div className="w-5 h-5 border-2 border-cyan-500 border-t-transparent rounded-full animate-spin" />
              </div>
            )}
          </div>
          <div className="flex gap-2">
            <select value={entryType} onChange={(e) => setEntryType(e.target.value)} className="input py-3 px-4 bg-slate-800/80 border-slate-700/80">
              <option value="all">All Types</option>
              <option value="document">Document</option>
              <option value="article">Article</option>
              <option value="pdf">PDF</option>
              <option value="code">Code</option>
              <option value="messaging">Messaging</option>
              <option value="conversation">Conversation</option>
            </select>
            <input
              type="text"
              value={sourceDomain}
              onChange={(e) => setSourceDomain(e.target.value)}
              placeholder="Filter by domain..."
              className="input py-3 px-4 bg-slate-800/80 border-slate-700/80 w-44"
            />
            <button type="submit" className="btn-primary px-6 flex items-center gap-2">
              <Sparkles className="w-5 h-5" />
              Search
            </button>
          </div>
        </div>
      </form>

      {/* Before Search: Show Examples */}
      {!submittedQuery && (
        <div className="space-y-6">
          {/* Recent Searches */}
          {recentSearches.length > 0 && (
            <div className="bg-slate-900/60 backdrop-blur-sm rounded-xl border border-slate-800/80 p-6">
              <h3 className="text-xs font-semibold text-slate-500 uppercase tracking-wider flex items-center gap-2 mb-4">
                <Clock className="w-4 h-4" />
                Recent Searches
              </h3>
              <div className="flex flex-wrap gap-2">
                {recentSearches.map((search) => (
                  <button
                    key={search}
                    onClick={() => {
                      setQuery(search);
                      handleSearch(search);
                    }}
                    className="px-3 py-1.5 text-sm bg-slate-800/80 text-slate-300 rounded-lg hover:bg-slate-700 hover:text-white transition-colors border border-slate-700/50"
                  >
                    {search}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Example Queries */}
          <div className="bg-slate-900/60 backdrop-blur-sm rounded-xl border border-slate-800/80 p-6">
            <h3 className="text-xs font-semibold text-slate-500 uppercase tracking-wider flex items-center gap-2 mb-4">
              <Filter className="w-4 h-4" />
              Example Queries
            </h3>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              {EXAMPLE_QUERIES.map((example) => (
                <button
                  key={example}
                  onClick={() => {
                    setQuery(example);
                    handleSearch(example);
                  }}
                  className="text-left p-4 rounded-lg bg-slate-800/50 hover:bg-slate-800 border border-slate-700/50 hover:border-cyan-500/30 transition-all group"
                >
                  <div className="flex items-center gap-3">
                    <Search className="w-4 h-4 text-slate-500 group-hover:text-cyan-400 flex-shrink-0" />
                    <span className="text-slate-300 group-hover:text-white">{example}</span>
                  </div>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Search Results */}
      {submittedQuery && (
        <div className="space-y-4">
          {/* Results header with stats */}
          <div className="flex items-center justify-between flex-wrap gap-4">
            <div>
              <p className="text-slate-400">
                <span className="text-white font-semibold">{results.length}</span> results for{' '}
                <span className="text-cyan-400 font-medium">"{submittedQuery}"</span>
              </p>
              {/* Chunk type breakdown */}
              {results.length > 0 && (
                <div className="flex items-center gap-3 mt-2">
                  {Object.entries(chunkStats).map(([type, count]) => {
                    const style = getChunkTypeStyle(type as ChunkType);
                    return (
                      <div key={type} className={clsx('flex items-center gap-1 text-xs', style.text)}>
                        {style.icon}
                        <span>
                          {count} {style.label}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
            {results.length > 0 && (
              <button
                onClick={() => {
                  setSubmittedQuery('');
                  setQuery('');
                }}
                className="text-sm text-slate-500 hover:text-white transition-colors"
              >
                Clear search
              </button>
            )}
          </div>

          {isLoading ? (
            <div className="flex items-center justify-center py-16">
              <div className="flex flex-col items-center gap-4">
                <div className="w-10 h-10 border-3 border-cyan-500 border-t-transparent rounded-full animate-spin" />
                <p className="text-slate-500 text-sm font-mono">Searching...</p>
              </div>
            </div>
          ) : results.length === 0 ? (
            <div className="bg-slate-900/40 rounded-xl border border-slate-800/60 p-12 text-center">
              <Search className="w-12 h-12 text-slate-700 mx-auto mb-4" />
              <p className="text-slate-400 text-lg">No entries found matching your query</p>
              <p className="text-slate-600 text-sm mt-1">Try different keywords or phrasing</p>
            </div>
          ) : (
            <div className="space-y-4">
              {results.map((result, index) => (
                <SearchResultCard key={`${result.id}-${index}`} result={result} rank={index + 1} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
