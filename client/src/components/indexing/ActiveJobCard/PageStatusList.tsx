import { useMemo, memo, useState, useCallback, type ReactElement } from 'react';
import { List, type RowComponentProps } from 'react-window';
import { Clock, Loader2, CheckCircle, XCircle, Trash2, Download } from 'lucide-react';
import { PageStatusRow } from './PageStatusRow';
import type { PageState, PageStatus } from '../../../api/indexingClient';

interface PageStatusListProps {
  pages: Record<string, PageState>;
  maxHeight?: number;
  onClear?: () => void;
}

interface StatusCounts {
  running: number;
  pending: number;
  completed: number;
  error: number;
}

// Status priority for sorting (lower = higher priority / shown first)
const statusPriority: Record<PageStatus, number> = {
  running: 0,
  pending: 1,
  completed: 2,
  error: 3,
};

function sortPages(pages: PageState[]): PageState[] {
  return [...pages].sort((a, b) => {
    // First sort by status priority
    const priorityDiff = statusPriority[a.status] - statusPriority[b.status];
    if (priorityDiff !== 0) return priorityDiff;

    // Within same status, sort by discovery time (most recent first for completed/error)
    if (a.status === 'completed' || a.status === 'error') {
      return (b.completedAt || '').localeCompare(a.completedAt || '');
    }

    // For running/pending, show most recently started/discovered first
    if (a.status === 'running') {
      return (b.startedAt || '').localeCompare(a.startedAt || '');
    }

    return (b.discoveredAt || '').localeCompare(a.discoveredAt || '');
  });
}

function countByStatus(pages: PageState[]): StatusCounts {
  return pages.reduce(
    (acc, page) => {
      acc[page.status]++;
      return acc;
    },
    { running: 0, pending: 0, completed: 0, error: 0 }
  );
}

const ROW_HEIGHT = 36;

// Custom row props containing the sorted pages array
interface PageRowProps {
  pages: PageState[];
}

// Row component for react-window
const PageRow = ({
  index,
  style,
  pages,
}: RowComponentProps<PageRowProps>): ReactElement | null => {
  const page = pages[index];
  if (!page) return null;
  return <PageStatusRow page={page} style={style} />;
};

export const PageStatusList = memo(function PageStatusList({
  pages,
  maxHeight = 300,
  onClear,
}: PageStatusListProps) {
  // Convert pages Record to sorted array, filtering out invalid entries
  const sortedPages = useMemo(() => {
    const pageArray = Object.values(pages).filter(page => page && page.url);
    return sortPages(pageArray);
  }, [pages]);

  // Count by status
  const counts = useMemo(() => countByStatus(sortedPages), [sortedPages]);

  const isEmpty = sortedPages.length === 0;

  // Export functionality with clipboard feedback
  const [exportTooltip, setExportTooltip] = useState('Export page status');

  const handleExport = useCallback(async () => {
    const exportText = sortedPages
      .map((page) => `${page.status} | ${page.url}`)
      .join('\n');

    try {
      await navigator.clipboard.writeText(exportText);
      setExportTooltip('Copied!');
      setTimeout(() => setExportTooltip('Export page status'), 2000);
    } catch {
      // Fallback for older browsers
      const textarea = document.createElement('textarea');
      textarea.value = exportText;
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
      setExportTooltip('Copied!');
      setTimeout(() => setExportTooltip('Export page status'), 2000);
    }
  }, [sortedPages]);

  return (
    <div className="rounded-lg border border-slate-700/60 bg-slate-900/50 overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-slate-700/40 bg-slate-800/30">
        <div className="flex items-center gap-2">
          <span className="text-[10px] font-semibold tracking-wider text-slate-500 uppercase">
            Page Status
          </span>

          {/* Status count badges */}
          <div className="flex items-center gap-2 ml-2">
            {counts.running > 0 && (
              <StatusBadge
                icon={<Loader2 className="w-2.5 h-2.5 animate-spin" />}
                count={counts.running}
                colorClass="text-blue-400"
              />
            )}
            {counts.pending > 0 && (
              <StatusBadge
                icon={<Clock className="w-2.5 h-2.5" />}
                count={counts.pending}
                colorClass="text-yellow-500"
              />
            )}
            {counts.completed > 0 && (
              <StatusBadge
                icon={<CheckCircle className="w-2.5 h-2.5" />}
                count={counts.completed}
                colorClass="text-green-500"
              />
            )}
            {counts.error > 0 && (
              <StatusBadge
                icon={<XCircle className="w-2.5 h-2.5" />}
                count={counts.error}
                colorClass="text-red-500"
              />
            )}
          </div>
        </div>

        {/* Action buttons */}
        {!isEmpty && (
          <div className="flex items-center gap-1">
            {/* Export button */}
            <button
              onClick={handleExport}
              className="p-1 rounded text-slate-500 hover:text-slate-300 hover:bg-slate-700/50 transition-colors"
              title={exportTooltip}
            >
              <Download className="w-3.5 h-3.5" />
            </button>

            {/* Clear button */}
            {onClear && (
              <button
                onClick={onClear}
                className="p-1 rounded text-slate-500 hover:text-slate-300 hover:bg-slate-700/50 transition-colors"
                title="Clear list"
              >
                <Trash2 className="w-3.5 h-3.5" />
              </button>
            )}
          </div>
        )}
      </div>

      {/* Virtualized list */}
      {isEmpty ? (
        <div className="flex items-center justify-center h-20 text-slate-500 text-xs">
          No pages discovered yet
        </div>
      ) : (
        <List<PageRowProps>
          rowComponent={PageRow}
          rowCount={sortedPages.length}
          rowHeight={ROW_HEIGHT}
          rowProps={{ pages: sortedPages }}
          defaultHeight={Math.min(sortedPages.length * ROW_HEIGHT, maxHeight)}
          className="scrollbar-thin scrollbar-thumb-slate-700 scrollbar-track-transparent"
          style={{ maxHeight }}
        />
      )}

      {/* Footer with total count */}
      {!isEmpty && sortedPages.length > maxHeight / ROW_HEIGHT && (
        <div className="px-3 py-1.5 border-t border-slate-700/40 bg-slate-800/20">
          <span className="text-[10px] text-slate-500">
            {sortedPages.length} pages total
          </span>
        </div>
      )}
    </div>
  );
});

// Helper component for status badges
interface StatusBadgeProps {
  icon: React.ReactNode;
  count: number;
  colorClass: string;
}

function StatusBadge({ icon, count, colorClass }: StatusBadgeProps) {
  return (
    <span className={`flex items-center gap-1 text-[10px] ${colorClass}`}>
      {icon}
      <span className="tabular-nums">{count}</span>
    </span>
  );
}
