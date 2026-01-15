import { useState, useCallback, useMemo } from 'react';
import {
  ChevronDown,
  ChevronRight,
  Globe,
  FileText,
  FileType,
  Tag,
  FolderOpen,
  Layers,
  ChevronUp,
} from 'lucide-react';
import clsx from 'clsx';
import type { Entry } from '../types';
import type { EntryGroup } from '../hooks/useEntryGroups';
import type { ViewMode, GroupByDimension } from '../hooks/useEntriesFilters';

interface GroupedEntryViewProps {
  groups: EntryGroup[];
  groupBy: GroupByDimension;
  viewMode: ViewMode;
  onGroupClick?: (groupKey: string) => void;
  renderEntry: (entry: Entry, viewMode: ViewMode) => React.ReactNode;
}

// Group styling based on dimension
const GROUP_STYLES: Record<GroupByDimension, {
  icon: React.ReactNode;
  gradient: string;
  accent: string;
  border: string;
  badge: string;
}> = {
  source_domain: {
    icon: <Globe className="w-4 h-4" />,
    gradient: 'from-teal-500/20 via-transparent to-transparent',
    accent: 'text-teal-400',
    border: 'border-teal-500/20',
    badge: 'bg-teal-500/15 text-teal-400 border-teal-500/30',
  },
  entry_type: {
    icon: <FileText className="w-4 h-4" />,
    gradient: 'from-blue-500/20 via-transparent to-transparent',
    accent: 'text-blue-400',
    border: 'border-blue-500/20',
    badge: 'bg-blue-500/15 text-blue-400 border-blue-500/30',
  },
  source_type: {
    icon: <FileType className="w-4 h-4" />,
    gradient: 'from-purple-500/20 via-transparent to-transparent',
    accent: 'text-purple-400',
    border: 'border-purple-500/20',
    badge: 'bg-purple-500/15 text-purple-400 border-purple-500/30',
  },
  tag: {
    icon: <Tag className="w-4 h-4" />,
    gradient: 'from-amber-500/20 via-transparent to-transparent',
    accent: 'text-amber-400',
    border: 'border-amber-500/20',
    badge: 'bg-amber-500/15 text-amber-400 border-amber-500/30',
  },
  none: {
    icon: <Layers className="w-4 h-4" />,
    gradient: 'from-slate-500/20 via-transparent to-transparent',
    accent: 'text-slate-400',
    border: 'border-slate-500/20',
    badge: 'bg-slate-500/15 text-slate-400 border-slate-500/30',
  },
};

