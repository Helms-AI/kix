import { useParams, Link } from 'react-router-dom';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState, useMemo } from 'react';
import {
  ArrowLeft,
  Tag,
  Link as LinkIcon,
  ChevronRight,
  Layers,
  RefreshCw,
  Globe,
  ExternalLink,
  Copy,
  Clock,
  FileText,
  Code,
  FileType,
} from 'lucide-react';
import MarkdownViewer from '../components/MarkdownViewer';
import clsx from 'clsx';
import { api } from '../api/client';
import type { Chunk } from '../types';

export default function EntryDetail() {
  const { id } = useParams<{ id: string }>();
  const decodedId = id ? decodeURIComponent(id) : '';
  const queryClient = useQueryClient();
  const [isReindexing, setIsReindexing] = useState(false);
  const [reindexError, setReindexError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const { data: entry, isLoading, error } = useQuery({
    queryKey: ['entry', decodedId],
    queryFn: () => api.getEntry(decodedId),
    enabled: !!decodedId,
  });

  const { data: relatedData } = useQuery({
    queryKey: ['related', decodedId],
    queryFn: () => api.getRelatedEntries(decodedId),
    enabled: !!decodedId,
  });

  const { data: chunksData, isLoading: isLoadingChunks } = useQuery({
    queryKey: ['chunks', decodedId],
    queryFn: () => api.getChunks(decodedId),
    enabled: !!decodedId,
  });

  const relatedEntries = relatedData?.entries || [];
  const rawChunks = chunksData?.chunks || [];

  // Sort chunks by chunk_index to ensure proper order
  const chunks = useMemo(() => {
    return [...rawChunks].sort((a, b) => {
      // Sort by chunk_index if both have it, otherwise maintain original order
      const aIndex = a.chunk_index ?? Number.MAX_SAFE_INTEGER;
      const bIndex = b.chunk_index ?? Number.MAX_SAFE_INTEGER;
      return aIndex - bIndex;
    });
  }, [rawChunks]);

  const handleReindex = async () => {
    if (!entry?.source_path) return;
    setIsReindexing(true);
    setReindexError(null);
    try {
      await api.reindexEntry(decodedId, { skip_render: false });
      // Refetch entry and chunks after re-indexing starts
      // The actual content will update after the job completes
      queryClient.invalidateQueries({ queryKey: ['entry', decodedId] });
      queryClient.invalidateQueries({ queryKey: ['chunks', decodedId] });
    } catch (err) {
      console.error('Re-index failed:', err);
      setReindexError(err instanceof Error ? err.message : 'Re-index failed');
    } finally {
      setIsReindexing(false);
    }
  };

  const handleCopy = async () => {
    const textToCopy = entry?.source_path || entry?.id || '';
    await navigator.clipboard.writeText(textToCopy);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Helper for relative time
  const formatRelativeTime = (dateStr?: string) => {
    if (!dateStr) return null;
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
    if (diffDays === 0) return 'Today';
    if (diffDays === 1) return 'Yesterday';
    if (diffDays < 7) return `${diffDays} days ago`;
    if (diffDays < 30) return `${Math.floor(diffDays / 7)} weeks ago`;
    return date.toLocaleDateString();
  };

  // Helper for source type icon
  const getSourceTypeIcon = (type?: string) => {
    switch (type) {
      case 'html':
      case 'url':
        return Globe;
      case 'pdf':
        return FileText;
      case 'source_code':
        return Code;
      default:
        return FileType;
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-[60vh]">
        <div className="flex flex-col items-center gap-4">
          <div className="w-12 h-12 border-4 border-cyan-500 border-t-transparent rounded-full animate-spin" />
          <p className="text-slate-400 font-mono">Loading entry...</p>
        </div>
      </div>
    );
  }

  if (error || !entry) {
    return (
      <div className="space-y-6">
        <Link to="/entries" className="inline-flex items-center gap-2 text-slate-400 hover:text-white transition-colors">
          <ArrowLeft className="w-4 h-4" />
          Back to entries
        </Link>
        <div className="card p-8 text-center">
          <p className="text-red-400">Entry not found</p>
          <p className="text-slate-500 text-sm mt-2">{(error as Error)?.message || 'The requested entry does not exist'}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Breadcrumb */}
      <nav className="flex items-center gap-2 text-sm">
        <Link to="/entries" className="text-slate-400 hover:text-white transition-colors">
          Entries
        </Link>
        <ChevronRight className="w-4 h-4 text-slate-600" />
        <span className="text-cyan-400">{entry.title}</span>
      </nav>

      {/* Header */}
      <div className="card overflow-hidden">
        {/* Metadata Bar */}
        <div className="px-6 py-3 bg-slate-800/50 border-b border-slate-700/50 flex flex-wrap items-center gap-3">
          {/* Entry Type Badge */}
          <span
            className={clsx(
              'px-2.5 py-1 text-xs font-semibold rounded-md uppercase tracking-wide',
              entry.entry_type === 'messaging'
                ? 'bg-violet-500/20 text-violet-400 ring-1 ring-violet-500/30'
                : entry.entry_type === 'code'
                  ? 'bg-emerald-500/20 text-emerald-400 ring-1 ring-emerald-500/30'
                  : 'bg-amber-500/20 text-amber-400 ring-1 ring-amber-500/30'
            )}
          >
            {entry.entry_type}
          </span>

          {/* Source Type Badge */}
          {entry.source_type && (
            <span className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-md bg-slate-700/50 text-slate-300 ring-1 ring-slate-600/50">
              {(() => {
                const Icon = getSourceTypeIcon(entry.source_type);
                return <Icon className="w-3 h-3" />;
              })()}
              {entry.source_type}
            </span>
          )}

          {/* Source Domain */}
          {entry.source_domain && (
            <a
              href={entry.source_path}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-md bg-cyan-500/10 text-cyan-400 ring-1 ring-cyan-500/30 hover:bg-cyan-500/20 transition-colors"
            >
              <Globe className="w-3 h-3" />
              {entry.source_domain}
              <ExternalLink className="w-3 h-3 opacity-50" />
            </a>
          )}

          {/* Divider */}
          <div className="h-4 w-px bg-slate-700 hidden sm:block" />

          {/* Chunks Count */}
          <span className="inline-flex items-center gap-1.5 text-xs text-slate-400">
            <Layers className="w-3.5 h-3.5" />
            <span className="font-mono">{chunks.length}</span> chunks
          </span>

          {/* Indexed Date */}
          {(entry.updated_at || entry.created_at) && (
            <span className="inline-flex items-center gap-1.5 text-xs text-slate-500">
              <Clock className="w-3.5 h-3.5" />
              {formatRelativeTime(entry.updated_at || entry.created_at)}
            </span>
          )}
        </div>

        {/* Main Content */}
        <div className="p-6">
          <div className="flex flex-col lg:flex-row lg:items-start gap-6">
            {/* Title & Tags */}
            <div className="flex-1 min-w-0">
              <h1 className="text-3xl font-bold text-white mb-4 leading-tight">
                {entry.title}
              </h1>

              {/* Tags */}
              {entry.tags && entry.tags.length > 0 && (
                <div className="flex items-center gap-2 flex-wrap">
                  <Tag className="w-4 h-4 text-slate-500 flex-shrink-0" />
                  {entry.tags.map((tag) => (
                    <Link
                      key={tag}
                      to={`/entries?tag=${encodeURIComponent(tag)}`}
                      className="px-2.5 py-1 text-xs font-medium bg-slate-800 text-slate-300 rounded-md hover:bg-slate-700 hover:text-white transition-colors ring-1 ring-slate-700 hover:ring-slate-600"
                    >
                      {tag}
                    </Link>
                  ))}
                </div>
              )}
            </div>

            {/* Actions */}
            <div className="flex items-center gap-2 flex-shrink-0">
              {/* External Link */}
              {entry.source_path && (
                <a
                  href={entry.source_path}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="p-2.5 rounded-lg bg-slate-800 text-slate-400 hover:text-white hover:bg-slate-700 transition-colors ring-1 ring-slate-700"
                  title="Open source"
                >
                  <ExternalLink className="w-4 h-4" />
                </a>
              )}

              {/* Copy Button */}
              <button
                onClick={handleCopy}
                className={clsx(
                  'p-2.5 rounded-lg transition-all ring-1',
                  copied
                    ? 'bg-emerald-500/20 text-emerald-400 ring-emerald-500/30'
                    : 'bg-slate-800 text-slate-400 hover:text-white hover:bg-slate-700 ring-slate-700'
                )}
                title={copied ? 'Copied!' : 'Copy URL'}
              >
                <Copy className="w-4 h-4" />
              </button>

              {/* Re-index Button */}
              {entry.source_path && (
                <button
                  onClick={handleReindex}
                  disabled={isReindexing}
                  className={clsx(
                    'inline-flex items-center gap-2 px-4 py-2.5 rounded-lg font-medium transition-all ring-1',
                    isReindexing
                      ? 'bg-slate-700 text-slate-400 cursor-not-allowed ring-slate-600'
                      : 'bg-cyan-600 hover:bg-cyan-500 text-white ring-cyan-500 shadow-lg shadow-cyan-500/20'
                  )}
                >
                  <RefreshCw className={clsx('w-4 h-4', isReindexing && 'animate-spin')} />
                  {isReindexing ? 'Re-indexing...' : 'Re-index'}
                </button>
              )}
            </div>
          </div>

          {/* Error Message */}
          {reindexError && (
            <p className="mt-4 text-sm text-red-400 bg-red-500/10 px-3 py-2 rounded-lg">
              {reindexError}
            </p>
          )}
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Main Content */}
        <div className="lg:col-span-2 space-y-6">
          {/* Content */}
          <div className="card p-6">
            <h2 className="text-xl font-semibold text-white flex items-center gap-2 mb-4">
              <Layers className="w-5 h-5 text-cyan-400" />
              Content
              {chunks.length > 0 && (
                <span className="text-sm font-normal text-slate-500">
                  ({chunks.length} chunk{chunks.length !== 1 ? 's' : ''})
                </span>
              )}
            </h2>
            <div className="prose prose-invert prose-slate max-w-none">
              {isLoadingChunks ? (
                <div className="flex items-center gap-3 text-slate-400">
                  <div className="w-5 h-5 border-2 border-cyan-500 border-t-transparent rounded-full animate-spin" />
                  Loading content...
                </div>
              ) : chunks.length > 0 ? (
                <div className="space-y-4">
                  {chunks.map((chunk: Chunk, index: number) => (
                    <div
                      key={chunk.id}
                      className="relative"
                    >
                      <div className="flex items-start gap-3">
                        <span className="flex-shrink-0 px-2 py-0.5 text-xs font-mono bg-slate-700 text-slate-400 rounded-full">
                          {chunk.chunk_index ?? index}
                        </span>
                        <MarkdownViewer content={chunk.text} className="flex-1" />
                      </div>
                      {index < chunks.length - 1 && (
                        <hr className="border-slate-700/50 mt-4" />
                      )}
                    </div>
                  ))}
                </div>
              ) : entry.description ? (
                <MarkdownViewer content={entry.description} />
              ) : (
                <p className="text-slate-500 italic">No content available</p>
              )}
            </div>
          </div>
        </div>

        {/* Sidebar */}
        <div className="space-y-6">
          {/* Related Entries */}
          <div className="card p-6">
            <h2 className="text-lg font-semibold text-white flex items-center gap-2 mb-4">
              <LinkIcon className="w-5 h-5 text-cyan-400" />
              Related Entries
            </h2>
            {relatedEntries.length === 0 ? (
              <p className="text-slate-500 text-sm">No related entries found</p>
            ) : (
              <div className="space-y-2">
                {relatedEntries.map((related) => (
                  <Link
                    key={related.id}
                    to={`/entries/${encodeURIComponent(related.id)}`}
                    className="block p-3 rounded-lg bg-slate-800/50 hover:bg-slate-800 border border-slate-700 hover:border-cyan-500/50 transition-all group"
                  >
                    <div className="flex items-center justify-between">
                      <span className="text-slate-300 group-hover:text-white text-sm font-medium">
                        {related.title}
                      </span>
                      <span
                        className={clsx(
                          'px-2 py-0.5 text-xs font-mono rounded',
                          related.entry_type === 'messaging'
                            ? 'bg-violet-500/20 text-violet-400'
                            : 'bg-amber-500/20 text-amber-400'
                        )}
                      >
                        {related.entry_type}
                      </span>
                    </div>
                  </Link>
                ))}
              </div>
            )}
          </div>

          {/* Quick Links */}
          <div className="card p-6">
            <h2 className="text-lg font-semibold text-white mb-4">Quick Links</h2>
            <div className="space-y-2">
              <Link
                to="/search"
                className="block p-3 rounded-lg bg-slate-800/50 hover:bg-slate-800 border border-slate-700 hover:border-cyan-500/50 transition-all text-sm text-slate-300 hover:text-white"
              >
                Search for similar entries
              </Link>
              <Link
                to="/graph"
                className="block p-3 rounded-lg bg-slate-800/50 hover:bg-slate-800 border border-slate-700 hover:border-cyan-500/50 transition-all text-sm text-slate-300 hover:text-white"
              >
                View in graph
              </Link>
            </div>
          </div>
        </div>
      </div>

      {/* Back Link */}
      <div className="pt-4">
        <Link
          to="/entries"
          className="inline-flex items-center gap-2 text-slate-400 hover:text-white transition-colors"
        >
          <ArrowLeft className="w-4 h-4" />
          Back to all entries
        </Link>
      </div>
    </div>
  );
}
