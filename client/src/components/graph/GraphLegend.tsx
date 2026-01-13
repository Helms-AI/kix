import { useState } from 'react';
import { ChevronUp, ChevronDown } from 'lucide-react';
import clsx from 'clsx';
import type { ClusterByType } from '../../types/graph';

interface LegendItem {
  label: string;
  color: string;
  count: number;
}

interface GraphLegendProps {
  items: LegendItem[];
  clusterBy: ClusterByType;
  show: boolean;
}

export default function GraphLegend({ items, clusterBy, show }: GraphLegendProps) {
  const [isExpanded, setIsExpanded] = useState(true);

  if (!show || items.length === 0) return null;

  const title = clusterBy === 'domain'
    ? 'Domains'
    : clusterBy === 'category'
    ? 'Categories'
    : clusterBy === 'type'
    ? 'Types'
    : 'Categories';

  // Show max 8 items when collapsed
  const displayItems = isExpanded ? items : items.slice(0, 8);
  const hasMore = items.length > 8;

  return (
    <div className="absolute bottom-4 left-4 z-10">
      <div className="card bg-slate-900/95 backdrop-blur-lg border border-slate-700/50 p-3 max-w-xs">
        <button
          onClick={() => setIsExpanded(!isExpanded)}
          className="flex items-center justify-between w-full text-left mb-2"
        >
          <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
            {title}
          </h4>
          {hasMore && (
            isExpanded
              ? <ChevronDown className="w-3 h-3 text-slate-500" />
              : <ChevronUp className="w-3 h-3 text-slate-500" />
          )}
        </button>

        <div className={clsx(
          'grid gap-2',
          items.length > 4 ? 'grid-cols-2' : 'grid-cols-1'
        )}>
          {displayItems.map((item) => (
            <div
              key={item.label}
              className="flex items-center gap-2 group cursor-default"
            >
              <div
                className="w-2.5 h-2.5 rounded-full flex-shrink-0 ring-2 ring-transparent
                           group-hover:ring-white/20 transition-all"
                style={{ backgroundColor: item.color }}
              />
              <span className="text-xs text-slate-400 truncate group-hover:text-slate-200 transition-colors">
                {item.label}
              </span>
              <span className="text-xs text-slate-600 ml-auto">
                {item.count}
              </span>
            </div>
          ))}
        </div>

        {!isExpanded && hasMore && (
          <button
            onClick={() => setIsExpanded(true)}
            className="text-xs text-cyan-400 hover:text-cyan-300 mt-2 transition-colors"
          >
            +{items.length - 8} more
          </button>
        )}
      </div>
    </div>
  );
}
