import type { BoardColumn } from '../../types/project';
import { BOARD_COLUMNS } from '../../types/project';

interface BoardHeaderProps {
  totalItems: number;
  columnCounts: Record<BoardColumn, number>;
  viewMode: 'board' | 'list';
  onViewModeChange: (mode: 'board' | 'list') => void;
  onCreateItem: () => void;
  onRefresh: () => void;
  isLoading?: boolean;
}

export function BoardHeader({
  totalItems,
  columnCounts,
  viewMode,
  onViewModeChange,
  onCreateItem,
  onRefresh,
  isLoading = false,
}: BoardHeaderProps) {
  return (
    <div className="flex flex-col gap-3 px-4 py-3 border-b border-slate-800/50 bg-slate-900/30">
      {/* Top Row - View Toggle & Actions */}
      <div className="flex items-center justify-between">
        {/* Left - Stats & View Toggle */}
        <div className="flex items-center gap-4">
          {/* Item Count */}
          <div className="flex items-center gap-2">
            <span className="text-sm text-slate-500">Total:</span>
            <span className="text-sm font-medium text-slate-300">{totalItems}</span>
          </div>

          {/* View Toggle */}
          <div className="flex items-center bg-slate-800/50 rounded-lg p-0.5">
            <button
              onClick={() => onViewModeChange('board')}
              className={`px-3 py-1.5 text-sm rounded-md transition-colors ${
                viewMode === 'board'
                  ? 'bg-slate-700 text-slate-200'
                  : 'text-slate-500 hover:text-slate-300'
              }`}
            >
              <span className="flex items-center gap-1.5">
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 17V7m0 10a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2h2a2 2 0 012 2m0 10a2 2 0 002 2h2a2 2 0 002-2M9 7a2 2 0 012-2h2a2 2 0 012 2m0 10V7m0 10a2 2 0 002 2h2a2 2 0 002-2V7a2 2 0 00-2-2h-2a2 2 0 00-2 2" />
                </svg>
                Board
              </span>
            </button>
            <button
              onClick={() => onViewModeChange('list')}
              className={`px-3 py-1.5 text-sm rounded-md transition-colors ${
                viewMode === 'list'
                  ? 'bg-slate-700 text-slate-200'
                  : 'text-slate-500 hover:text-slate-300'
              }`}
            >
              <span className="flex items-center gap-1.5">
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 10h16M4 14h16M4 18h16" />
                </svg>
                List
              </span>
            </button>
          </div>
        </div>

        {/* Right - Actions */}
        <div className="flex items-center gap-2">
          {/* Refresh */}
          <button
            onClick={onRefresh}
            disabled={isLoading}
            className="p-2 text-slate-500 hover:text-slate-300 hover:bg-slate-800/50 rounded-lg transition-colors disabled:opacity-50"
            title="Refresh board"
          >
            <svg
              className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`}
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
          </button>

          {/* Create Work Item */}
          <button
            onClick={onCreateItem}
            className="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 text-white rounded-lg transition-colors flex items-center gap-1.5"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
            </svg>
            New Item
          </button>
        </div>
      </div>

      {/* Column Summary */}
      <div className="flex items-center gap-4 pt-2 border-t border-slate-800/30 overflow-x-auto">
        {BOARD_COLUMNS.map((col) => {
          const count = columnCounts[col.id as BoardColumn] || 0;
          return (
            <div key={col.id} className="flex items-center gap-1.5 text-xs shrink-0">
              <span className="text-slate-500">{col.display_name}:</span>
              <span className="text-slate-400 font-medium">{count}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default BoardHeader;
