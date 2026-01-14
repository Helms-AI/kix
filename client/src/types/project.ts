// Project Management Types

export type IssueState = 'open' | 'closed' | 'in_progress' | 'blocked';
export type IssueSource = 'local' | 'github';
export type IssuePriority = 'low' | 'medium' | 'high' | 'critical';

export interface ProjectStats {
  total_issues: number;
  open_issues: number;
  closed_issues: number;
  in_progress_issues: number;
  linked_entries: number;
}

// Project as returned by the API (flat structure)
export interface Project {
  id: string;
  name: string;
  slug: string;
  description?: string;
  color?: string;
  github_owner?: string;
  github_repo?: string;
  has_github: boolean;
  has_token?: boolean;
  /** URL to the linked GitHub Project V2 board (if any) */
  github_project_v2_url?: string;
  stats?: ProjectStats;
  archived: boolean;
  created_at: string;
  updated_at: string;
}

export interface Issue {
  id: string;
  project_id: string;
  number: number;
  title: string;
  body?: string;
  state: IssueState;
  labels: string[];
  priority?: IssuePriority;
  assignees: string[];
  github_number?: number;
  github_url?: string;
  source: IssueSource;
  created_at: string;
  updated_at: string;
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

// Project template types (for GitHub Projects V2)
export type ProjectTemplate = 'kanban' | 'bug_tracking' | 'sprint_planning' | 'feature_roadmap';

// API Request/Response Types
export interface CreateProjectRequest {
  name: string;
  description?: string;
  color?: string;
  // GitHub config (optional for local-only projects)
  github_owner?: string;
  github_repo?: string;
  // GitHub Project V2 template (required for GitHub projects)
  template?: ProjectTemplate;
  // Token config
  use_global_token?: boolean;
  token?: string;
}

export interface UpdateProjectRequest {
  name?: string;
  description?: string;
  color?: string;
  archived?: boolean;
}

export interface CreateIssueRequest {
  title: string;
  body?: string;
  state?: IssueState;
  labels?: string[];
  priority?: IssuePriority;
  assignees?: string[];
  create_on_github?: boolean;
}

export interface UpdateIssueRequest {
  title?: string;
  body?: string;
  state?: IssueState;
  labels?: string[];
  priority?: IssuePriority;
  assignees?: string[];
  sync_to_github?: boolean;
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

export interface IssueListResponse {
  issues: Issue[];
  total: number;
}

export interface ProjectEntryListResponse {
  entries: ProjectEntry[];
  total: number;
}

// GitHub Sync Types
export interface SyncChangeDetail {
  issue_number: number;
  title: string;
  action: 'pulled' | 'pushed' | 'merged' | 'created' | 'updated' | 'conflicted';
  fields_affected: string[];
}

export interface GitHubSyncResult {
  success: boolean;
  message: string;
  pulled: number;
  pushed: number;
  merged: number;
  conflicts_resolved: number;
  change_details: SyncChangeDetail[];
  errors: string[];
}

// Project Event Types (for SSE)
export type ProjectEventType =
  | 'project_created'
  | 'project_updated'
  | 'project_deleted'
  | 'issue_created'
  | 'issue_updated'
  | 'issue_deleted'
  | 'entry_linked'
  | 'entry_unlinked'
  | 'github_sync_started'
  | 'github_sync_completed'
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
    // Sync fields
    created?: number;
    updated?: number;
    errors?: string[];
    // Entry fields
    entry_id?: string;
    entry_title?: string;
  };
}

// ============================================================================
// GitHub API Types
// ============================================================================

export interface GitHubTokenStatus {
  has_global_token: boolean;
  token_suffix?: string;
  verified_user?: string;
}

export interface SetGlobalTokenResponse {
  success: boolean;
  username?: string;
}

export interface GitHubUser {
  id: number;
  login: string;
  name?: string;
  avatar_url: string;
  html_url: string;
  email?: string;
  user_type: string;
}

export interface GitHubOrg {
  login: string;
  avatar_url: string;
  type: 'user' | 'org';
  description?: string;
}

export interface GitHubOrgsResponse {
  orgs: GitHubOrg[];
}

export interface GitHubRepo {
  name: string;
  full_name: string;
  description?: string;
  private: boolean;
}

export interface GitHubReposResponse {
  repos: GitHubRepo[];
  has_more: boolean;
}

export interface VerifyAccessRequest {
  owner: string;
  repo: string;
  token?: string;
}

export interface VerifyAccessResponse {
  has_access: boolean;
  permissions?: {
    admin: boolean;
    push: boolean;
    pull: boolean;
  };
}

// Project token type (matches backend ProjectTokenType)
export type ProjectTokenType =
  | { type: 'project_specific'; suffix?: string; verified_user?: string }
  | { type: 'using_global'; suffix?: string }
  | { type: 'not_configured' };
