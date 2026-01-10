import { useParams, Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { ArrowLeft, Tag, Link as LinkIcon, FileText, ChevronRight } from 'lucide-react';
import clsx from 'clsx';
import { api } from '../api/client';

export default function EntryDetail() {
  const { id } = useParams<{ id: string }>();
  const decodedId = id ? decodeURIComponent(id) : '';

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

  const relatedEntries = relatedData?.entries || [];

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
      <div className="card p-8">
        <div className="flex flex-col lg:flex-row lg:items-start gap-6">
          <div className="flex-1">
            <div className="flex items-center gap-3 mb-4">
              <span
                className={clsx(
                  'px-3 py-1 text-sm font-mono rounded-full',
                  entry.entry_type === 'messaging'
                    ? 'bg-violet-500/20 text-violet-400 border border-violet-500/30'
                    : 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                )}
              >
                {entry.entry_type}
              </span>
            </div>
            <h1 className="text-4xl font-bold text-white mb-4">{entry.title}</h1>

            {/* Tags */}
            {entry.tags && entry.tags.length > 0 && (
              <div className="flex items-center gap-2 flex-wrap">
                <Tag className="w-4 h-4 text-slate-500" />
                {entry.tags.map((tag) => (
                  <Link
                    key={tag}
                    to={`/patterns?tag=${encodeURIComponent(tag)}`}
                    className="px-3 py-1 text-sm bg-slate-800 text-slate-300 rounded-lg hover:bg-slate-700 hover:text-white transition-colors"
                  >
                    {tag}
                  </Link>
                ))}
              </div>
            )}
          </div>

          {/* Entry ID */}
          <div className="lg:text-right">
            <p className="text-xs text-slate-500 font-mono uppercase tracking-wider mb-1">Entry ID</p>
            <p className="text-sm text-slate-400 font-mono bg-slate-800 px-3 py-1.5 rounded-lg inline-block">
              {entry.id}
            </p>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Main Content */}
        <div className="lg:col-span-2 space-y-6">
          {/* Description */}
          <div className="card p-6">
            <h2 className="text-xl font-semibold text-white flex items-center gap-2 mb-4">
              <FileText className="w-5 h-5 text-cyan-400" />
              Description
            </h2>
            <div className="prose prose-invert prose-slate max-w-none">
              <p className="text-slate-300 leading-relaxed whitespace-pre-wrap">
                {entry.description}
              </p>
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
