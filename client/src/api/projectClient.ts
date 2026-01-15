import type {
  Project,
  WorkItem,
  ProjectEntry,
  ProjectListResponse,
  WorkItemListResponse,
  ProjectEntryListResponse,
  CreateProjectRequest,
  UpdateProjectRequest,
  CreateWorkItemRequest,
  UpdateWorkItemRequest,
  LinkEntryRequest,
  // Board types
  WorkItemType,
  BoardResponse,
  MoveCardRequest,
  MoveCardResponse,
  ColumnCountsResponse,
  ChildWorkItemsResponse,
  // Epic-based board types
  EpicBoardResponse,
  ReorderEpicRequest,
  ReorderEpicResponse,
} from '../types/project';

const API_BASE = '/api/projects';

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url);
  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(errorBody || `HTTP error! status: ${response.status}`);
  }
  return response.json();
}

async function postJson<T>(url: string, body: unknown): Promise<T> {
  const response = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(errorBody || `HTTP error! status: ${response.status}`);
  }
  return response.json();
}

async function putJson<T>(url: string, body: unknown): Promise<T> {
  const response = await fetch(url, {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(errorBody || `HTTP error! status: ${response.status}`);
  }
  return response.json();
}

async function deleteRequest<T>(url: string): Promise<T> {
  const response = await fetch(url, {
    method: 'DELETE',
  });
  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(errorBody || `HTTP error! status: ${response.status}`);
  }
  return response.json();
}

// Work item filter params
export interface WorkItemFilters {
  state?: string;
  priority?: string;
  label?: string;
  assignee?: string;
  item_type?: string;
  limit?: number;
  offset?: number;
}

export const projectApi = {
  // ============================================================================
  // Project Operations
  // ============================================================================

  // List all projects
  listProjects: (params?: { archived?: boolean; limit?: number; offset?: number }) => {
    const searchParams = new URLSearchParams();
    if (params?.archived !== undefined) searchParams.set('archived', params.archived.toString());
    if (params?.limit) searchParams.set('limit', params.limit.toString());
    if (params?.offset) searchParams.set('offset', params.offset.toString());
    const query = searchParams.toString();
    return fetchJson<ProjectListResponse>(`${API_BASE}${query ? `?${query}` : ''}`);
  },

  // Get a project by ID
  getProject: (id: string) => fetchJson<Project>(`${API_BASE}/${encodeURIComponent(id)}`),

  // Create a new project
  createProject: (data: CreateProjectRequest) =>
    postJson<Project>(`${API_BASE}`, data),

  // Update a project
  updateProject: (id: string, data: UpdateProjectRequest) =>
    putJson<Project>(`${API_BASE}/${encodeURIComponent(id)}`, data),

  // Delete a project
  deleteProject: (id: string) =>
    deleteRequest<{ status: string }>(`${API_BASE}/${encodeURIComponent(id)}`),

  // ============================================================================
  // Work Item Operations
  // ============================================================================

  // List work items for a project
  listWorkItems: (projectId: string, filters?: WorkItemFilters) => {
    const searchParams = new URLSearchParams();
    if (filters?.state) searchParams.set('state', filters.state);
    if (filters?.priority) searchParams.set('priority', filters.priority);
    if (filters?.label) searchParams.set('label', filters.label);
    if (filters?.assignee) searchParams.set('assignee', filters.assignee);
    if (filters?.item_type) searchParams.set('item_type', filters.item_type);
    if (filters?.limit) searchParams.set('limit', filters.limit.toString());
    if (filters?.offset) searchParams.set('offset', filters.offset.toString());
    const query = searchParams.toString();
    return fetchJson<WorkItemListResponse>(
      `${API_BASE}/${encodeURIComponent(projectId)}/work-items${query ? `?${query}` : ''}`
    );
  },

  // Get a single work item
  getWorkItem: (projectId: string, itemId: string) =>
    fetchJson<WorkItem>(
      `${API_BASE}/${encodeURIComponent(projectId)}/work-items/${encodeURIComponent(itemId)}`
    ),

  // Create a work item
  createWorkItem: (projectId: string, data: CreateWorkItemRequest) =>
    postJson<WorkItem>(`${API_BASE}/${encodeURIComponent(projectId)}/work-items`, data),

  // Update a work item
  updateWorkItem: (projectId: string, itemId: string, data: UpdateWorkItemRequest) =>
    putJson<WorkItem>(
      `${API_BASE}/${encodeURIComponent(projectId)}/work-items/${encodeURIComponent(itemId)}`,
      data
    ),

  // Delete a work item
  deleteWorkItem: (projectId: string, itemId: string) =>
    deleteRequest<{ status: string }>(
      `${API_BASE}/${encodeURIComponent(projectId)}/work-items/${encodeURIComponent(itemId)}`
    ),

  // ============================================================================
  // Knowledge Entry Links
  // ============================================================================

  // List linked entries for a project
  listLinkedEntries: (projectId: string, limit?: number, offset?: number) => {
    const searchParams = new URLSearchParams();
    if (limit) searchParams.set('limit', limit.toString());
    if (offset) searchParams.set('offset', offset.toString());
    const query = searchParams.toString();
    return fetchJson<ProjectEntryListResponse>(
      `${API_BASE}/${encodeURIComponent(projectId)}/entries${query ? `?${query}` : ''}`
    );
  },

  // Link an entry to a project
  linkEntry: (projectId: string, data: LinkEntryRequest) =>
    postJson<ProjectEntry>(`${API_BASE}/${encodeURIComponent(projectId)}/entries`, data),

  // Unlink an entry from a project
  unlinkEntry: (projectId: string, entryId: string) =>
    deleteRequest<{ status: string }>(
      `${API_BASE}/${encodeURIComponent(projectId)}/entries/${encodeURIComponent(entryId)}`
    ),

  // ============================================================================
  // Board Operations
  // ============================================================================

  // Get board data with work items organized by column and swimlane
  getBoard: (projectId: string, itemType?: WorkItemType) => {
    const searchParams = new URLSearchParams();
    if (itemType) searchParams.set('item_type', itemType);
    const query = searchParams.toString();
    return fetchJson<BoardResponse>(
      `${API_BASE}/${encodeURIComponent(projectId)}/board${query ? `?${query}` : ''}`
    );
  },

  // Move a card to a new column and position (drag-drop)
  moveCard: (projectId: string, request: MoveCardRequest) =>
    postJson<MoveCardResponse>(
      `${API_BASE}/${encodeURIComponent(projectId)}/board/move`,
      request
    ),

  // Get work item counts by column
  getColumnCounts: (projectId: string) =>
    fetchJson<ColumnCountsResponse>(
      `${API_BASE}/${encodeURIComponent(projectId)}/board/columns`
    ),

  // Get child work items for a parent
  getChildWorkItems: (projectId: string, parentId: string) =>
    fetchJson<ChildWorkItemsResponse>(
      `${API_BASE}/${encodeURIComponent(projectId)}/work-items/${encodeURIComponent(parentId)}/children`
    ),

  // ============================================================================
  // Epic-Based Board Operations
  // ============================================================================

  /**
   * Get Epic-based board data.
   * Each Epic becomes a swimlane containing its descendant items.
   * Items without an Epic ancestor appear in the "unassigned" swimlane.
   */
  getEpicBoard: (projectId: string) =>
    fetchJson<EpicBoardResponse>(
      `${API_BASE}/${encodeURIComponent(projectId)}/board/epics`
    ),

  /**
   * Reorder an Epic swimlane on the board.
   * Changes the vertical position of the Epic swimlane.
   */
  reorderEpic: (projectId: string, request: ReorderEpicRequest) =>
    postJson<ReorderEpicResponse>(
      `${API_BASE}/${encodeURIComponent(projectId)}/board/reorder-epic`,
      request
    ),
};

