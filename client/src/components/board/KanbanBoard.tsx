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
  WorkItemType,
  BoardResponse,
  MoveCardRequest,
} from '../../types/project';
import { BOARD_COLUMNS, WORK_ITEM_TYPES } from '../../types/project';
import { projectApi } from '../../api/projectClient';
import { Swimlane } from './Swimlane';
import { BoardCard } from './BoardCard';

// Hierarchy info computed from board data
export interface HierarchyInfo {
  childrenCount: Map<string, number>;  // itemId -> count of children
  parentInfo: Map<string, { id: string; title: string; number: number }>; // itemId -> parent info
  itemMap: Map<string, WorkItem>; // itemId -> item (for quick lookup)
}

interface KanbanBoardProps {
  projectId: string;
  boardData: BoardResponse;
  onBoardUpdate: () => void;
  onCardClick?: (item: WorkItem) => void;
  onAddCard?: (column: BoardColumn, itemType?: WorkItemType) => void;
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

    // First pass: build item map and count children
    for (const swimlane of Object.values(boardData.items_by_swimlane)) {
      for (const columnItems of Object.values(swimlane)) {
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

    // Second pass: build parent info map
    for (const swimlane of Object.values(boardData.items_by_swimlane)) {
      for (const columnItems of Object.values(swimlane)) {
        for (const item of columnItems) {
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
  const boardDataWithOptimisticUpdates = useMemo(() => {
    if (optimisticUpdates.size === 0) return boardData;

    // Deep clone the board data
    const updated: BoardResponse = {
      ...boardData,
      items_by_swimlane: {} as BoardResponse['items_by_swimlane'],
    };

    // Clone each swimlane
    for (const [swimlane, columns] of Object.entries(boardData.items_by_swimlane)) {
      updated.items_by_swimlane[swimlane as WorkItemType] = {} as Record<BoardColumn, WorkItem[]>;
      for (const [column, items] of Object.entries(columns)) {
        // Filter out items that have been moved
        updated.items_by_swimlane[swimlane as WorkItemType][column as BoardColumn] = items.filter(
          (item) => {
            const update = optimisticUpdates.get(item.id);
            return !update || update.column === column;
          }
        );
      }
    }

    // Add moved items to their new columns
    for (const [itemId, { column }] of optimisticUpdates) {
      const item = findItem(itemId);
      if (item) {
        const swimlane = item.item_type;
        if (!updated.items_by_swimlane[swimlane]) {
          updated.items_by_swimlane[swimlane] = {} as Record<BoardColumn, WorkItem[]>;
        }
        if (!updated.items_by_swimlane[swimlane][column]) {
          updated.items_by_swimlane[swimlane][column] = [];
        }
        // Add to the beginning of the column
        updated.items_by_swimlane[swimlane][column] = [
          { ...item, board_column: column },
          ...updated.items_by_swimlane[swimlane][column],
        ];
      }
    }

    return updated;
  }, [boardData, optimisticUpdates, findItem]);

  // Filter swimlanes that have items
  const activeSwimlanes = useMemo(() => {
    return WORK_ITEM_TYPES.filter(({ type }) => {
      const swimlane = boardDataWithOptimisticUpdates.items_by_swimlane[type];
      if (!swimlane) return false;
      return Object.values(swimlane).some((items) => items.length > 0);
    });
  }, [boardDataWithOptimisticUpdates]);

  // All swimlanes for showing empty ones too
  const allSwimlanes = WORK_ITEM_TYPES;

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
          {/* Swimlanes */}
          {allSwimlanes.map(({ type }) => {
            const swimlaneData =
              boardDataWithOptimisticUpdates.items_by_swimlane[type] ||
              ({} as Record<BoardColumn, WorkItem[]>);

            // Initialize empty columns
            const itemsByColumn: Record<BoardColumn, WorkItem[]> = {
              backlog: swimlaneData.backlog || [],
              todo: swimlaneData.todo || [],
              in_progress: swimlaneData.in_progress || [],
              in_review: swimlaneData.in_review || [],
              testing: swimlaneData.testing || [],
              done: swimlaneData.done || [],
            };

            const hasItems = Object.values(itemsByColumn).some(
              (items) => items.length > 0
            );

            // Only show swimlanes with items (or all if none have items)
            if (!hasItems && activeSwimlanes.length > 0) return null;

            return (
              <Swimlane
                key={type}
                itemType={type}
                columns={BOARD_COLUMNS}
                itemsByColumn={itemsByColumn}
                hierarchyInfo={hierarchyInfo}
                onCardClick={onCardClick}
                onAddCard={(column) => onAddCard?.(column, type)}
                defaultExpanded={hasItems}
              />
            );
          })}

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
