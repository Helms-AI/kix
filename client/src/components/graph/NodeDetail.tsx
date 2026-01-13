import { X, ExternalLink, Tag, Globe, Calendar, Link2 } from 'lucide-react';
import { Link } from 'react-router-dom';
import clsx from 'clsx';
import type { GraphNodeExtended } from '../../types/graph';
import { GRAPH_COLORS } from '../../types/graph';

interface NodeDetailProps {
  node: GraphNodeExtended | null;
  relatedCount: number;
  onClose: () => void;
}

const TYPE_STYLES: Record<string, { bg: string; text: string; label: string }> = {
  document: { bg: 'bg-blue-500/20', text: 'text-blue-400', label: 'Document' },
  article: { bg: 'bg-emerald-500/20', text: 'text-emerald-400', label: 'Article' },
  pdf: { bg: 'bg-rose-500/20', text: 'text-rose-400', label: 'PDF' },
  code: { bg: 'bg-amber-500/20', text: 'text-amber-400', label: 'Code' },
};

export default function NodeDetail({ node, relatedCount, onClose }: NodeDetailProps) {
  if (!node) return null;

  const typeStyle = TYPE_STYLES[node.entry_type] || TYPE_STYLES.document;

  return (
    <div className="absolute top-4 right-4 z-10 w-72 animate-in slide-in-from-right-4 duration-200">
      <div className="card bg-slate-900/95 backdrop-blur-lg border border-slate-700/50 overflow-hidden">
        {/* Header with gradient accent */}
        <div
          className="h-1"
          style={{ background: `linear-gradient(90deg, ${node.color || GRAPH_COLORS.primary}, transparent)` }}
        />

        <div className="p-4">
          {/* Close button */}
          <button
            onClick={onClose}
            className="absolute top-3 right-3 p-1.5 text-slate-400 hover:text-white
                       hover:bg-slate-800 rounded-lg transition-colors"
          >
            <X className="w-4 h-4" />
          </button>

          {/* Title & Type Badge */}
          <div className="pr-8">
            <h3 className="font-semibold text-white text-base leading-tight mb-2">
              {node.label}
            </h3>
            <div className="flex items-center gap-2 flex-wrap">
              <span className={clsx(
                'px-2 py-0.5 text-xs font-medium rounded-full',
                typeStyle.bg,
                typeStyle.text
              )}>
                {typeStyle.label}
              </span>
              <span
                className="px-2 py-0.5 text-xs font-medium rounded-full"
                style={{
                  backgroundColor: `${node.color}20`,
                  color: node.color,
                }}
              >
                {node.category}
              </span>
            </div>
          </div>

          {/* Description */}
          {node.description && (
            <p className="text-sm text-slate-400 mt-3 line-clamp-3">
              {node.description}
            </p>
          )}

          {/* Metadata */}
          <div className="mt-4 space-y-2">
            {node.source_domain && (
              <div className="flex items-center gap-2 text-xs text-slate-500">
                <Globe className="w-3.5 h-3.5" />
                <span className="truncate">{node.source_domain}</span>
              </div>
            )}

            {node.tags && node.tags.length > 0 && (
              <div className="flex items-start gap-2 text-xs">
                <Tag className="w-3.5 h-3.5 text-slate-500 mt-0.5 flex-shrink-0" />
                <div className="flex flex-wrap gap-1">
                  {node.tags.slice(0, 5).map((tag) => (
                    <span
                      key={tag}
                      className="px-1.5 py-0.5 bg-slate-800 text-slate-400 rounded"
                    >
                      {tag}
                    </span>
                  ))}
                  {node.tags.length > 5 && (
                    <span className="text-slate-500">+{node.tags.length - 5}</span>
                  )}
                </div>
              </div>
            )}

            {relatedCount > 0 && (
              <div className="flex items-center gap-2 text-xs text-slate-500">
                <Link2 className="w-3.5 h-3.5" />
                <span>{relatedCount} connected nodes</span>
              </div>
            )}

            {node.created_at && (
              <div className="flex items-center gap-2 text-xs text-slate-500">
                <Calendar className="w-3.5 h-3.5" />
                <span>{new Date(node.created_at).toLocaleDateString()}</span>
              </div>
            )}
          </div>

          {/* View Entry Button */}
          <Link
            to={`/entries/${encodeURIComponent(node.id)}`}
            className="flex items-center justify-center gap-2 mt-4 px-4 py-2
                       bg-cyan-500/20 hover:bg-cyan-500/30 text-cyan-400
                       rounded-lg text-sm font-medium transition-colors"
          >
            <span>View Entry</span>
            <ExternalLink className="w-3.5 h-3.5" />
          </Link>
        </div>
      </div>
    </div>
  );
}
