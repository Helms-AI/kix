import { useDroppable } from '@dnd-kit/core';
import {
  SortableContext,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import type { WorkItem, BoardColumn as BoardColumnType } from '../../types/project';
import { getBoardColumnColor } from '../../api/projectClient';
import { BoardCard } from './BoardCard';
import type { HierarchyInfo } from './KanbanBoard';

interface BoardColumnProps {
  id: BoardColumnType;
  displayName: string;
  items: WorkItem[];
  hierarchyInfo?: HierarchyInfo;
  onCardClick?: (item: WorkItem) => void;
  onAddCard?: (column: BoardColumnType) => void;
}

export function BoardColumn({
  id,
  displayName,
  items,
  hierarchyInfo,
  onCardClick,
  onAddCard,
}: BoardColumnProps) {
  const { setNodeRef, isOver } = useDroppable({ id });
  const colorClass = getBoardColumnColor(id);

  return (
    <div
      className={`
        flex flex-col w-72 min-w-72 bg-slate-900/50 rounded-xl border
        ${isOver ? 'border-blue-500/50 bg-slate-800/30' : 'border-slate-800/50'}
        transition-colors duration-150
      `}
    >
      {/* Column Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-slate-800/50">
        <div className="flex items-center gap-2">
          <h3 className={`text-sm font-semibold ${colorClass.split(' ')[0]}`}>
            {displayName}
          </h3>
          <span className="text-xs text-slate-500 bg-slate-800/50 px-1.5 py-0.5 rounded">
            {items.length}
          </span>
        </div>
        {/* Add card button */}
        <button
          onClick={() => onAddCard?.(id)}
          className="p-1 text-slate-500 hover:text-slate-300 hover:bg-slate-800/50 rounded transition-colors"
          title={`Add card to ${displayName}`}
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
        </button>
      </div>

      {/* Cards Container */}
      <div
        ref={setNodeRef}
        className={`
          flex-1 p-2 space-y-2 overflow-y-auto min-h-[200px]
          ${isOver ? 'bg-blue-500/5' : ''}
        `}
      >
        <SortableContext
          items={items.map((i) => i.id)}
          strategy={verticalListSortingStrategy}
        >
          {items.map((item) => (
            <BoardCard
              key={item.id}
              item={item}
              childrenCount={hierarchyInfo?.childrenCount.get(item.id) || 0}
              parentInfo={hierarchyInfo?.parentInfo.get(item.id)}
              onClick={onCardClick}
            />
          ))}
        </SortableContext>

        {/* Empty state */}
        {items.length === 0 && (
          <div className="flex items-center justify-center h-24 text-slate-600 text-sm">
            <div className="text-center">
              <svg
                className="w-8 h-8 mx-auto mb-2 text-slate-700"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={1.5}
                  d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
                />
              </svg>
              Drop cards here
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default BoardColumn;
