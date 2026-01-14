import { useState } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  ArrowLeft,
  FolderKanban,
  GitBranch,
  Plus,
  RefreshCw,
  Settings,
  FileText,
  AlertCircle,
  CheckCircle2,
  Circle,
  MoreVertical,
  Trash2,
  ExternalLink,
  Clock,
  Edit3,
  X,
  BookOpen,
  Link2,
} from 'lucide-react';
import clsx from 'clsx';
import { projectApi, formatRelativeTime, getPriorityColor } from '../../api/projectClient';
import type {
  Project,
  Issue,
  CreateIssueRequest,
  UpdateIssueRequest,
  IssueState,
  IssuePriority,
} from '../../types/project';
import { useProjectEventRefetch } from '../../hooks/useProjectEvents';
import SyncNotification from '../../components/SyncNotification';
import type { GitHubSyncResult } from '../../types/project';

// Tab types
type TabId = 'issues' | 'knowledge' | 'settings';

const TABS: { id: TabId; label: string; icon: React.ElementType }[] = [
  { id: 'issues', label: 'Issues', icon: AlertCircle },
  { id: 'knowledge', label: 'Knowledge', icon: BookOpen },
  { id: 'settings', label: 'Settings', icon: Settings },
];

// Issue state icons
function StateIcon({ state }: { state: IssueState }) {
  switch (state) {
    case 'open':
      return <Circle className="w-4 h-4 text-emerald-400" />;
    case 'in_progress':
      return <Clock className="w-4 h-4 text-blue-400" />;
    case 'blocked':
      return <AlertCircle className="w-4 h-4 text-red-400" />;
    case 'closed':
      return <CheckCircle2 className="w-4 h-4 text-slate-400" />;
    default:
      return <Circle className="w-4 h-4 text-slate-400" />;
  }
}

