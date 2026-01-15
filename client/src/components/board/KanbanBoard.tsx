import { useState, useCallback, useMemo } from 'react';
import {
  DndContext,
  DragOverlay,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragStartEvent,
  type DragEndEvent,
  type DragOverEvent,
} from '@dnd-kit/core';
import { sortableKeyboardCoordinates } from '@dnd-kit/sortable';
import type {
  WorkItem,
  BoardColumn,
  EpicBoardResponse,
  EpicSwimlaneData,
  MoveCardRequest,
} from '../../types/project';
import { BOARD_COLUMNS } from '../../types/project';
import { projectApi } from '../../api/projectClient';
import { EpicSwimlane } from './EpicSwimlane';
import { BoardCard } from './BoardCard';

// Hierarchy info computed from board data
export interface HierarchyInfo {
  childrenCount: Map<string, number>;  // itemId -> count of children
  parentInfo: Map<string, { id: string; title: string; number: number }>; // itemId -> parent info
  itemMap: Map<string, WorkItem>; // itemId -> item (for quick lookup)
}

interface KanbanBoardProps {
  projectId: string;
  boardData: EpicBoardResponse;
  onBoardUpdate: () => void;
  onCardClick?: (item: WorkItem) => void;
  onAddCard?: (column: BoardColumn) => void;
}

