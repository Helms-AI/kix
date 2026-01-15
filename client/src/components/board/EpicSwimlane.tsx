import { useState } from 'react';
import type { WorkItem, BoardColumn, BoardColumnInfo, EpicStats } from '../../types/project';
import { BoardColumn as BoardColumnComponent } from './BoardColumn';
import type { HierarchyInfo } from './KanbanBoard';

interface EpicSwimlaneProps {
  /** The Epic work item (null for unassigned swimlane) */
  epic: WorkItem | null;
  /** Column definitions */
  columns: BoardColumnInfo[];
  /** Items organized by column */
  itemsByColumn: Record<BoardColumn, WorkItem[]>;
  /** Statistics for the swimlane */
  stats: EpicStats;
  /** Hierarchy info for parent/child relationships */
  hierarchyInfo?: HierarchyInfo;
  /** Whether this is the "unassigned" swimlane */
  isUnassigned?: boolean;
  /** Callback when a card is clicked */
  onCardClick?: (item: WorkItem) => void;
  /** Callback when add card button is clicked */
  onAddCard?: (column: BoardColumn) => void;
  /** Whether to expand by default */
  defaultExpanded?: boolean;
}

/**
 * EpicSwimlane - A horizontal swimlane for an Epic or unassigned items.
 *
 * Each Epic becomes a swimlane container with:
 * - Epic name + description always visible
 * - Collapsible columns containing descendant items (Stories, Tasks, Bugs, Subtasks)
 * - Color bar on left edge using epic_color
 *
 * The "unassigned" swimlane uses special styling (dashed border) for orphan items.
 */
export function EpicSwimlane({
  epic,
  columns,
  itemsByColumn,
  stats,
  hierarchyInfo,
  isUnassigned = false,
  onCardClick,
  onAddCard,
  defaultExpanded = true,
}: EpicSwimlaneProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);

  // Count total items in this swimlane
  const totalItems = stats.total_items;

  // Format story points
  const storyPointsDisplay = stats.total_story_points > 0
    ? `${stats.total_story_points} pts`
    : null;

  // Generate column summary text (e.g., "3 backlog, 2 in progress, 1 done")
  const columnSummary = columns
    .map((col) => {
      const count = stats.items_by_column[col.id as BoardColumn] || 0;
      if (count === 0) return null;
      return `${count} ${col.display_name.toLowerCase()}`;
    })
    .filter(Boolean)
    .join(' \u2022 '); // bullet separator

  // Truncate description for collapsed view (first ~100 chars)
  const truncateDescription = (text: string, maxLength: number = 100) => {
    if (text.length <= maxLength) return text;
    return text.substring(0, maxLength).trim() + '...';
  };

  const epicDescription = epic?.body || '';
  const truncatedDescription = truncateDescription(epicDescription);

  // Epic color for the left border (default to purple for unassigned)
  const epicColor = epic?.epic_color ? `#${epic.epic_color}` : (isUnassigned ? '#475569' : '#a855f7');

  return (
    <div
      className={`
        border-b last:border-b-0
        ${isUnassigned
          ? 'border-dashed border-slate-700/50'
          : 'border-slate-800/50'
        }
      `}
    >
      {/* Color Bar + Header */}
      <div className="relative">
        {/* Epic Color Bar */}
        <div
          className="absolute left-0 top-0 bottom-0 w-1"
          style={{ backgroundColor: epicColor }}
        />

        {/* Swimlane Header (clickable to expand/collapse) */}
        <button
          onClick={() => setIsExpanded(!isExpanded)}
          className="w-full flex flex-col items-start gap-1 pl-4 pr-4 py-3 hover:bg-slate-800/30 transition-colors text-left"
        >
          {/* Top Row: Expand/Collapse + Epic Name + Stats */}
          <div className="w-full flex items-center gap-3">
            {/* Expand/Collapse Icon */}
            <svg
              className={`w-4 h-4 text-slate-500 transition-transform flex-shrink-0 ${
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

            {/* Epic Icon */}
            {isUnassigned ? (
              <span className="text-slate-500 flex-shrink-0">
                <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </span>
            ) : (
              <span className="text-purple-400 flex-shrink-0">
                <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
              </span>
            )}

            {/* Epic Title */}
            <h3 className={`text-sm font-semibold truncate max-w-[400px] ${
              isUnassigned ? 'text-slate-400' : 'text-slate-200'
            }`}>
              {isUnassigned ? 'Unassigned' : epic?.title}
            </h3>

            {/* Stats Badge */}
            <div className="flex items-center gap-2 text-xs text-slate-500 flex-shrink-0">
              <span className="bg-slate-800/50 px-2 py-0.5 rounded">
                {totalItems} {totalItems === 1 ? 'item' : 'items'}
              </span>
              {storyPointsDisplay && (
                <span className="bg-slate-800/50 px-2 py-0.5 rounded">
                  {storyPointsDisplay}
                </span>
              )}
            </div>

            {/* Spacer */}
            <div className="flex-1" />

            {/* Menu Button (placeholder) */}
            <button
              onClick={(e) => {
                e.stopPropagation();
                // TODO: Epic actions menu
              }}
              className="p-1 text-slate-600 hover:text-slate-400 rounded transition-colors"
            >
              <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 5v.01M12 12v.01M12 19v.01M12 6a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2z" />
              </svg>
            </button>
          </div>

          {/* Description Row (always visible) */}
          {(epicDescription || isUnassigned) && (
            <div className={`w-full pl-7 text-sm ${
              isUnassigned ? 'text-slate-600' : 'text-slate-400'
            }`}>
              {isUnassigned ? (
                'Items not assigned to an Epic - drag to organize'
              ) : (
                isExpanded ? null : truncatedDescription
              )}
            </div>
          )}

          {/* Column Summary Row (collapsed view only) */}
          {!isExpanded && columnSummary && (
            <div className="w-full pl-7 text-xs text-slate-600">
              {columnSummary}
            </div>
          )}
        </button>
      </div>

      {/* Expanded Content */}
      {isExpanded && (
        <>
          {/* Full Description (expanded view) */}
          {epicDescription && !isUnassigned && (
            <div className="px-4 pl-8 pb-3">
              <div className="p-3 bg-slate-800/30 rounded-lg border border-slate-700/30">
                <p className="text-sm text-slate-300 whitespace-pre-wrap">
                  {epicDescription}
                </p>
              </div>
            </div>
          )}

          {/* Columns */}
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
        </>
      )}

      {/* Collapsed Empty State */}
      {!isExpanded && totalItems === 0 && (
        <div className="px-12 pb-3 text-sm text-slate-600">
          {isUnassigned
            ? 'No unassigned items'
            : 'Click to expand and view Epic contents'
          }
        </div>
      )}
    </div>
  );
}

export default EpicSwimlane;