// Individual group section component
function GroupSection({
  group,
  groupBy,
  viewMode,
  isExpanded,
  onToggle,
  onGroupClick,
  renderEntry,
  index,
}: {
  group: EntryGroup;
  groupBy: GroupByDimension;
  viewMode: ViewMode;
  isExpanded: boolean;
  onToggle: () => void;
  onGroupClick?: (groupKey: string) => void;
  renderEntry: (entry: Entry, viewMode: ViewMode) => React.ReactNode;
  index: number;
}) {
  const styles = GROUP_STYLES[groupBy];

  // Calculate animation delay based on index (staggered reveal)
  const animationDelay = `${Math.min(index * 50, 300)}ms`;

  return (
    <div
      className={clsx(
        'rounded-xl border overflow-hidden transition-all duration-300',
        'bg-slate-900/40 backdrop-blur-sm',
        isExpanded ? 'border-slate-700/60' : 'border-slate-800/40',
        'hover:border-slate-700/80',
        'animate-in fade-in slide-in-from-bottom-2'
      )}
      style={{ animationDelay, animationFillMode: 'backwards' }}
    >
      {/* Group Header */}
      <button
        onClick={onToggle}
        className={clsx(
          'w-full flex items-center gap-3 px-4 py-3 transition-all duration-200',
          'hover:bg-slate-800/30',
          isExpanded && `bg-gradient-to-r ${styles.gradient}`
        )}
      >
        {/* Expand/Collapse Icon */}
        <div className={clsx(
          'flex-shrink-0 w-6 h-6 rounded-md flex items-center justify-center transition-all duration-200',
          isExpanded ? 'bg-slate-800/60' : 'bg-slate-800/40'
        )}>
          {isExpanded ? (
            <ChevronDown className={clsx('w-4 h-4 transition-transform', styles.accent)} />
          ) : (
            <ChevronRight className={clsx('w-4 h-4 transition-transform', 'text-slate-500')} />
          )}
        </div>

        {/* Group Icon */}
        <div className={clsx(
          'flex-shrink-0 w-8 h-8 rounded-lg flex items-center justify-center',
          isExpanded ? styles.badge.split(' ')[0] : 'bg-slate-800/60',
          'transition-all duration-200'
        )}>
          <span className={isExpanded ? styles.accent : 'text-slate-500'}>
            {styles.icon}
          </span>
        </div>

        {/* Group Label */}
        <div className="flex-1 min-w-0 text-left">
          <h3 className={clsx(
            'font-medium truncate transition-colors duration-200',
            isExpanded ? 'text-white' : 'text-slate-300'
          )}>
            {group.label}
          </h3>
        </div>

        {/* Entry Count Badge */}
        <div className={clsx(
          'flex-shrink-0 px-2.5 py-1 rounded-full text-xs font-medium border transition-all duration-200',
          isExpanded ? styles.badge : 'bg-slate-800/60 text-slate-500 border-slate-700/50'
        )}>
          {group.count} {group.count === 1 ? 'entry' : 'entries'}
        </div>

        {/* Filter action button */}
        {onGroupClick && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onGroupClick(group.key);
            }}
            className={clsx(
              'flex-shrink-0 px-2 py-1 rounded-md text-xs font-medium transition-all duration-200',
              'text-slate-500 hover:text-cyan-400 hover:bg-cyan-500/10',
              'opacity-0 group-hover:opacity-100'
            )}
            title={`Filter to ${group.label}`}
          >
            Filter
          </button>
        )}
      </button>

      {/* Group Content */}
      <div
        className={clsx(
          'overflow-hidden transition-all duration-300 ease-in-out',
          isExpanded ? 'max-h-[2000px] opacity-100' : 'max-h-0 opacity-0'
        )}
      >
        <div className="p-4 pt-2 border-t border-slate-800/50">
          {viewMode === 'grid' ? (
            <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
              {group.entries.map((entry) => (
                <div key={entry.id} className="animate-in fade-in duration-200">
                  {renderEntry(entry, viewMode)}
                </div>
              ))}
            </div>
          ) : viewMode === 'list' ? (
            <div className="space-y-3">
              {group.entries.map((entry) => (
                <div key={entry.id} className="animate-in fade-in duration-200">
                  {renderEntry(entry, viewMode)}
                </div>
              ))}
            </div>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
              {group.entries.map((entry) => (
                <div key={entry.id} className="animate-in fade-in duration-200">
                  {renderEntry(entry, viewMode)}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export function GroupedEntryView({
  groups,
  groupBy,
  viewMode,
  onGroupClick,
  renderEntry,
}: GroupedEntryViewProps) {
  // Track expanded state for each group
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(() => {
    // Initially expand the first 3 groups
    return new Set(groups.slice(0, 3).map(g => g.key));
  });

  const toggleGroup = useCallback((key: string) => {
    setExpandedGroups(prev => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }, []);

  const expandAll = useCallback(() => {
    setExpandedGroups(new Set(groups.map(g => g.key)));
  }, [groups]);

  const collapseAll = useCallback(() => {
    setExpandedGroups(new Set());
  }, []);

  const allExpanded = expandedGroups.size === groups.length;
  const noneExpanded = expandedGroups.size === 0;

  // Calculate total entries
  const totalEntries = useMemo(() =>
    groups.reduce((sum, g) => sum + g.count, 0),
    [groups]
  );

  const styles = GROUP_STYLES[groupBy];

  if (groups.length === 0) {
    return (
      <div className="bg-slate-900/40 rounded-xl border border-slate-800/60 p-12 text-center">
        <FolderOpen className="w-12 h-12 text-slate-700 mx-auto mb-4" />
        <p className="text-slate-400 text-lg">No groups to display</p>
        <p className="text-slate-600 text-sm mt-1">Try adjusting your filters</p>
      </div>
    );
  }

  // Single group (no grouping or 'none' selected) - just render entries directly
  if (groupBy === 'none' || groups.length === 1) {
    const allEntries = groups.flatMap(g => g.entries);
    return (
      <div>
        {viewMode === 'grid' ? (
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
            {allEntries.map((entry) => (
              <div key={entry.id}>{renderEntry(entry, viewMode)}</div>
            ))}
          </div>
        ) : viewMode === 'list' ? (
          <div className="space-y-3">
            {allEntries.map((entry) => (
              <div key={entry.id}>{renderEntry(entry, viewMode)}</div>
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
            {allEntries.map((entry) => (
              <div key={entry.id}>{renderEntry(entry, viewMode)}</div>
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Group Summary Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className={clsx(
            'flex items-center gap-2 px-3 py-1.5 rounded-lg',
            styles.badge.split(' ')[0],
            'border',
            styles.border
          )}>
            <span className={styles.accent}>{styles.icon}</span>
            <span className={clsx('text-sm font-medium', styles.accent)}>
              {groups.length} {groups.length === 1 ? 'group' : 'groups'}
            </span>
          </div>
          <span className="text-sm text-slate-500">
            {totalEntries} {totalEntries === 1 ? 'entry' : 'entries'} total
          </span>
        </div>

        {/* Expand/Collapse All */}
        <div className="flex items-center gap-2">
          <button
            onClick={collapseAll}
            disabled={noneExpanded}
            className={clsx(
              'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all',
              noneExpanded
                ? 'text-slate-600 cursor-not-allowed'
                : 'text-slate-400 hover:text-white hover:bg-slate-800/60'
            )}
          >
            <ChevronUp className="w-3.5 h-3.5" />
            Collapse
          </button>
          <button
            onClick={expandAll}
            disabled={allExpanded}
            className={clsx(
              'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all',
              allExpanded
                ? 'text-slate-600 cursor-not-allowed'
                : 'text-slate-400 hover:text-white hover:bg-slate-800/60'
            )}
          >
            <ChevronDown className="w-3.5 h-3.5" />
            Expand
          </button>
        </div>
      </div>

      {/* Group Sections */}
      <div className="space-y-3">
        {groups.map((group, index) => (
          <GroupSection
            key={group.key}
            group={group}
            groupBy={groupBy}
            viewMode={viewMode}
            isExpanded={expandedGroups.has(group.key)}
            onToggle={() => toggleGroup(group.key)}
            onGroupClick={onGroupClick}
            renderEntry={renderEntry}
            index={index}
          />
        ))}
      </div>
    </div>
  );
}

export default GroupedEntryView;
