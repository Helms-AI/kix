import { useState } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  ArrowLeft,
  FolderKanban,
  Plus,
  RefreshCw,
  Settings,
  FileText,
  AlertCircle,
  CheckCircle2,
  Circle,
  MoreVertical,
  Trash2,
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
  WorkItem,
  CreateWorkItemRequest,
  UpdateWorkItemRequest,
  WorkItemState,
  WorkItemPriority,
  WorkItemType,
  BoardColumn,
} from '../../types/project';
import { KanbanBoard, BoardHeader } from '../../components/board';
import { useProjectEventRefetch } from '../../hooks/useProjectEvents';

// Tab types
type TabId = 'items' | 'knowledge' | 'settings';

const TABS: { id: TabId; label: string; icon: React.ElementType }[] = [
  { id: 'items', label: 'Work Items', icon: AlertCircle },
  { id: 'knowledge', label: 'Knowledge', icon: BookOpen },
  { id: 'settings', label: 'Settings', icon: Settings },
];

// Work item state icons
function StateIcon({ state }: { state: WorkItemState }) {
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

// Create/Edit Work Item Modal
function WorkItemModal({
  isOpen,
  onClose,
  projectId,
  item,
}: {
  isOpen: boolean;
  onClose: () => void;
  projectId: string;
  item?: WorkItem;
}) {
  const queryClient = useQueryClient();
  const isEdit = !!item;

  const [formData, setFormData] = useState<CreateWorkItemRequest>({
    title: item?.title || '',
    body: item?.body || '',
    state: item?.state || 'open',
    priority: item?.priority,
    labels: item?.labels || [],
    item_type: item?.item_type || 'task',
  });
  const [error, setError] = useState<string | null>(null);

  const createMutation = useMutation({
    mutationFn: (data: CreateWorkItemRequest) => projectApi.createWorkItem(projectId, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project', projectId, 'items'] });
      queryClient.invalidateQueries({ queryKey: ['project', projectId] });
      onClose();
    },
    onError: (err) => setError(err instanceof Error ? err.message : 'Failed to create work item'),
  });

  const updateMutation = useMutation({
    mutationFn: (data: UpdateWorkItemRequest) =>
      projectApi.updateWorkItem(projectId, item!.id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project', projectId, 'items'] });
      queryClient.invalidateQueries({ queryKey: ['project', projectId] });
      onClose();
    },
    onError: (err) => setError(err instanceof Error ? err.message : 'Failed to update work item'),
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
              {isEdit ? 'Edit Work Item' : 'Create Work Item'}
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
              placeholder="Work item title..."
              required
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-slate-300 mb-2">Description</label>
            <textarea
              value={formData.body || ''}
              onChange={(e) => setFormData({ ...formData, body: e.target.value })}
              className="w-full px-4 py-2.5 bg-slate-800 border border-slate-700 rounded-lg text-white placeholder-slate-500 focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500 resize-none"
              placeholder="Describe the work item..."
              rows={6}
            />
          </div>

          <div className="grid grid-cols-3 gap-4">
            <div>
              <label className="block text-sm font-medium text-slate-300 mb-2">Type</label>
              <select
                value={formData.item_type}
                onChange={(e) =>
                  setFormData({ ...formData, item_type: e.target.value as WorkItemType })
                }
                className="w-full px-4 py-2.5 bg-slate-800 border border-slate-700 rounded-lg text-white focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500"
              >
                <option value="task">Task</option>
                <option value="story">Story</option>
                <option value="epic">Epic</option>
                <option value="bug">Bug</option>
                <option value="subtask">Subtask</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-300 mb-2">State</label>
              <select
                value={formData.state}
                onChange={(e) =>
                  setFormData({ ...formData, state: e.target.value as WorkItemState })
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
                    priority: e.target.value ? (e.target.value as WorkItemPriority) : undefined,
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
                  {isEdit ? 'Save Changes' : 'Create Work Item'}
                </>
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// Work Item Row Component
function WorkItemRow({
  item,
  projectId,
  onEdit,
}: {
  item: WorkItem;
  projectId: string;
  onEdit: (item: WorkItem) => void;
}) {
  const queryClient = useQueryClient();
  const [menuOpen, setMenuOpen] = useState(false);

  const deleteMutation = useMutation({
    mutationFn: () => projectApi.deleteWorkItem(projectId, item.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['project', projectId, 'items'] });
      queryClient.invalidateQueries({ queryKey: ['project', projectId] });
    },
  });

  return (
    <div className="group flex items-center gap-4 p-4 hover:bg-slate-800/30 rounded-lg transition-colors">
      <StateIcon state={item.state} />

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-white font-medium truncate">{item.title}</span>
          <span className="text-xs text-slate-500 font-mono">#{item.number}</span>
        </div>

        <div className="flex items-center gap-2 mt-1">
          <span className="px-1.5 py-0.5 text-xs rounded bg-slate-700 text-slate-300">
            {item.item_type}
          </span>
          {item.priority && (
            <span
              className={clsx(
                'px-1.5 py-0.5 text-xs rounded border',
                getPriorityColor(item.priority)
              )}
            >
              {item.priority}
            </span>
          )}
          {item.labels.slice(0, 3).map((label) => (
            <span
              key={label}
              className="px-1.5 py-0.5 text-xs rounded bg-slate-700 text-slate-300"
            >
              {label}
            </span>
          ))}
          <span className="text-xs text-slate-500">
            {formatRelativeTime(item.updated_at)}
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
                  onEdit(item);
                  setMenuOpen(false);
                }}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm text-slate-300 hover:text-white hover:bg-slate-700/50"
              >
                <Edit3 className="w-4 h-4" />
                Edit
              </button>
              <div className="border-t border-slate-700 my-1" />
              <button
                onClick={() => {
                  if (confirm('Delete this work item?')) {
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

// Work Items Tab Content with Board/List View Toggle
function WorkItemsTab({ project }: { project: Project }) {
  const queryClient = useQueryClient();
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingItem, setEditingItem] = useState<WorkItem | undefined>();
  const [stateFilter, setStateFilter] = useState<string>('');
  const [viewMode, setViewMode] = useState<'board' | 'list'>('board'); // Default to board
  const [filterItemType, setFilterItemType] = useState<WorkItemType | null>(null);

  // List view query
  const { data: listData, isLoading: isListLoading } = useQuery({
    queryKey: ['project', project.id, 'items', stateFilter],
    queryFn: () =>
      projectApi.listWorkItems(project.id, {
        state: stateFilter || undefined,
      }),
    enabled: viewMode === 'list',
  });

  // Board view query
  const { data: boardData, isLoading: isBoardLoading, refetch: refetchBoard } = useQuery({
    queryKey: ['project', project.id, 'board', filterItemType],
    queryFn: () => projectApi.getBoard(project.id, filterItemType || undefined),
    enabled: viewMode === 'board',
  });

  const items = listData?.items || [];

  // Handle board update (refresh after drag-drop)
  const handleBoardUpdate = () => {
    refetchBoard();
    queryClient.invalidateQueries({ queryKey: ['project', project.id] });
  };

  // Handle card click in board view
  const handleCardClick = (item: WorkItem) => {
    setEditingItem(item);
  };

  // Handle add card from board
  const handleAddCard = (_column: BoardColumn, _itemType?: WorkItemType) => {
    setShowCreateModal(true);
    // Could pre-fill the column and type in the modal
  };

  // Calculate column counts for header
  const columnCounts = boardData?.column_counts || {
    backlog: 0,
    todo: 0,
    in_progress: 0,
    in_review: 0,
    testing: 0,
    done: 0,
  };

  return (
    <div className="space-y-0 -m-6">
      {/* Board Header (only in board view) */}
      {viewMode === 'board' && (
        <BoardHeader
          totalItems={boardData?.total_items || 0}
          columnCounts={columnCounts as Record<BoardColumn, number>}
          viewMode={viewMode}
          onViewModeChange={setViewMode}
          filterItemType={filterItemType}
          onFilterItemTypeChange={setFilterItemType}
          onCreateItem={() => setShowCreateModal(true)}
          onRefresh={handleBoardUpdate}
          isLoading={isBoardLoading}
        />
      )}

      {/* List View Header */}
      {viewMode === 'list' && (
        <div className="flex items-center justify-between gap-4 px-6 py-4 border-b border-slate-800/50">
          <div className="flex items-center gap-2">
            {/* View Toggle */}
            <div className="flex items-center bg-slate-800/50 rounded-lg p-0.5 mr-4">
              <button
                onClick={() => setViewMode('board')}
                className="px-3 py-1.5 text-sm rounded-md transition-colors text-slate-500 hover:text-slate-300"
              >
                Board
              </button>
              <button
                onClick={() => setViewMode('list')}
                className="px-3 py-1.5 text-sm rounded-md transition-colors bg-slate-700 text-slate-200"
              >
                List
              </button>
            </div>
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
            New Item
          </button>
        </div>
      )}

      {/* Content Area */}
      <div className="p-6">
        {/* Board View */}
        {viewMode === 'board' && (
          <>
            {isBoardLoading ? (
              <div className="flex justify-center py-12">
                <RefreshCw className="w-6 h-6 text-cyan-500 animate-spin" />
              </div>
            ) : boardData ? (
              <KanbanBoard
                projectId={project.id}
                boardData={boardData}
                onBoardUpdate={handleBoardUpdate}
                onCardClick={handleCardClick}
                onAddCard={handleAddCard}
              />
            ) : (
              <div className="text-center py-12">
                <AlertCircle className="w-12 h-12 text-slate-600 mx-auto mb-4" />
                <p className="text-slate-400">Failed to load board</p>
              </div>
            )}
          </>
        )}

        {/* List View */}
        {viewMode === 'list' && (
          <>
            {isListLoading ? (
              <div className="flex justify-center py-12">
                <RefreshCw className="w-6 h-6 text-cyan-500 animate-spin" />
              </div>
            ) : items.length > 0 ? (
              <div className="divide-y divide-slate-800">
                {items.map((item) => (
                  <WorkItemRow
                    key={item.id}
                    item={item}
                    projectId={project.id}
                    onEdit={(item) => setEditingItem(item)}
                  />
                ))}
              </div>
            ) : (
              <div className="text-center py-12">
                <AlertCircle className="w-12 h-12 text-slate-600 mx-auto mb-4" />
                <p className="text-slate-400">No work items found</p>
                <button
                  onClick={() => setShowCreateModal(true)}
                  className="mt-4 text-cyan-400 hover:text-cyan-300 text-sm"
                >
                  Create the first work item
                </button>
              </div>
            )}
          </>
        )}
      </div>

      {/* Modals */}
      <WorkItemModal
        isOpen={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        projectId={project.id}
      />
      {editingItem && (
        <WorkItemModal
          isOpen={true}
          onClose={() => setEditingItem(undefined)}
          projectId={project.id}
          item={editingItem}
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

  const deleteMutation = useMutation({
    mutationFn: () => projectApi.deleteProject(project.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['projects'] });
      navigate('/projects');
    },
  });

  return (
    <div className="space-y-6 max-w-2xl">
      {/* Danger Zone */}
      <div className="p-6 rounded-xl bg-red-500/5 border border-red-500/20">
        <h3 className="text-lg font-semibold text-red-400 mb-4">Danger Zone</h3>
        <p className="text-sm text-slate-400 mb-4">
          Deleting a project will remove all work items and entry links. This action cannot be undone.
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

// Main Component
export default function ProjectDetail() {
  const { id } = useParams<{ id: string }>();
  const [activeTab, setActiveTab] = useState<TabId>('items');

  const { data: project, isLoading, error, refetch } = useQuery({
    queryKey: ['project', id],
    queryFn: () => projectApi.getProject(id!),
    enabled: !!id,
  });

  const { refetch: refetchItems } = useQuery({
    queryKey: ['project', id, 'items'],
    queryFn: () => projectApi.listWorkItems(id!),
    enabled: !!id,
  });

  const { refetch: refetchEntries } = useQuery({
    queryKey: ['project', id, 'entries'],
    queryFn: () => projectApi.listLinkedEntries(id!),
    enabled: !!id,
  });

  // Subscribe to project events for real-time updates
  useProjectEventRefetch(id, {
    refetchProject: refetch,
    refetchWorkItems: refetchItems,
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
              {project.description && (
                <p className="text-sm text-slate-400">{project.description}</p>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Stats Bar */}
      <div className="flex items-center gap-6 p-4 bg-slate-800/30 rounded-xl border border-slate-700/50">
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded-full bg-emerald-400" />
          <span className="text-white font-medium">{project.stats?.open_items || 0}</span>
          <span className="text-slate-400 text-sm">open</span>
        </div>
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded-full bg-slate-400" />
          <span className="text-white font-medium">{project.stats?.closed_items || 0}</span>
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
        {activeTab === 'items' && <WorkItemsTab project={project} />}
        {activeTab === 'knowledge' && <KnowledgeTab project={project} />}
        {activeTab === 'settings' && <SettingsTab project={project} />}
      </div>
    </div>
  );
}
