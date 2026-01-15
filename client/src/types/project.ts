// Project Management Types

export type WorkItemState = 'open' | 'closed' | 'in_progress' | 'blocked';
export type WorkItemPriority = 'low' | 'medium' | 'high' | 'critical';

// Board-related types
export type WorkItemType = 'epic' | 'story' | 'task' | 'subtask' | 'bug';
export type BoardColumn = 'backlog' | 'todo' | 'in_progress' | 'in_review' | 'testing' | 'done';

export interface ProjectStats {
  total_items: number;
  open_items: number;
  closed_items: number;
  linked_entries: number;
}

// Project as returned by the API (flat structure)
export interface Project {
  id: string;
  name: string;
  slug: string;
  description?: string;
  color?: string;
  stats?: ProjectStats;
  archived: boolean;
  created_at: string;
  updated_at: string;
}

export interface WorkItem {
  id: string;
  project_id: string;
  number: number;
  title: string;
  body?: string;
  state: WorkItemState;
  labels: string[];
  priority?: WorkItemPriority;
  assignees: string[];
  created_at: string;
  updated_at: string;
  closed_at?: string;
  // Board fields
  item_type: WorkItemType;
  parent_id?: string;
  position: number;
  board_column: BoardColumn;
  story_points?: number;
  epic_color?: string;
}

export interface ProjectEntry {
  id: string;
  project_id: string;
  entry_id: string;
  entry_type: string;
  title: string;
  source_url?: string;
  relevance?: number;
  notes?: string;
  linked_at: string;
}

// API Request/Response Types
export interface CreateProjectRequest {
  name: string;
  description?: string;
  color?: string;
}

export interface UpdateProjectRequest {
  name?: string;
  description?: string;
  color?: string;
  archived?: boolean;
}

export interface CreateWorkItemRequest {
  title: string;
  body?: string;
  state?: WorkItemState;
  labels?: string[];
  priority?: WorkItemPriority;
  assignees?: string[];
  item_type?: WorkItemType;
  parent_id?: string;
  board_column?: BoardColumn;
  story_points?: number;
  epic_color?: string;
}

export interface UpdateWorkItemRequest {
  title?: string;
  body?: string;
  state?: WorkItemState;
  labels?: string[];
  priority?: WorkItemPriority;
  assignees?: string[];
  item_type?: WorkItemType;
  parent_id?: string;
  board_column?: BoardColumn;
  story_points?: number;
  epic_color?: string;
}

export interface LinkEntryRequest {
  entry_id: string;
  relevance?: number;
  notes?: string;
}

export interface ProjectListResponse {
  projects: Project[];
  total: number;
}

export interface WorkItemListResponse {
  items: WorkItem[];
  total: number;
}

export interface ProjectEntryListResponse {
  entries: ProjectEntry[];
  total: number;
}

// Project Event Types (for SSE)
export type ProjectEventType =
  | 'project_created'
  | 'project_updated'
  | 'project_deleted'
  | 'work_item_created'
  | 'work_item_updated'
  | 'work_item_deleted'
  | 'entry_linked'
  | 'entry_unlinked'
  | 'heartbeat';

export interface ProjectEvent {
  event_type: ProjectEventType;
  project_id?: string;
  timestamp: string;
  data: {
    // Common fields
    id?: string;
    name?: string;
    title?: string;
    // Entry fields
    entry_id?: string;
    entry_title?: string;
  };
}

// ============================================================================
// Board API Types
// ============================================================================

export interface BoardColumnInfo {
  id: string;
  name: string;
  display_name: string;
}

export interface BoardResponse {
  columns: BoardColumnInfo[];
  swimlanes: WorkItemType[];
  items_by_swimlane: Record<WorkItemType, Record<BoardColumn, WorkItem[]>>;
  column_counts: Record<BoardColumn, number>;
  total_items: number;
}

export interface MoveCardRequest {
  item_id: string;
  to_column: BoardColumn;
  to_position: number;
}

export interface MoveCardResponse {
  success: boolean;
  item_id: string;
  from_column: BoardColumn;
  to_column: BoardColumn;
  to_position: number;
}

export interface ColumnCountsResponse {
  columns: Record<BoardColumn, number>;
  total: number;
}

export interface ChildWorkItemsResponse {
  parent_id: string;
  parent_type: WorkItemType;
  children: WorkItem[];
  total: number;
}

// Board column display configuration
export const BOARD_COLUMNS: BoardColumnInfo[] = [
  { id: 'backlog', name: 'backlog', display_name: 'Backlog' },
  { id: 'todo', name: 'todo', display_name: 'To Do' },
  { id: 'in_progress', name: 'in_progress', display_name: 'In Progress' },
  { id: 'in_review', name: 'in_review', display_name: 'In Review' },
  { id: 'testing', name: 'testing', display_name: 'Testing' },
  { id: 'done', name: 'done', display_name: 'Done' },
];

// Work item type display configuration
export const WORK_ITEM_TYPES: { type: WorkItemType; label: string; color: string }[] = [
  { type: 'epic', label: 'Epic', color: 'purple' },
  { type: 'story', label: 'Story', color: 'blue' },
  { type: 'task', label: 'Task', color: 'slate' },
  { type: 'subtask', label: 'Subtask', color: 'gray' },
  { type: 'bug', label: 'Bug', color: 'red' },
];