// Utility functions
export function getStateColor(state: string): string {
  switch (state) {
    case 'open':
      return 'text-emerald-400 bg-emerald-400/10 border-emerald-400/30';
    case 'in_progress':
      return 'text-blue-400 bg-blue-400/10 border-blue-400/30';
    case 'blocked':
      return 'text-red-400 bg-red-400/10 border-red-400/30';
    case 'closed':
      return 'text-slate-400 bg-slate-400/10 border-slate-400/30';
    default:
      return 'text-slate-400 bg-slate-400/10 border-slate-400/30';
  }
}

export function getPriorityColor(priority?: string): string {
  switch (priority) {
    case 'critical':
      return 'text-red-400 bg-red-400/10 border-red-400/30';
    case 'high':
      return 'text-orange-400 bg-orange-400/10 border-orange-400/30';
    case 'medium':
      return 'text-yellow-400 bg-yellow-400/10 border-yellow-400/30';
    case 'low':
      return 'text-slate-400 bg-slate-400/10 border-slate-400/30';
    default:
      return '';
  }
}

export function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMins < 1) return 'just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

// ============================================================================
// Board Utility Functions
// ============================================================================

export function getWorkItemTypeColor(itemType: string): string {
  switch (itemType) {
    case 'epic':
      return 'text-purple-400 bg-purple-400/10 border-purple-400/30';
    case 'story':
      return 'text-blue-400 bg-blue-400/10 border-blue-400/30';
    case 'task':
      return 'text-slate-400 bg-slate-400/10 border-slate-400/30';
    case 'subtask':
      return 'text-gray-400 bg-gray-400/10 border-gray-400/30';
    case 'bug':
      return 'text-red-400 bg-red-400/10 border-red-400/30';
    default:
      return 'text-slate-400 bg-slate-400/10 border-slate-400/30';
  }
}

export function getBoardColumnColor(column: string): string {
  switch (column) {
    case 'backlog':
      return 'text-slate-400 bg-slate-400/10';
    case 'todo':
      return 'text-blue-400 bg-blue-400/10';
    case 'in_progress':
      return 'text-amber-400 bg-amber-400/10';
    case 'in_review':
      return 'text-purple-400 bg-purple-400/10';
    case 'testing':
      return 'text-orange-400 bg-orange-400/10';
    case 'done':
      return 'text-emerald-400 bg-emerald-400/10';
    default:
      return 'text-slate-400 bg-slate-400/10';
  }
}

export function getWorkItemTypeIcon(itemType: string): string {
  switch (itemType) {
    case 'epic':
      return '⚡'; // Lightning bolt for epic
    case 'story':
      return '📝'; // Memo for story
    case 'task':
      return '✓'; // Checkmark for task
    case 'subtask':
      return '○'; // Circle for subtask
    case 'bug':
      return '🐛'; // Bug for bug
    default:
      return '•'; // Bullet for unknown
  }
}
