import type {
  Project,
  Issue,
  ProjectEntry,
  ProjectListResponse,
  IssueListResponse,
  ProjectEntryListResponse,
  CreateProjectRequest,
  UpdateProjectRequest,
  CreateIssueRequest,
  UpdateIssueRequest,
  LinkEntryRequest,
  GitHubSyncResult,
  GitHubTokenStatus,
  SetGlobalTokenResponse,
  GitHubUser,
  GitHubOrgsResponse,
  GitHubReposResponse,
  VerifyAccessResponse,
  ProjectTokenType,
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

// Issue filter params
export interface IssueFilters {
  state?: string;
  priority?: string;
  label?: string;
  assignee?: string;
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
  // Issue Operations
  // ============================================================================

  // List issues for a project
  listIssues: (projectId: string, filters?: IssueFilters) => {
    const searchParams = new URLSearchParams();
    if (filters?.state) searchParams.set('state', filters.state);
    if (filters?.priority) searchParams.set('priority', filters.priority);
    if (filters?.label) searchParams.set('label', filters.label);
    if (filters?.assignee) searchParams.set('assignee', filters.assignee);
    if (filters?.limit) searchParams.set('limit', filters.limit.toString());
    if (filters?.offset) searchParams.set('offset', filters.offset.toString());
    const query = searchParams.toString();
    return fetchJson<IssueListResponse>(
      `${API_BASE}/${encodeURIComponent(projectId)}/issues${query ? `?${query}` : ''}`
    );
  },

  // Get a single issue
  getIssue: (projectId: string, issueId: string) =>
    fetchJson<Issue>(
      `${API_BASE}/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}`
    ),

  // Create an issue
  createIssue: (projectId: string, data: CreateIssueRequest) =>
    postJson<Issue>(`${API_BASE}/${encodeURIComponent(projectId)}/issues`, data),

  // Update an issue
  updateIssue: (projectId: string, issueId: string, data: UpdateIssueRequest) =>
    putJson<Issue>(
      `${API_BASE}/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}`,
      data
    ),

  // Delete an issue
  deleteIssue: (projectId: string, issueId: string) =>
    deleteRequest<{ status: string }>(
      `${API_BASE}/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}`
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
  // GitHub Sync
  // ============================================================================

  // Sync issues with GitHub
  syncGitHub: (projectId: string) =>
    postJson<GitHubSyncResult>(`${API_BASE}/${encodeURIComponent(projectId)}/github/sync`, {}),

  // Set GitHub token for a project
  setGitHubToken: (projectId: string, token: string) =>
    postJson<{ status: string }>(`${API_BASE}/${encodeURIComponent(projectId)}/github/token`, {
      token,
    }),

  // Link/create a GitHub Project V2 board
  linkGitHubProject: (projectId: string, template: string) =>
    postJson<{ success: boolean; github_project_v2_url?: string; warning?: string }>(
      `${API_BASE}/${encodeURIComponent(projectId)}/github/link-project`,
      { template }
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
// GitHub API Client
// ============================================================================

const GITHUB_API_BASE = '/api/github';

export const githubApi = {
  // ============================================================================
  // Global Token Management
  // ============================================================================

  // Check if global token is configured
  getTokenStatus: () =>
    fetchJson<GitHubTokenStatus>(`${GITHUB_API_BASE}/token/status`),

  // Set the global GitHub token
  setGlobalToken: (token: string) =>
    postJson<SetGlobalTokenResponse>(`${GITHUB_API_BASE}/token/global`, { token }),

  // ============================================================================
  // User and Organization Discovery
  // ============================================================================

  // Get authenticated user info
  getUser: (token?: string) => {
    const headers: HeadersInit = {};
    if (token) headers['Authorization'] = `Bearer ${token}`;
    return fetch(`${GITHUB_API_BASE}/user`, { headers })
      .then(async (response) => {
        if (!response.ok) {
          const errorBody = await response.text();
          throw new Error(errorBody || `HTTP error! status: ${response.status}`);
        }
        return response.json() as Promise<GitHubUser>;
      });
  },

  // List user's organizations (includes user as first item)
  listOrgs: (token?: string) => {
    const headers: HeadersInit = {};
    if (token) headers['Authorization'] = `Bearer ${token}`;
    return fetch(`${GITHUB_API_BASE}/orgs`, { headers })
      .then(async (response) => {
        if (!response.ok) {
          const errorBody = await response.text();
          throw new Error(errorBody || `HTTP error! status: ${response.status}`);
        }
        return response.json() as Promise<GitHubOrgsResponse>;
      });
  },

  // ============================================================================
  // Repository Discovery
  // ============================================================================

  // List/search repositories for an owner
  listRepos: (
    owner: string,
    options?: { search?: string; page?: number; per_page?: number; token?: string }
  ) => {
    const searchParams = new URLSearchParams();
    searchParams.set('owner', owner);
    if (options?.search) searchParams.set('search', options.search);
    if (options?.page) searchParams.set('page', options.page.toString());
    if (options?.per_page) searchParams.set('per_page', options.per_page.toString());

    const headers: HeadersInit = {};
    if (options?.token) headers['Authorization'] = `Bearer ${options.token}`;

    return fetch(`${GITHUB_API_BASE}/repos?${searchParams.toString()}`, { headers })
      .then(async (response) => {
        if (!response.ok) {
          const errorBody = await response.text();
          throw new Error(errorBody || `HTTP error! status: ${response.status}`);
        }
        return response.json() as Promise<GitHubReposResponse>;
      });
  },

  // ============================================================================
  // Access Verification
  // ============================================================================

  // Verify token access to a repository
  verifyAccess: (owner: string, repo: string, token?: string) =>
    postJson<VerifyAccessResponse>(`${GITHUB_API_BASE}/verify-access`, {
      owner,
      repo,
      token,
    }),

  // ============================================================================
  // Project Token Management
  // ============================================================================

  // Get project's token configuration type
  getProjectTokenStatus: (projectId: string) =>
    fetchJson<ProjectTokenType>(
      `${API_BASE}/${encodeURIComponent(projectId)}/github/token-status`
    ),
};