export function KanbanBoard({
  projectId,
  boardData,
  onBoardUpdate,
  onCardClick,
  onAddCard,
}: KanbanBoardProps) {
  const [activeItem, setActiveItem] = useState<WorkItem | null>(null);
  const [optimisticUpdates, setOptimisticUpdates] = useState<Map<string, { column: BoardColumn; position: number }>>(new Map());
  const [isMoving, setIsMoving] = useState(false);

  // Configure drag sensors
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8, // 8px drag distance before activation
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  // Compute hierarchy info (children counts and parent references)
  const hierarchyInfo = useMemo((): HierarchyInfo => {
    const childrenCount = new Map<string, number>();
    const parentInfo = new Map<string, { id: string; title: string; number: number }>();
    const itemMap = new Map<string, WorkItem>();

    // Process all epics and their items
    for (const epicData of boardData.epics) {
      // Add the epic itself to the item map
      itemMap.set(epicData.epic.id, epicData.epic);

      // Process items in each column
      for (const columnItems of Object.values(epicData.items_by_column)) {
        for (const item of columnItems) {
          itemMap.set(item.id, item);

          // Count children for parent items
          if (item.parent_id) {
            const currentCount = childrenCount.get(item.parent_id) || 0;
            childrenCount.set(item.parent_id, currentCount + 1);
          }
        }
      }
    }

    // Process unassigned items
    for (const columnItems of Object.values(boardData.unassigned)) {
      for (const item of columnItems) {
        itemMap.set(item.id, item);

        // Count children for parent items
        if (item.parent_id) {
          const currentCount = childrenCount.get(item.parent_id) || 0;
          childrenCount.set(item.parent_id, currentCount + 1);
        }
      }
    }

    // Second pass: build parent info map
    for (const [_id, item] of itemMap) {
      if (item.parent_id) {
        const parent = itemMap.get(item.parent_id);
        if (parent) {
          parentInfo.set(item.id, {
            id: parent.id,
            title: parent.title,
            number: parent.number,
          });
        }
      }
    }

    return { childrenCount, parentInfo, itemMap };
  }, [boardData]);

  // Find item by ID across all swimlanes and columns
  const findItem = useCallback(
    (itemId: string): WorkItem | undefined => {
      return hierarchyInfo.itemMap.get(itemId);
    },
    [hierarchyInfo]
  );

  // Handle drag start
  const handleDragStart = (event: DragStartEvent) => {
    const { active } = event;
    const item = findItem(active.id as string);
    if (item) {
      setActiveItem(item);
    }
  };

  // Handle drag over (for preview)
  const handleDragOver = (_event: DragOverEvent) => {
    // Could be used for visual feedback during drag
  };

  // Handle drag end - move the card
  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveItem(null);

    if (!over || isMoving) return;

    const itemId = active.id as string;
    const targetColumn = over.id as BoardColumn;

    // Find the dragged item
    const item = findItem(itemId);
    if (!item) return;

    // If dropping in the same column at the same position, do nothing
    if (item.board_column === targetColumn) {
      return;
    }

    // Optimistic update
    setOptimisticUpdates((prev) => {
      const next = new Map(prev);
      next.set(itemId, { column: targetColumn, position: 0 });
      return next;
    });

    setIsMoving(true);

    try {
      const request: MoveCardRequest = {
        item_id: itemId,
        to_column: targetColumn,
        to_position: 0, // Will be placed at the top of the column
      };

      await projectApi.moveCard(projectId, request);

      // Refresh board data
      onBoardUpdate();
    } catch (error) {
      console.error('Failed to move card:', error);
      // Revert optimistic update
      setOptimisticUpdates((prev) => {
        const next = new Map(prev);
        next.delete(itemId);
        return next;
      });
    } finally {
      setIsMoving(false);
      // Clear optimistic updates after refresh
      setOptimisticUpdates(new Map());
    }
  };

  // Apply optimistic updates to board data
  const boardDataWithOptimisticUpdates = useMemo((): EpicBoardResponse => {
    if (optimisticUpdates.size === 0) return boardData;

    // Deep clone the board data
    const updated: EpicBoardResponse = {
      ...boardData,
      epics: boardData.epics.map((epicData): EpicSwimlaneData => ({
        ...epicData,
        items_by_column: { ...epicData.items_by_column },
      })),
      unassigned: { ...boardData.unassigned },
    };

    // Filter out moved items from their original locations and add to new locations
    for (const [itemId, { column }] of optimisticUpdates) {
      const item = findItem(itemId);
      if (!item) continue;

      const originalColumn = item.board_column;

      // Remove from original location in epics
      for (const epicData of updated.epics) {
        if (epicData.items_by_column[originalColumn]) {
          epicData.items_by_column[originalColumn] = epicData.items_by_column[originalColumn].filter(
            (i) => i.id !== itemId
          );
        }
      }

      // Remove from unassigned
      if (updated.unassigned[originalColumn]) {
        updated.unassigned[originalColumn] = updated.unassigned[originalColumn].filter(
          (i) => i.id !== itemId
        );
      }

      // Find which epic this item belongs to and add to new column
      let foundEpic = false;
      for (const epicData of updated.epics) {
        // Check if this item was in this epic's swimlane
        const wasInThisEpic = boardData.epics
          .find((e) => e.epic.id === epicData.epic.id)
          ?.items_by_column[originalColumn]
          ?.some((i) => i.id === itemId);

        if (wasInThisEpic) {
          if (!epicData.items_by_column[column]) {
            epicData.items_by_column[column] = [];
          }
          epicData.items_by_column[column] = [
            { ...item, board_column: column },
            ...epicData.items_by_column[column],
          ];
          foundEpic = true;
          break;
        }
      }

      // If not in any epic, add to unassigned
      if (!foundEpic) {
        if (!updated.unassigned[column]) {
          updated.unassigned[column] = [];
        }
        updated.unassigned[column] = [
          { ...item, board_column: column },
          ...updated.unassigned[column],
        ];
      }
    }

    return updated;
  }, [boardData, optimisticUpdates, findItem]);

  // Check if there are any unassigned items
  const hasUnassignedItems = useMemo(() => {
    return Object.values(boardDataWithOptimisticUpdates.unassigned).some(
      (items) => items.length > 0
    );
  }, [boardDataWithOptimisticUpdates]);

  // Compute unassigned stats
  const unassignedStats = useMemo(() => {
    const items_by_column: Record<BoardColumn, number> = {} as Record<BoardColumn, number>;
    let total_items = 0;
    let total_story_points = 0;

    for (const [column, items] of Object.entries(boardDataWithOptimisticUpdates.unassigned)) {
      items_by_column[column as BoardColumn] = items.length;
      total_items += items.length;
      total_story_points += items.reduce((sum, item) => sum + (item.story_points || 0), 0);
    }

    return {
      total_items,
      total_story_points,
      items_by_column,
    };
  }, [boardDataWithOptimisticUpdates]);

  // Initialize empty columns for consistent rendering
  const initializeColumns = (items: Record<BoardColumn, WorkItem[]>): Record<BoardColumn, WorkItem[]> => {
    return {
      backlog: items.backlog || [],
      todo: items.todo || [],
      in_progress: items.in_progress || [],
      in_review: items.in_review || [],
      testing: items.testing || [],
      done: items.done || [],
    };
  };

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragStart={handleDragStart}
      onDragOver={handleDragOver}
      onDragEnd={handleDragEnd}
    >
      <div className="flex flex-col h-full">
        {/* Board Content */}
        <div className="flex-1 overflow-y-auto">
          {/* Epic Swimlanes */}
          {boardDataWithOptimisticUpdates.epics.map((epicData) => (
            <EpicSwimlane
              key={epicData.epic.id}
              epic={epicData.epic}
              columns={BOARD_COLUMNS}
              itemsByColumn={initializeColumns(epicData.items_by_column)}
              stats={epicData.stats}
              hierarchyInfo={hierarchyInfo}
              onCardClick={onCardClick}
              onAddCard={onAddCard}
              defaultExpanded={epicData.stats.total_items > 0}
            />
          ))}

          {/* Unassigned Swimlane - only show if there are unassigned items */}
          {hasUnassignedItems && (
            <EpicSwimlane
              epic={null}
              columns={BOARD_COLUMNS}
              itemsByColumn={initializeColumns(boardDataWithOptimisticUpdates.unassigned)}
              stats={unassignedStats}
              hierarchyInfo={hierarchyInfo}
              isUnassigned
              onCardClick={onCardClick}
              onAddCard={onAddCard}
              defaultExpanded={true}
            />
          )}

          {/* Empty Board State */}
          {boardData.total_items === 0 && (
            <div className="flex flex-col items-center justify-center py-16 text-slate-500">
              <svg
                className="w-16 h-16 mb-4 text-slate-700"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={1.5}
                  d="M9 17V7m0 10a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2h2a2 2 0 012 2m0 10a2 2 0 002 2h2a2 2 0 002-2M9 7a2 2 0 012-2h2a2 2 0 012 2m0 10V7m0 10a2 2 0 002 2h2a2 2 0 002-2V7a2 2 0 00-2-2h-2a2 2 0 00-2 2"
                />
              </svg>
              <h3 className="text-lg font-medium text-slate-400 mb-2">
                No work items yet
              </h3>
              <p className="text-sm text-slate-600 mb-4">
                Create your first work item to get started with the board
              </p>
            </div>
          )}
        </div>
      </div>

      {/* Drag Overlay - Shows the card being dragged */}
      <DragOverlay>
        {activeItem ? (
          <div className="opacity-90">
            <BoardCard item={activeItem} isDragging />
          </div>
        ) : null}
      </DragOverlay>
    </DndContext>
  );
}

export default KanbanBoard;