// Create/Edit Issue Modal
function IssueModal({
  isOpen,
  onClose,
  projectId,
  issue,
}: {
  isOpen: boolean;
  onClose: () => void;
  projectId: string;
  issue?: Issue;
}) {
  const queryClient = useQueryClient();
  const isEdit = !!issue;

  const [formData, setFormData] = useState<CreateIssueRequest>({
    title: issue?.title || '',
    body: issue?.body || '',
    state: issue?.state || 'open',
    priority: issue?.priority,
    labels: issue?.labels || [],
    create_on_github: true, // Always sync to GitHub
  });
  const [error, setError] = useState<string | null>(null);

  const createMutation = useMutation({
    mutationFn: (data: CreateIssueRequest) => projectApi.createIssue(projectId, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project', projectId, 'issues'] });
      queryClient.invalidateQueries({ queryKey: ['project', projectId] });
      onClose();
    },
    onError: (err) => setError(err instanceof Error ? err.message : 'Failed to create issue'),
  });

  const updateMutation = useMutation({
    mutationFn: (data: UpdateIssueRequest) =>
      projectApi.updateIssue(projectId, issue!.id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project', projectId, 'issues'] });
      queryClient.invalidateQueries({ queryKey: ['project', projectId] });
      onClose();
    },
    onError: (err) => setError(err instanceof Error ? err.message : 'Failed to update issue'),
  });

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (isEdit) {
      updateMutation.mutate(formData);
    } else {
      createMutation.mutate(formData);
    }
  };

  const isPending = createMutation.isPending || updateMutation.isPending;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative w-full max-w-2xl mx-4 bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl max-h-[90vh] overflow-y-auto">
        <div className="sticky top-0 p-6 border-b border-slate-800 bg-slate-900/95 backdrop-blur z-10">
          <div className="flex items-center justify-between">
            <h2 className="text-xl font-semibold text-white">
              {isEdit ? 'Edit Issue' : 'Create Issue'}
            </h2>
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        <form onSubmit={handleSubmit} className="p-6 space-y-5">
          {error && (
            <div className="p-3 rounded-lg bg-red-500/10 border border-red-500/30 text-red-400 text-sm">
              {error}
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-slate-300 mb-2">Title</label>
            <input
              type="text"
              value={formData.title}
              onChange={(e) => setFormData({ ...formData, title: e.target.value })}
              className="w-full px-4 py-2.5 bg-slate-800 border border-slate-700 rounded-lg text-white placeholder-slate-500 focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500"
              placeholder="Issue title..."
              required
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-slate-300 mb-2">Description</label>
            <textarea
              value={formData.body || ''}
              onChange={(e) => setFormData({ ...formData, body: e.target.value })}
              className="w-full px-4 py-2.5 bg-slate-800 border border-slate-700 rounded-lg text-white placeholder-slate-500 focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500 resize-none"
              placeholder="Describe the issue..."
              rows={6}
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-slate-300 mb-2">State</label>
              <select
                value={formData.state}
                onChange={(e) =>
                  setFormData({ ...formData, state: e.target.value as IssueState })
                }
                className="w-full px-4 py-2.5 bg-slate-800 border border-slate-700 rounded-lg text-white focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500"
              >
                <option value="open">Open</option>
                <option value="in_progress">In Progress</option>
                <option value="blocked">Blocked</option>
                <option value="closed">Closed</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-300 mb-2">Priority</label>
              <select
                value={formData.priority || ''}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    priority: e.target.value ? (e.target.value as IssuePriority) : undefined,
                  })
                }
                className="w-full px-4 py-2.5 bg-slate-800 border border-slate-700 rounded-lg text-white focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500"
              >
                <option value="">No priority</option>
                <option value="low">Low</option>
                <option value="medium">Medium</option>
                <option value="high">High</option>
                <option value="critical">Critical</option>
              </select>
            </div>
          </div>

          <div className="flex justify-end gap-3 pt-4">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-sm font-medium text-slate-400 hover:text-white"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isPending}
              className="px-4 py-2 text-sm font-medium bg-cyan-500 text-white rounded-lg hover:bg-cyan-400 disabled:opacity-50 flex items-center gap-2"
            >
              {isPending ? (
                <>
                  <RefreshCw className="w-4 h-4 animate-spin" />
                  {isEdit ? 'Saving...' : 'Creating...'}
                </>
              ) : (
                <>
                  {isEdit ? <Edit3 className="w-4 h-4" /> : <Plus className="w-4 h-4" />}
                  {isEdit ? 'Save Changes' : 'Create Issue'}
                </>
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// Issue Row Component
function IssueRow({
  issue,
  projectId,
  onEdit,
}: {
  issue: Issue;
  projectId: string;
  onEdit: (issue: Issue) => void;
}) {
  const queryClient = useQueryClient();
  const [menuOpen, setMenuOpen] = useState(false);

  const deleteMutation = useMutation({
    mutationFn: () => projectApi.deleteIssue(projectId, issue.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project', projectId, 'issues'] });
      queryClient.invalidateQueries({ queryKey: ['project', projectId] });
    },
  });

  // Determine display number - prefer GitHub number as primary
  const displayNumber = issue.github_number || issue.number;
  const isGitHubSynced = !!issue.github_number;

  return (
    <div className="group flex items-center gap-4 p-4 hover:bg-slate-800/30 rounded-lg transition-colors">
      <StateIcon state={issue.state} />

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-white font-medium truncate">{issue.title}</span>
          {/* Issue number with GitHub indicator */}
          {isGitHubSynced ? (
            <a
              href={issue.github_url || `https://github.com/issues/${issue.github_number}`}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-slate-700/50 hover:bg-slate-700 transition-colors"
              title={`GitHub Issue #${displayNumber}`}
            >
              <GitBranch className="w-3 h-3 text-slate-400" />
              <span className="text-xs text-slate-300 font-mono">#{displayNumber}</span>
            </a>
          ) : (
            <span className="text-xs text-slate-500 font-mono">#{displayNumber}</span>
          )}
        </div>

        <div className="flex items-center gap-2 mt-1">
          {issue.priority && (
            <span
              className={clsx(
                'px-1.5 py-0.5 text-xs rounded border',
                getPriorityColor(issue.priority)
              )}
            >
              {issue.priority}
            </span>
          )}
          {issue.labels.slice(0, 3).map((label) => (
            <span
              key={label}
              className="px-1.5 py-0.5 text-xs rounded bg-slate-700 text-slate-300"
            >
              {label}
            </span>
          ))}
          <span className="text-xs text-slate-500">
            {formatRelativeTime(issue.updated_at)}
          </span>
        </div>
      </div>

      <div className="relative">
        <button
          onClick={() => setMenuOpen(!menuOpen)}
          className="p-1.5 rounded-lg text-slate-500 hover:text-white hover:bg-slate-700 opacity-0 group-hover:opacity-100 transition-all"
        >
          <MoreVertical className="w-4 h-4" />
        </button>

        {menuOpen && (
          <>
            <div className="fixed inset-0 z-10" onClick={() => setMenuOpen(false)} />
            <div className="absolute right-0 top-full mt-1 w-40 bg-slate-800 border border-slate-700 rounded-lg shadow-xl z-20 py-1">
              <button
                onClick={() => {
                  onEdit(issue);
                  setMenuOpen(false);
                }}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm text-slate-300 hover:text-white hover:bg-slate-700/50"
              >
                <Edit3 className="w-4 h-4" />
                Edit
              </button>
              {issue.github_url && (
                <a
                  href={issue.github_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-slate-300 hover:text-white hover:bg-slate-700/50"
                  onClick={() => setMenuOpen(false)}
                >
                  <ExternalLink className="w-4 h-4" />
                  View on GitHub
                </a>
              )}
              <div className="border-t border-slate-700 my-1" />
              <button
                onClick={() => {
                  if (confirm('Delete this issue?')) {
                    deleteMutation.mutate();
                  }
                  setMenuOpen(false);
                }}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm text-red-400 hover:text-red-300 hover:bg-red-500/10"
              >
                <Trash2 className="w-4 h-4" />
                Delete
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

// Issues Tab Content
function IssuesTab({ project }: { project: Project }) {
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingIssue, setEditingIssue] = useState<Issue | undefined>();
  const [stateFilter, setStateFilter] = useState<string>('');

  const { data, isLoading } = useQuery({
    queryKey: ['project', project.id, 'issues', stateFilter],
    queryFn: () =>
      projectApi.listIssues(project.id, {
        state: stateFilter || undefined,
      }),
  });

  const issues = data?.issues || [];

  return (
    <div className="space-y-4">
      {/* Toolbar */}
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-center gap-2">
          <select
            value={stateFilter}
            onChange={(e) => setStateFilter(e.target.value)}
            className="px-3 py-1.5 bg-slate-800 border border-slate-700 rounded-lg text-sm text-white focus:border-cyan-500"
          >
            <option value="">All States</option>
            <option value="open">Open</option>
            <option value="in_progress">In Progress</option>
            <option value="blocked">Blocked</option>
            <option value="closed">Closed</option>
          </select>
        </div>
        <button
          onClick={() => setShowCreateModal(true)}
          className="flex items-center gap-2 px-3 py-1.5 bg-cyan-500 text-white rounded-lg hover:bg-cyan-400 text-sm font-medium"
        >
          <Plus className="w-4 h-4" />
          New Issue
        </button>
      </div>

      {/* Issues List */}
      {isLoading ? (
        <div className="flex justify-center py-12">
          <RefreshCw className="w-6 h-6 text-cyan-500 animate-spin" />
        </div>
      ) : issues.length > 0 ? (
        <div className="divide-y divide-slate-800">
          {issues.map((issue) => (
            <IssueRow
              key={issue.id}
              issue={issue}
              projectId={project.id}
              onEdit={(issue) => setEditingIssue(issue)}
            />
          ))}
        </div>
      ) : (
        <div className="text-center py-12">
          <AlertCircle className="w-12 h-12 text-slate-600 mx-auto mb-4" />
          <p className="text-slate-400">No issues found</p>
          <button
            onClick={() => setShowCreateModal(true)}
            className="mt-4 text-cyan-400 hover:text-cyan-300 text-sm"
          >
            Create the first issue
          </button>
        </div>
      )}

      {/* Modals */}
      <IssueModal
        isOpen={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        projectId={project.id}
      />
      {editingIssue && (
        <IssueModal
          isOpen={true}
          onClose={() => setEditingIssue(undefined)}
          projectId={project.id}
          issue={editingIssue}
        />
      )}
    </div>
  );
}

// Knowledge Tab Content
function KnowledgeTab({ project }: { project: Project }) {
  const queryClient = useQueryClient();

  const { data, isLoading } = useQuery({
    queryKey: ['project', project.id, 'entries'],
    queryFn: () => projectApi.listLinkedEntries(project.id),
  });

  const unlinkMutation = useMutation({
    mutationFn: (entryId: string) => projectApi.unlinkEntry(project.id, entryId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project', project.id, 'entries'] });
    },
  });

  const entries = data?.entries || [];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <p className="text-sm text-slate-400">
          Knowledge entries linked to this project
        </p>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <RefreshCw className="w-6 h-6 text-cyan-500 animate-spin" />
        </div>
      ) : entries.length > 0 ? (
        <div className="space-y-2">
          {entries.map((entry) => (
            <div
              key={entry.id}
              className="flex items-center gap-4 p-4 bg-slate-800/30 rounded-lg border border-slate-700/50 hover:border-slate-600/50 transition-colors"
            >
              <FileText className="w-5 h-5 text-cyan-400 flex-shrink-0" />
              <div className="flex-1 min-w-0">
                <Link
                  to={`/entries/${entry.entry_id}`}
                  className="text-white font-medium hover:text-cyan-400 truncate block"
                >
                  {entry.title}
                </Link>
                <div className="flex items-center gap-2 mt-1">
                  <span className="text-xs text-slate-500 bg-slate-800 px-1.5 py-0.5 rounded">
                    {entry.entry_type}
                  </span>
                  {entry.relevance && (
                    <span className="text-xs text-cyan-400">
                      {Math.round(entry.relevance * 100)}% relevant
                    </span>
                  )}
                  <span className="text-xs text-slate-500">
                    Linked {formatRelativeTime(entry.linked_at)}
                  </span>
                </div>
                {entry.notes && (
                  <p className="text-sm text-slate-400 mt-2">{entry.notes}</p>
                )}
              </div>
              <button
                onClick={() => {
                  if (confirm('Unlink this entry from the project?')) {
                    unlinkMutation.mutate(entry.entry_id);
                  }
                }}
                className="p-1.5 rounded-lg text-slate-500 hover:text-red-400 hover:bg-red-500/10"
              >
                <Link2 className="w-4 h-4" />
              </button>
            </div>
          ))}
        </div>
      ) : (
        <div className="text-center py-12">
          <BookOpen className="w-12 h-12 text-slate-600 mx-auto mb-4" />
          <p className="text-slate-400">No knowledge entries linked</p>
          <p className="text-slate-500 text-sm mt-2">
            Use MCP tools to link entries to this project
          </p>
        </div>
      )}
    </div>
  );
}

// Settings Tab Content
function SettingsTab({ project }: { project: Project }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [token, setToken] = useState('');
  const [showTokenForm, setShowTokenForm] = useState(false);

  // GitHub token creation URL with pre-selected scopes
  const tokenDescription = encodeURIComponent(`Kix - ${project.name}`);
  const githubTokenUrl = `https://github.com/settings/tokens/new?scopes=repo,project&description=${tokenDescription}`;

  const deleteMutation = useMutation({
    mutationFn: () => projectApi.deleteProject(project.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['projects'] });
      navigate('/projects');
    },
  });

  const setTokenMutation = useMutation({
    mutationFn: (token: string) => projectApi.setGitHubToken(project.id, token),
    onSuccess: () => {
      setToken('');
      setShowTokenForm(false);
      queryClient.invalidateQueries({ queryKey: ['project', project.id] });
    },
  });

  return (
    <div className="space-y-6 max-w-2xl">
      {/* GitHub Settings */}
      <div className="p-6 rounded-xl bg-slate-800/30 border border-slate-700">
        <h3 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
          <GitBranch className="w-5 h-5 text-cyan-400" />
          GitHub Configuration
        </h3>

        <div className="space-y-3">
          <div className="flex justify-between text-sm">
            <span className="text-slate-400">Repository</span>
            <a
              href={`https://github.com/${project.github_owner}/${project.github_repo}`}
              target="_blank"
              rel="noopener noreferrer"
              className="text-cyan-400 hover:text-cyan-300 flex items-center gap-1"
            >
              {project.github_owner}/{project.github_repo}
              <ExternalLink className="w-3.5 h-3.5" />
            </a>
          </div>

          <div className="flex justify-between text-sm">
            <span className="text-slate-400">Token Status</span>
            <span className={project.has_token ? 'text-emerald-400' : 'text-yellow-400'}>
              {project.has_token ? 'Configured' : 'Not set'}
            </span>
          </div>

          {!showTokenForm ? (
            <div className="mt-4 flex flex-col gap-2">
              <button
                onClick={() => setShowTokenForm(true)}
                className="text-sm text-cyan-400 hover:text-cyan-300 text-left"
              >
                {project.has_token ? 'Update Token' : 'Set GitHub Token'}
              </button>
              <a
                href={githubTokenUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm text-slate-400 hover:text-slate-300 flex items-center gap-1.5"
              >
                <Plus className="w-3.5 h-3.5" />
                Create a new token with required permissions
                <ExternalLink className="w-3 h-3" />
              </a>
            </div>
          ) : (
            <div className="mt-4 space-y-3">
              <div className="flex items-center gap-2">
                <a
                  href={githubTokenUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-xs text-slate-400 hover:text-cyan-400 flex items-center gap-1"
                >
                  <Plus className="w-3 h-3" />
                  Create new token
                  <ExternalLink className="w-3 h-3" />
                </a>
              </div>
              <input
                type="password"
                value={token}
                onChange={(e) => setToken(e.target.value)}
                placeholder="ghp_xxxxx..."
                className="w-full px-4 py-2 bg-slate-800 border border-slate-700 rounded-lg text-white placeholder-slate-500 focus:border-cyan-500"
              />
              <div className="flex gap-2">
                <button
                  onClick={() => setTokenMutation.mutate(token)}
                  disabled={!token || setTokenMutation.isPending}
                  className="px-3 py-1.5 bg-cyan-500 text-white rounded-lg hover:bg-cyan-400 disabled:opacity-50 text-sm"
                >
                  {setTokenMutation.isPending ? 'Saving...' : 'Save Token'}
                </button>
                <button
                  onClick={() => {
                    setShowTokenForm(false);
                    setToken('');
                  }}
                  className="px-3 py-1.5 text-slate-400 hover:text-white text-sm"
                >
                  Cancel
                </button>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Danger Zone */}
      <div className="p-6 rounded-xl bg-red-500/5 border border-red-500/20">
        <h3 className="text-lg font-semibold text-red-400 mb-4">Danger Zone</h3>
        <p className="text-sm text-slate-400 mb-4">
          Deleting a project will remove all local issues and entry links. GitHub issues will
          not be affected.
        </p>
        <button
          onClick={() => {
            if (confirm('Are you sure you want to delete this project? This action cannot be undone.')) {
              deleteMutation.mutate();
            }
          }}
          disabled={deleteMutation.isPending}
          className="flex items-center gap-2 px-4 py-2 bg-red-500/10 text-red-400 border border-red-500/30 rounded-lg hover:bg-red-500/20 text-sm"
        >
          <Trash2 className="w-4 h-4" />
          {deleteMutation.isPending ? 'Deleting...' : 'Delete Project'}
        </button>
      </div>
    </div>
  );
}

// Link GitHub Project Modal
function LinkGitHubProjectModal({
  isOpen,
  onClose,
  projectId,
  onSuccess,
}: {
  isOpen: boolean;
  onClose: () => void;
  projectId: string;
  onSuccess: () => void;
}) {
  const [selectedTemplate, setSelectedTemplate] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const linkMutation = useMutation({
    mutationFn: (template: string) => projectApi.linkGitHubProject(projectId, template),
    onSuccess: (data) => {
      if (data.warning) {
        setError(data.warning);
      } else {
        onSuccess();
        onClose();
      }
    },
    onError: (err) => setError(err instanceof Error ? err.message : 'Failed to link GitHub Project'),
  });

  if (!isOpen) return null;

  const templates = [
    { id: 'kanban', name: 'Kanban', description: 'Simple board with Todo → In Progress → Done' },
    { id: 'bug_tracking', name: 'Bug Tracking', description: 'Triage workflow with Priority and Severity' },
    { id: 'sprint_planning', name: 'Sprint Planning', description: 'Agile sprints with Story Points' },
    { id: 'feature_roadmap', name: 'Feature Roadmap', description: 'Quarter-based planning with Teams' },
  ];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative w-full max-w-lg mx-4 bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl">
        <div className="p-6 border-b border-slate-800">
          <div className="flex items-center justify-between">
            <h2 className="text-xl font-semibold text-white">Link GitHub Project V2</h2>
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
          <p className="mt-2 text-sm text-slate-400">
            Select a template for your GitHub Project board
          </p>
        </div>

        <div className="p-6 space-y-3">
          {templates.map((template) => (
            <button
              key={template.id}
              onClick={() => setSelectedTemplate(template.id)}
              className={clsx(
                'w-full p-4 rounded-lg border-2 text-left transition-all',
                selectedTemplate === template.id
                  ? 'border-cyan-500 bg-cyan-500/10'
                  : 'border-slate-700 bg-slate-800/50 hover:border-slate-600'
              )}
            >
              <div className="font-medium text-white">{template.name}</div>
              <div className="text-sm text-slate-400 mt-1">{template.description}</div>
            </button>
          ))}

          {error && (
            <div className="p-3 rounded-lg bg-red-500/10 border border-red-500/30 text-red-400 text-sm">
              {error}
            </div>
          )}
        </div>

        <div className="p-6 border-t border-slate-800 flex justify-end gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-slate-400 hover:text-white"
          >
            Cancel
          </button>
          <button
            onClick={() => selectedTemplate && linkMutation.mutate(selectedTemplate)}
            disabled={!selectedTemplate || linkMutation.isPending}
            className="px-4 py-2 text-sm font-medium bg-cyan-500 text-white rounded-lg hover:bg-cyan-400 disabled:opacity-50 flex items-center gap-2"
          >
            {linkMutation.isPending ? (
              <>
                <RefreshCw className="w-4 h-4 animate-spin" />
                Creating...
              </>
            ) : (
              <>
                <Link2 className="w-4 h-4" />
                Create & Link
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

// Main Component
export default function ProjectDetail() {
  const { id } = useParams<{ id: string }>();
  const queryClient = useQueryClient();
  const [activeTab, setActiveTab] = useState<TabId>('issues');
  const [isLinkProjectModalOpen, setIsLinkProjectModalOpen] = useState(false);
  const [syncResult, setSyncResult] = useState<GitHubSyncResult | null>(null);
  const [lastSyncTime, setLastSyncTime] = useState<Date | null>(null);

  const { data: project, isLoading, error, refetch } = useQuery({
    queryKey: ['project', id],
    queryFn: () => projectApi.getProject(id!),
    enabled: !!id,
  });

  const { refetch: refetchIssues } = useQuery({
    queryKey: ['project', id, 'issues'],
    queryFn: () => projectApi.listIssues(id!),
    enabled: !!id,
  });

  const { refetch: refetchEntries } = useQuery({
    queryKey: ['project', id, 'entries'],
    queryFn: () => projectApi.listLinkedEntries(id!),
    enabled: !!id,
  });

  const syncMutation = useMutation({
    mutationFn: () => projectApi.syncGitHub(id!),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['project', id] });
      queryClient.invalidateQueries({ queryKey: ['project', id, 'issues'] });
      setSyncResult(data);
      setLastSyncTime(new Date());
    },
    onError: (error) => {
      setSyncResult({
        success: false,
        message: error instanceof Error ? error.message : 'Sync failed',
        pulled: 0,
        pushed: 0,
        merged: 0,
        conflicts_resolved: 0,
        change_details: [],
        errors: [error instanceof Error ? error.message : 'An unknown error occurred'],
      });
    },
  });

  // Subscribe to project events for real-time updates
  useProjectEventRefetch(id, {
    refetchProject: refetch,
    refetchIssues,
    refetchEntries,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-[60vh]">
        <div className="flex flex-col items-center gap-4">
          <div className="w-12 h-12 border-4 border-cyan-500 border-t-transparent rounded-full animate-spin" />
          <p className="text-slate-400 font-mono">Loading project...</p>
        </div>
      </div>
    );
  }

  if (error || !project) {
    return (
      <div className="card p-8 text-center">
        <p className="text-red-400">Failed to load project</p>
        <p className="text-slate-500 text-sm mt-2">{(error as Error)?.message || 'Project not found'}</p>
        <Link
          to="/projects"
          className="mt-4 inline-flex items-center gap-2 text-cyan-400 hover:text-cyan-300"
        >
          <ArrowLeft className="w-4 h-4" />
          Back to projects
        </Link>
      </div>
    );
  }

  // Default color if none set
  const projectColor = project.color || '#06b6d4';

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-start gap-4">
        <Link
          to="/projects"
          className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
        >
          <ArrowLeft className="w-5 h-5" />
        </Link>

        <div className="flex-1">
          <div className="flex items-center gap-3">
            <div
              className="w-12 h-12 rounded-xl flex items-center justify-center"
              style={{ backgroundColor: `${projectColor}20` }}
            >
              <FolderKanban className="w-6 h-6" style={{ color: projectColor }} />
            </div>
            <div>
              <h1 className="text-2xl font-bold text-white">{project.name}</h1>
              <div className="flex items-center gap-2 text-sm text-slate-400">
                <GitBranch className="w-4 h-4" />
                <a
                  href={`https://github.com/${project.github_owner}/${project.github_repo}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="hover:text-cyan-400"
                >
                  {project.github_owner}/{project.github_repo}
                </a>
              </div>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {/* Link GitHub Project button - shown when project has GitHub but no Project V2 */}
          {project.has_github && !project.github_project_v2_url && (
            <button
              onClick={() => setIsLinkProjectModalOpen(true)}
              className="flex items-center gap-2 px-4 py-2 bg-amber-500/10 text-amber-400 border border-amber-500/30 rounded-lg hover:bg-amber-500/20 transition-colors text-sm"
            >
              <Link2 className="w-4 h-4" />
              Link GitHub Project
            </button>
          )}

          {/* GitHub Project V2 link - shown when linked */}
          {project.github_project_v2_url && (
            <a
              href={project.github_project_v2_url}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-2 px-4 py-2 bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 rounded-lg hover:bg-emerald-500/20 transition-colors text-sm"
            >
              <ExternalLink className="w-4 h-4" />
              Project Board
            </a>
          )}

          {/* Enhanced Sync Button with Last Sync Time */}
          <div className="flex items-center gap-2">
            {lastSyncTime && !syncMutation.isPending && (
              <span className="text-xs text-slate-500 flex items-center gap-1">
                <Clock className="w-3 h-3" />
                {formatRelativeTime(lastSyncTime.toISOString())}
              </span>
            )}
            <button
              onClick={() => syncMutation.mutate()}
              disabled={syncMutation.isPending}
              className={clsx(
                'flex items-center gap-2 px-4 py-2 rounded-lg transition-colors text-sm',
                syncMutation.isPending
                  ? 'bg-cyan-500/20 text-cyan-400 border border-cyan-500/30'
                  : 'bg-slate-800 text-white border border-slate-700 hover:bg-slate-700'
              )}
            >
              <RefreshCw className={clsx('w-4 h-4', syncMutation.isPending && 'animate-spin')} />
              {syncMutation.isPending ? 'Syncing...' : 'Sync GitHub'}
            </button>
          </div>
        </div>
      </div>

      {/* Stats Bar */}
      <div className="flex items-center gap-6 p-4 bg-slate-800/30 rounded-xl border border-slate-700/50">
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded-full bg-emerald-400" />
          <span className="text-white font-medium">{project.stats?.open_issues || 0}</span>
          <span className="text-slate-400 text-sm">open</span>
        </div>
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded-full bg-blue-400" />
          <span className="text-white font-medium">{project.stats?.in_progress_issues || 0}</span>
          <span className="text-slate-400 text-sm">in progress</span>
        </div>
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded-full bg-slate-400" />
          <span className="text-white font-medium">{project.stats?.closed_issues || 0}</span>
          <span className="text-slate-400 text-sm">closed</span>
        </div>
        <div className="border-l border-slate-700 h-6" />
        <div className="flex items-center gap-2">
          <FileText className="w-4 h-4 text-cyan-400" />
          <span className="text-white font-medium">{project.stats?.linked_entries || 0}</span>
          <span className="text-slate-400 text-sm">entries linked</span>
        </div>
      </div>

      {/* Tabs */}
      <div className="border-b border-slate-800">
        <nav className="flex gap-1">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={clsx(
                'flex items-center gap-2 px-4 py-3 text-sm font-medium transition-colors relative',
                activeTab === tab.id
                  ? 'text-cyan-400'
                  : 'text-slate-400 hover:text-white'
              )}
            >
              <tab.icon className="w-4 h-4" />
              {tab.label}
              {activeTab === tab.id && (
                <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-cyan-400" />
              )}
            </button>
          ))}
        </nav>
      </div>

      {/* Tab Content */}
      <div className="card p-6">
        {activeTab === 'issues' && <IssuesTab project={project} />}
        {activeTab === 'knowledge' && <KnowledgeTab project={project} />}
        {activeTab === 'settings' && <SettingsTab project={project} />}
      </div>

      {/* Link GitHub Project Modal */}
      <LinkGitHubProjectModal
        isOpen={isLinkProjectModalOpen}
        onClose={() => setIsLinkProjectModalOpen(false)}
        projectId={project.id}
        onSuccess={() => {
          refetch();
          queryClient.invalidateQueries({ queryKey: ['project', id] });
        }}
      />

      {/* Sync Notification */}
      <SyncNotification
        result={syncResult}
        onClose={() => setSyncResult(null)}
      />
    </div>
  );
}
