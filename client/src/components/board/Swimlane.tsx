import { useState } from 'react';
import type { WorkItem, BoardColumn, WorkItemType, BoardColumnInfo } from '../../types/project';
import { getWorkItemTypeColor, getWorkItemTypeIcon } from '../../api/projectClient';
import { BoardColumn as BoardColumnComponent } from './BoardColumn';
import type { HierarchyInfo } from './KanbanBoard';

interface SwimlaneProps {
  itemType: WorkItemType;
  columns: BoardColumnInfo[];
  itemsByColumn: Record<BoardColumn, WorkItem[]>;
  hierarchyInfo?: HierarchyInfo;
  onCardClick?: (item: WorkItem) => void;
  onAddCard?: (column: BoardColumn) => void;
  defaultExpanded?: boolean;
}

export function Swimlane({
  itemType,
  columns,
  itemsByColumn,
  hierarchyInfo,
  onCardClick,
  onAddCard,
  defaultExpanded = true,
}: SwimlaneProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const typeClass = getWorkItemTypeColor(itemType);
  const typeIcon = getWorkItemTypeIcon(itemType);

  // Count total items in this swimlane
  const totalItems = Object.values(itemsByColumn).reduce(
    (sum, items) => sum + items.length,
    0
  );

  // Get item type label
  const typeLabel = itemType.charAt(0).toUpperCase() + itemType.slice(1);

  return (
    <div className="border-b border-slate-800/50 last:border-b-0">
      {/* Swimlane Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center gap-3 px-4 py-3 hover:bg-slate-800/30 transition-colors"
      >
        {/* Expand/Collapse Icon */}
        <svg
          className={`w-4 h-4 text-slate-500 transition-transform ${
            isExpanded ? 'rotate-90' : ''
          }`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M9 5l7 7-7 7"
          />
        </svg>

        {/* Work Item Type Badge */}
        <span className={`text-sm px-2 py-0.5 rounded border ${typeClass}`}>
          {typeIcon} {typeLabel}
        </span>

        {/* Item Count */}
        <span className="text-sm text-slate-500">
          {totalItems} {totalItems === 1 ? 'item' : 'items'}
        </span>

        {/* Spacer */}
        <div className="flex-1" />

        {/* Quick stats - items per column */}
        <div className="hidden sm:flex items-center gap-2 text-xs text-slate-600">
          {columns.map((col) => {
            const count = itemsByColumn[col.id as BoardColumn]?.length || 0;
            if (count === 0) return null;
            return (
              <span key={col.id} className="flex items-center gap-1">
                <span className="text-slate-500">{col.display_name}:</span>
                <span className="text-slate-400">{count}</span>
              </span>
            );
          })}
        </div>
      </button>

      {/* Columns */}
      {isExpanded && (
        <div className="flex gap-4 px-4 pb-4 overflow-x-auto">
          {columns.map((column) => (
            <BoardColumnComponent
              key={column.id}
              id={column.id as BoardColumn}
              displayName={column.display_name}
              items={itemsByColumn[column.id as BoardColumn] || []}
              hierarchyInfo={hierarchyInfo}
              onCardClick={onCardClick}
              onAddCard={onAddCard}
            />
          ))}
        </div>
      )}

      {/* Collapsed Summary */}
      {!isExpanded && totalItems > 0 && (
        <div className="px-12 pb-3 text-sm text-slate-600">
          Click to expand and view {totalItems} {itemType}{totalItems !== 1 ? 's' : ''}
        </div>
      )}
    </div>
  );
}

export default Swimlane;
