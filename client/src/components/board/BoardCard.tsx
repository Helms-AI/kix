import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { WorkItem } from '../../types/project';
import { getWorkItemTypeColor, getWorkItemTypeIcon, getPriorityColor } from '../../api/projectClient';

interface ParentInfo {
  id: string;
  title: string;
  number: number;
}

interface BoardCardProps {
  item: WorkItem;
  childrenCount?: number;
  parentInfo?: ParentInfo;
  onClick?: (item: WorkItem) => void;
  isDragging?: boolean;
}

export function BoardCard({ item, childrenCount = 0, parentInfo, onClick, isDragging }: BoardCardProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging: isSortableDragging,
  } = useSortable({ id: item.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging || isSortableDragging ? 0.5 : 1,
  };

  const priorityClass = item.priority ? getPriorityColor(item.priority) : '';
  const typeClass = getWorkItemTypeColor(item.item_type);
  const typeIcon = getWorkItemTypeIcon(item.item_type);

  return (
    <div
      ref={setNodeRef}
      style={style}
      {...attributes}
      {...listeners}
      onClick={() => onClick?.(item)}
      className={`
        group relative bg-slate-800/50 border border-slate-700/50 rounded-lg p-3
        hover:border-slate-600 hover:bg-slate-800/70 cursor-grab active:cursor-grabbing
        transition-all duration-150 ease-out
        ${isDragging || isSortableDragging ? 'shadow-lg shadow-slate-900/50 scale-[1.02] z-50' : ''}
      `}
    >
      {/* Parent Link (if has parent) */}
      {parentInfo && (
        <div className="flex items-center gap-1.5 mb-2 text-xs text-slate-500">
          <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 16l-4-4m0 0l4-4m-4 4h18" />
          </svg>
          <span className="truncate max-w-[180px]" title={parentInfo.title}>
            #{parentInfo.number} {parentInfo.title}
          </span>
        </div>
      )}

      {/* Card Header */}
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          {/* Work Item Type Badge */}
          <span className={`text-xs px-1.5 py-0.5 rounded border ${typeClass}`}>
            {typeIcon} {item.item_type}
          </span>
          {/* Work Item Number */}
          <span className="text-xs text-slate-500">#{item.number}</span>
        </div>
        {/* Priority Badge */}
        {item.priority && (
          <span className={`text-xs px-1.5 py-0.5 rounded border ${priorityClass}`}>
            {item.priority}
          </span>
        )}
      </div>

      {/* Title */}
      <h4 className="text-sm text-slate-200 font-medium line-clamp-2 mb-2">
        {item.title}
      </h4>

      {/* Card Footer */}
      <div className="flex items-center justify-between text-xs text-slate-500">
        {/* Labels */}
        {item.labels && item.labels.length > 0 && (
          <div className="flex items-center gap-1 flex-wrap">
            {item.labels.slice(0, 2).map((label, i) => (
              <span
                key={i}
                className="px-1.5 py-0.5 rounded bg-slate-700/50 text-slate-400"
              >
                {label}
              </span>
            ))}
            {item.labels.length > 2 && (
              <span className="text-slate-500">+{item.labels.length - 2}</span>
            )}
          </div>
        )}

        {/* Right side indicators */}
        <div className="flex items-center gap-2">
          {/* Children Count */}
          {childrenCount > 0 && (
            <span
              className="flex items-center gap-0.5 text-cyan-400"
              title={`${childrenCount} child ${childrenCount === 1 ? 'item' : 'items'}`}
            >
              <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16m-7 6h7" />
              </svg>
              {childrenCount}
            </span>
          )}

          {/* Story Points */}
          {item.story_points && (
            <span className="flex items-center gap-0.5" title="Story Points">
              <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
              </svg>
              {item.story_points}
            </span>
          )}

          {/* Assignees */}
          {item.assignees && item.assignees.length > 0 && (
            <div className="flex -space-x-1">
              {item.assignees.slice(0, 2).map((assignee, i) => (
                <div
                  key={i}
                  className="w-5 h-5 rounded-full bg-slate-600 border border-slate-800 flex items-center justify-center text-[10px] text-slate-300"
                  title={assignee}
                >
                  {assignee.charAt(0).toUpperCase()}
                </div>
              ))}
              {item.assignees.length > 2 && (
                <div className="w-5 h-5 rounded-full bg-slate-700 border border-slate-800 flex items-center justify-center text-[10px] text-slate-400">
                  +{item.assignees.length - 2}
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Epic indicator bar (for stories/tasks under an epic) */}
      {item.epic_color && (
        <div
          className="absolute left-0 top-0 bottom-0 w-1 rounded-l-lg"
          style={{ backgroundColor: `#${item.epic_color}` }}
        />
      )}
    </div>
  );
}

export default BoardCard;
