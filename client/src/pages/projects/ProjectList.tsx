import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import {
  Plus,
  FolderKanban,
  GitBranch,
  FileText,
  ArrowRight,
  Archive,
  MoreVertical,
  RefreshCw,
  Trash2,
  ExternalLink,
  Folder,
} from 'lucide-react';
import clsx from 'clsx';
import { projectApi, formatRelativeTime } from '../../api/projectClient';
import type { Project } from '../../types/project';
import { useProjectEventRefetch } from '../../hooks/useProjectEvents';
import CreateProjectWizard from './components/CreateProjectWizard';

function ProjectCard({ project }: { project: Project }) {
  const [menuOpen, setMenuOpen] = useState(false);
  const queryClient = useQueryClient();

  const deleteMutation = useMutation({
    mutationFn: () => projectApi.deleteProject(project.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['projects'] });
    },
  });

  const syncMutation = useMutation({
    mutationFn: () => projectApi.syncGitHub(project.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['projects'] });
    },
  });

  // Default color if none set
  const projectColor = project.color || '#06b6d4';
  const hasGitHub = project.has_github && project.github_owner && project.github_repo;

  return (
    <div
      className="group relative bg-slate-800/50 border border-slate-700 rounded-xl overflow-hidden hover:border-slate-600 transition-all duration-300"
    >
      {/* Color bar */}
      <div className="h-1" style={{ backgroundColor: projectColor }} />

      <div className="p-5">
        {/* Header */}
        <div className="flex items-start justify-between mb-4">
          <Link
            to={`/projects/${project.id}`}
            className="flex items-center gap-3 flex-1 group/link"
          >
            <div
              className="w-10 h-10 rounded-lg flex items-center justify-center"
              style={{ backgroundColor: `${projectColor}20` }}
            >
              {hasGitHub ? (
                <FolderKanban className="w-5 h-5" style={{ color: projectColor }} />
              ) : (
                <Folder className="w-5 h-5" style={{ color: projectColor }} />
              )}
            </div>
            <div className="flex-1 min-w-0">
              <h3 className="text-lg font-semibold text-white group-hover/link:text-cyan-400 transition-colors truncate">
                {project.name}
              </h3>
              {hasGitHub ? (
                <div className="flex items-center gap-2 text-sm text-slate-500">
                  <GitBranch className="w-3.5 h-3.5" />
                  <span className="truncate">
                    {project.github_owner}/{project.github_repo}
                  </span>
                </div>
              ) : (
                <div className="flex items-center gap-2 text-sm text-slate-500">
                  <Folder className="w-3.5 h-3.5" />
                  <span>Local Project</span>
                </div>
              )}
            </div>
          </Link>

          {/* Menu button */}
          <div className="relative">
            <button
              onClick={() => setMenuOpen(!menuOpen)}
              className="p-1.5 rounded-lg text-slate-500 hover:text-white hover:bg-slate-700 transition-colors"
            >
              <MoreVertical className="w-4 h-4" />
            </button>

            {menuOpen && (
              <>
                <div className="fixed inset-0 z-10" onClick={() => setMenuOpen(false)} />
                <div className="absolute right-0 top-full mt-1 w-48 bg-slate-800 border border-slate-700 rounded-lg shadow-xl z-20 py-1">
                  {hasGitHub && (
                    <>
                      <a
                        href={`https://github.com/${project.github_owner}/${project.github_repo}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="flex items-center gap-2 px-3 py-2 text-sm text-slate-300 hover:text-white hover:bg-slate-700/50"
                        onClick={() => setMenuOpen(false)}
                      >
                        <ExternalLink className="w-4 h-4" />
                        View on GitHub
                      </a>
                      <button
                        onClick={() => {
                          syncMutation.mutate();
                          setMenuOpen(false);
                        }}
                        className="w-full flex items-center gap-2 px-3 py-2 text-sm text-slate-300 hover:text-white hover:bg-slate-700/50"
                        disabled={syncMutation.isPending}
                      >
                        <RefreshCw className={clsx('w-4 h-4', syncMutation.isPending && 'animate-spin')} />
                        {syncMutation.isPending ? 'Syncing...' : 'Sync GitHub'}
                      </button>
                    </>
                  )}
                  <button
                    onClick={() => {
                      setMenuOpen(false);
                      // TODO: Archive functionality
                    }}
                    className="w-full flex items-center gap-2 px-3 py-2 text-sm text-slate-300 hover:text-white hover:bg-slate-700/50"
                  >
                    <Archive className="w-4 h-4" />
                    Archive
                  </button>
                  <div className="border-t border-slate-700 my-1" />
                  <button
                    onClick={() => {
                      if (confirm('Are you sure you want to delete this project?')) {
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

        {/* Description */}
        {project.description && (
          <p className="text-sm text-slate-400 mb-4 line-clamp-2">{project.description}</p>
        )}

        {/* Stats */}
        <div className="flex items-center gap-4 text-sm">
          <div className="flex items-center gap-1.5 text-slate-400">
            <div className="w-2 h-2 rounded-full bg-emerald-400" />
            <span>{project.stats?.open_issues || 0} open</span>
          </div>
          <div className="flex items-center gap-1.5 text-slate-400">
            <div className="w-2 h-2 rounded-full bg-blue-400" />
            <span>{project.stats?.in_progress_issues || 0} in progress</span>
          </div>
          <div className="flex items-center gap-1.5 text-slate-400">
            <FileText className="w-3.5 h-3.5" />
            <span>{project.stats?.linked_entries || 0} entries</span>
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between mt-4 pt-4 border-t border-slate-700/50">
          <span className="text-xs text-slate-500">
            Updated {formatRelativeTime(project.updated_at)}
          </span>
          <Link
            to={`/projects/${project.id}`}
            className="flex items-center gap-1 text-sm text-cyan-400 hover:text-cyan-300 opacity-0 group-hover:opacity-100 transition-all"
          >
            View project
            <ArrowRight className="w-4 h-4" />
          </Link>
        </div>
      </div>
    </div>
  );
}

function EmptyState({ onCreateClick }: { onCreateClick: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 px-4 text-center">
      <div className="w-20 h-20 rounded-2xl bg-slate-800/50 border border-slate-700 flex items-center justify-center mb-6">
        <FolderKanban className="w-10 h-10 text-slate-600" />
      </div>
      <h3 className="text-xl font-semibold text-white mb-2">No projects yet</h3>
      <p className="text-slate-400 max-w-md mb-6">
        Create your first project to start organizing issues and linking knowledge base entries.
        Each project connects to a GitHub repository for seamless issue tracking.
      </p>
      <button
        onClick={onCreateClick}
        className="flex items-center gap-2 px-5 py-2.5 bg-cyan-500 text-white rounded-lg hover:bg-cyan-400 transition-colors font-medium"
      >
        <Plus className="w-4 h-4" />
        Create Your First Project
      </button>
    </div>
  );
}

export default function ProjectList() {
  const [showCreateModal, setShowCreateModal] = useState(false);
  const queryClient = useQueryClient();

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['projects'],
    queryFn: () => projectApi.listProjects(),
  });

  // Subscribe to project events for real-time updates
  useProjectEventRefetch(undefined, {
    refetchProjects: refetch,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-[60vh]">
        <div className="flex flex-col items-center gap-4">
          <div className="w-12 h-12 border-4 border-cyan-500 border-t-transparent rounded-full animate-spin" />
          <p className="text-slate-400 font-mono">Loading projects...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="card p-8 text-center">
        <p className="text-red-400">Failed to load projects</p>
        <p className="text-slate-500 text-sm mt-2">{(error as Error).message}</p>
        <button
          onClick={() => refetch()}
          className="mt-4 px-4 py-2 text-sm bg-slate-700 text-white rounded-lg hover:bg-slate-600 transition-colors"
        >
          Try Again
        </button>
      </div>
    );
  }

  const projects = data?.projects || [];

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-white">Projects</h1>
          <p className="text-slate-400 mt-1">
            {projects.length} project{projects.length !== 1 ? 's' : ''}
          </p>
        </div>
        {projects.length > 0 && (
          <button
            onClick={() => setShowCreateModal(true)}
            className="flex items-center gap-2 px-4 py-2 bg-cyan-500 text-white rounded-lg hover:bg-cyan-400 transition-colors font-medium"
          >
            <Plus className="w-4 h-4" />
            New Project
          </button>
        )}
      </div>

      {/* Projects Grid or Empty State */}
      {projects.length > 0 ? (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {projects.map((project) => (
            <ProjectCard key={project.id} project={project} />
          ))}
        </div>
      ) : (
        <EmptyState onCreateClick={() => setShowCreateModal(true)} />
      )}

      {/* Create Project Wizard */}
      <CreateProjectWizard
        isOpen={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        onSuccess={() => queryClient.invalidateQueries({ queryKey: ['projects'] })}
      />
    </div>
  );
}
