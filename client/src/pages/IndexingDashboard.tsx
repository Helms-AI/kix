import { useState, useCallback, useRef } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Globe,
  Upload,
  Play,
  XCircle,
  CheckCircle,
  Clock,
  AlertCircle,
  RefreshCw,
  FileText,
  Folder,
  Trash2,
  ChevronDown,
  ChevronRight,
  Zap,
  Radio,
  Loader2,
  ExternalLink,
} from 'lucide-react';
import clsx from 'clsx';
import { useSSE, SSEEvent } from '../hooks/useSSE';
import { useJobEvents } from '../hooks/useJobEvents';
import {
  indexingApi,
  Job,
  fileToBase64,
  formatFileSize,
  FileUpload,
  EnhancedLiveJobData,
} from '../api/indexingClient';
import { ActiveJobCard } from '../components/indexing/ActiveJobCard';
import { DeleteAllDataModal } from '../components/DeleteAllDataModal';

// ============================================================================
// Status Badge Component
// ============================================================================
function StatusBadge({ status }: { status: Job['status'] }) {
  const config = {
    pending: { icon: Clock, class: 'bg-slate-700 text-slate-300', label: 'Pending' },
    queued: { icon: Clock, class: 'bg-amber-900/50 text-amber-400 border border-amber-700/50', label: 'Queued' },
    running: { icon: Loader2, class: 'bg-cyan-900/50 text-cyan-400 border border-cyan-700/50', label: 'Running' },
    completed: { icon: CheckCircle, class: 'bg-emerald-900/50 text-emerald-400 border border-emerald-700/50', label: 'Completed' },
    failed: { icon: AlertCircle, class: 'bg-red-900/50 text-red-400 border border-red-700/50', label: 'Failed' },
    cancelled: { icon: XCircle, class: 'bg-slate-700 text-slate-400 border border-slate-600', label: 'Cancelled' },
  };

  const { icon: Icon, class: className, label } = config[status];

  return (
    <span className={clsx('inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium font-mono', className)}>
      <Icon className={clsx('w-3 h-3', status === 'running' && 'animate-spin')} />
      {label}
    </span>
  );
}

// ============================================================================
// Progress Bar Component
// ============================================================================
function ProgressBar({
  percentage,
  animated = false,
  showGlow = false,
}: {
  percentage: number;
  animated?: boolean;
  showGlow?: boolean;
}) {
  return (
    <div className="relative h-2 bg-slate-800 rounded-full overflow-hidden">
      <div
        className={clsx(
          'h-full rounded-full transition-all duration-500 ease-out',
          'bg-gradient-to-r from-cyan-500 via-teal-400 to-cyan-500',
          animated && 'bg-[length:200%_100%] animate-shimmer',
          showGlow && 'shadow-[0_0_12px_rgba(34,211,238,0.5)]'
        )}
        style={{ width: `${Math.min(percentage, 100)}%` }}
      />
      {/* Scanline effect for active jobs */}
      {animated && (
        <div className="absolute inset-0 bg-gradient-to-b from-transparent via-white/5 to-transparent animate-scanline" />
      )}
    </div>
  );
}

// ============================================================================
// URL Indexing Form
// ============================================================================
function UrlIndexingForm({ onSuccess }: { onSuccess?: () => void }) {
  const [url, setUrl] = useState('');
  const [levels, setLevels] = useState(1);
  const [respectRobots, setRespectRobots] = useState(true);
  const [renderJs, setRenderJs] = useState(false);
  const [priority, setPriority] = useState(5);
  const [urlError, setUrlError] = useState('');

  const queryClient = useQueryClient();

  const mutation = useMutation({
    mutationFn: indexingApi.startUrlIndexing,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['jobs'] });
      setUrl('');
      onSuccess?.();
    },
  });

  const validateUrl = (value: string): boolean => {
    try {
      new URL(value);
      setUrlError('');
      return true;
    } catch {
      setUrlError('Please enter a valid URL');
      return false;
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!validateUrl(url)) return;
    mutation.mutate({
      url,
      levels,
      respect_robots: respectRobots,
      render_js: renderJs,
      priority,
    });
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-5">
      {/* URL Input */}
      <div>
        <label className="block text-sm font-medium text-slate-300 mb-2">
          URL to Index
        </label>
        <div className="relative">
          <Globe className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-slate-500" />
          <input
            type="text"
            value={url}
            onChange={(e) => {
              setUrl(e.target.value);
              if (urlError) validateUrl(e.target.value);
            }}
            placeholder="https://example.com/docs"
            className={clsx(
              'w-full pl-11 pr-4 py-3 bg-slate-800/50 border rounded-lg text-white placeholder-slate-500',
              'focus:outline-none focus:ring-2 focus:ring-cyan-500/50 focus:border-cyan-500',
              'transition-all duration-200 font-mono text-sm',
              urlError ? 'border-red-500' : 'border-slate-700'
            )}
          />
        </div>
        {urlError && (
          <p className="mt-1.5 text-xs text-red-400 flex items-center gap-1">
            <AlertCircle className="w-3 h-3" />
            {urlError}
          </p>
        )}
      </div>

      {/* Crawl Depth Slider */}
      <div>
        <div className="flex justify-between items-center mb-2">
          <label className="text-sm font-medium text-slate-300">
            Crawl Depth
          </label>
          <span className="text-sm font-mono text-cyan-400">{levels} level{levels !== 1 ? 's' : ''}</span>
        </div>
        <input
          type="range"
          min="0"
          max="5"
          value={levels}
          onChange={(e) => setLevels(parseInt(e.target.value))}
          className="w-full h-2 bg-slate-800 rounded-lg appearance-none cursor-pointer slider-thumb"
        />
        <div className="flex justify-between text-xs text-slate-600 mt-1 font-mono">
          <span>0 (URL only)</span>
          <span>5 (deep)</span>
        </div>
      </div>

      {/* Options Grid */}
      <div className="grid grid-cols-2 gap-4">
        <label className="flex items-center gap-3 p-3 bg-slate-800/30 rounded-lg border border-slate-700/50 cursor-pointer hover:border-slate-600 transition-colors">
          <input
            type="checkbox"
            checked={respectRobots}
            onChange={(e) => setRespectRobots(e.target.checked)}
            className="w-4 h-4 rounded border-slate-600 bg-slate-800 text-cyan-500 focus:ring-cyan-500 focus:ring-offset-0"
          />
          <div>
            <span className="text-sm text-slate-300 block">Respect robots.txt</span>
            <span className="text-xs text-slate-500">Follow crawl rules</span>
          </div>
        </label>

        <label className="flex items-center gap-3 p-3 bg-slate-800/30 rounded-lg border border-slate-700/50 cursor-pointer hover:border-slate-600 transition-colors">
          <input
            type="checkbox"
            checked={renderJs}
            onChange={(e) => setRenderJs(e.target.checked)}
            className="w-4 h-4 rounded border-slate-600 bg-slate-800 text-cyan-500 focus:ring-cyan-500 focus:ring-offset-0"
          />
          <div>
            <span className="text-sm text-slate-300 block">Render JavaScript</span>
            <span className="text-xs text-slate-500">Use headless browser</span>
          </div>
        </label>
      </div>

      {/* Priority Slider */}
      <div>
        <div className="flex justify-between items-center mb-2">
          <label className="text-sm font-medium text-slate-300">Priority</label>
          <span className="text-sm font-mono text-cyan-400">{priority}</span>
        </div>
        <input
          type="range"
          min="1"
          max="10"
          value={priority}
          onChange={(e) => setPriority(parseInt(e.target.value))}
          className="w-full h-2 bg-slate-800 rounded-lg appearance-none cursor-pointer slider-thumb"
        />
        <div className="flex justify-between text-xs text-slate-600 mt-1 font-mono">
          <span>1 (low)</span>
          <span>10 (urgent)</span>
        </div>
      </div>

      {/* Submit Button */}
      <button
        type="submit"
        disabled={mutation.isPending || !url}
        className={clsx(
          'w-full flex items-center justify-center gap-2 py-3 px-4 rounded-lg font-medium transition-all duration-200',
          'bg-gradient-to-r from-cyan-600 to-teal-600 hover:from-cyan-500 hover:to-teal-500',
          'text-white shadow-lg shadow-cyan-500/20 hover:shadow-cyan-500/30',
          'disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:shadow-cyan-500/20'
        )}
      >
        {mutation.isPending ? (
          <>
            <Loader2 className="w-5 h-5 animate-spin" />
            Starting...
          </>
        ) : (
          <>
            <Play className="w-5 h-5" />
            Start Indexing
          </>
        )}
      </button>

      {mutation.isError && (
        <p className="text-sm text-red-400 flex items-center gap-2">
          <AlertCircle className="w-4 h-4" />
          {(mutation.error as Error).message}
        </p>
      )}
    </form>
  );
}

// ============================================================================
// File Upload Form
// ============================================================================
function FileUploadForm({ onSuccess }: { onSuccess?: () => void }) {
  const [files, setFiles] = useState<File[]>([]);
  const [extractArchives, setExtractArchives] = useState(true);
  const [priority, setPriority] = useState(5);
  const [isDragging, setIsDragging] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const queryClient = useQueryClient();

  const mutation = useMutation({
    mutationFn: async (files: File[]) => {
      const fileUploads: FileUpload[] = await Promise.all(
        files.map(async (file) => ({
          name: file.name,
          content: await fileToBase64(file),
        }))
      );
      return indexingApi.startFileIndexing({
        files: fileUploads,
        extract_archives: extractArchives,
        priority,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['jobs'] });
      setFiles([]);
      onSuccess?.();
    },
  });

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    const droppedFiles = Array.from(e.dataTransfer.files);
    setFiles((prev) => [...prev, ...droppedFiles]);
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  }, []);

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files) {
      setFiles((prev) => [...prev, ...Array.from(e.target.files!)]);
    }
  };

  const removeFile = (index: number) => {
    setFiles((prev) => prev.filter((_, i) => i !== index));
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (files.length === 0) return;
    mutation.mutate(files);
  };

  const totalSize = files.reduce((acc, f) => acc + f.size, 0);

  return (
    <form onSubmit={handleSubmit} className="space-y-5">
      {/* Drop Zone */}
      <div
        onDrop={handleDrop}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onClick={() => fileInputRef.current?.click()}
        className={clsx(
          'relative border-2 border-dashed rounded-xl p-8 text-center cursor-pointer transition-all duration-200',
          isDragging
            ? 'border-cyan-500 bg-cyan-500/10'
            : 'border-slate-700 hover:border-slate-600 bg-slate-800/30'
        )}
      >
        <input
          ref={fileInputRef}
          type="file"
          multiple
          onChange={handleFileSelect}
          className="hidden"
          accept=".pdf,.docx,.doc,.txt,.md,.html,.htm,.json,.xml,.csv,.xlsx,.xls,.py,.js,.ts,.jsx,.tsx,.rs,.java,.go,.cpp,.c,.h,.zip"
        />
        <div className={clsx(
          'w-14 h-14 mx-auto mb-4 rounded-xl flex items-center justify-center transition-colors',
          isDragging ? 'bg-cyan-500/20 text-cyan-400' : 'bg-slate-700 text-slate-400'
        )}>
          <Upload className="w-7 h-7" />
        </div>
        <p className="text-slate-300 font-medium mb-1">
          {isDragging ? 'Drop files here' : 'Drag & drop files here'}
        </p>
        <p className="text-sm text-slate-500">
          or click to browse
        </p>
        <p className="text-xs text-slate-600 mt-2 font-mono">
          PDF, DOCX, TXT, MD, HTML, JSON, CSV, code files, ZIP archives
        </p>
      </div>

      {/* File List */}
      {files.length > 0 && (
        <div className="space-y-2">
          <div className="flex justify-between items-center text-sm">
            <span className="text-slate-400">{files.length} file{files.length !== 1 ? 's' : ''} selected</span>
            <span className="text-slate-500 font-mono">{formatFileSize(totalSize)}</span>
          </div>
          <div className="max-h-40 overflow-y-auto space-y-1.5 scrollbar-thin">
            {files.map((file, index) => (
              <div
                key={`${file.name}-${index}`}
                className="flex items-center gap-3 p-2.5 bg-slate-800/50 rounded-lg group"
              >
                <div className="p-1.5 bg-slate-700 rounded">
                  {file.name.endsWith('.zip') ? (
                    <Folder className="w-4 h-4 text-amber-400" />
                  ) : (
                    <FileText className="w-4 h-4 text-cyan-400" />
                  )}
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm text-slate-300 truncate font-mono">{file.name}</p>
                  <p className="text-xs text-slate-500">{formatFileSize(file.size)}</p>
                </div>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    removeFile(index);
                  }}
                  className="p-1.5 text-slate-500 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-all"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Extract Archives Option */}
      <label className="flex items-center gap-3 p-3 bg-slate-800/30 rounded-lg border border-slate-700/50 cursor-pointer hover:border-slate-600 transition-colors">
        <input
          type="checkbox"
          checked={extractArchives}
          onChange={(e) => setExtractArchives(e.target.checked)}
          className="w-4 h-4 rounded border-slate-600 bg-slate-800 text-cyan-500 focus:ring-cyan-500 focus:ring-offset-0"
        />
        <div>
          <span className="text-sm text-slate-300 block">Extract ZIP archives</span>
          <span className="text-xs text-slate-500">Index contents of ZIP files</span>
        </div>
      </label>

      {/* Priority Slider */}
      <div>
        <div className="flex justify-between items-center mb-2">
          <label className="text-sm font-medium text-slate-300">Priority</label>
          <span className="text-sm font-mono text-cyan-400">{priority}</span>
        </div>
        <input
          type="range"
          min="1"
          max="10"
          value={priority}
          onChange={(e) => setPriority(parseInt(e.target.value))}
          className="w-full h-2 bg-slate-800 rounded-lg appearance-none cursor-pointer slider-thumb"
        />
      </div>

      {/* Submit Button */}
      <button
        type="submit"
        disabled={mutation.isPending || files.length === 0}
        className={clsx(
          'w-full flex items-center justify-center gap-2 py-3 px-4 rounded-lg font-medium transition-all duration-200',
          'bg-gradient-to-r from-violet-600 to-purple-600 hover:from-violet-500 hover:to-purple-500',
          'text-white shadow-lg shadow-violet-500/20 hover:shadow-violet-500/30',
          'disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:shadow-violet-500/20'
        )}
      >
        {mutation.isPending ? (
          <>
            <Loader2 className="w-5 h-5 animate-spin" />
            Uploading...
          </>
        ) : (
          <>
            <Upload className="w-5 h-5" />
            Upload & Index
          </>
        )}
      </button>

      {mutation.isError && (
        <p className="text-sm text-red-400 flex items-center gap-2">
          <AlertCircle className="w-4 h-4" />
          {(mutation.error as Error).message}
        </p>
      )}
    </form>
  );
}


// ============================================================================
// Job History Row
// ============================================================================
function JobHistoryRow({ job, onCancel }: { job: Job; onCancel: (id: string) => void }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="border-b border-slate-800 last:border-0">
      <div
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-4 py-3 px-4 hover:bg-slate-800/30 cursor-pointer transition-colors"
      >
        <button className="text-slate-500">
          {expanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
        </button>
        <div className="flex-1 grid grid-cols-5 gap-4 items-center">
          <span className="font-mono text-sm text-slate-400 truncate">{job.id.slice(0, 8)}...</span>
          <span className="text-sm text-slate-300 capitalize flex items-center gap-2">
            {job.job_type === 'url' ? <Globe className="w-4 h-4 text-cyan-400" /> : <FileText className="w-4 h-4 text-violet-400" />}
            {job.job_type.replace('_', ' ')}
          </span>
          <StatusBadge status={job.status} />
          <span className="text-sm text-slate-500 font-mono">
            {new Date(job.created_at).toLocaleTimeString()}
          </span>
          <div className="w-full">
            {job.progress && (
              <div className="flex items-center gap-2">
                <ProgressBar percentage={job.progress.percentage} />
                <span className="text-xs text-slate-500 font-mono w-12">
                  {job.progress.percentage.toFixed(0)}%
                </span>
              </div>
            )}
          </div>
        </div>
      </div>
      {expanded && (
        <div className="px-12 pb-4 space-y-2">
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <span className="text-slate-500">Job ID:</span>
              <span className="ml-2 font-mono text-slate-300">{job.id}</span>
            </div>
            <div>
              <span className="text-slate-500">Created:</span>
              <span className="ml-2 text-slate-300">{new Date(job.created_at).toLocaleString()}</span>
            </div>
          </div>
          {(job.status === 'running' || job.status === 'queued') && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onCancel(job.id);
              }}
              className="flex items-center gap-2 px-3 py-1.5 text-sm text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
            >
              <XCircle className="w-4 h-4" />
              Cancel Job
            </button>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Main Dashboard Component
// ============================================================================
export default function IndexingDashboard() {
  const [activeTab, setActiveTab] = useState<'url' | 'files'>('url');
  const [liveJobData, setLiveJobData] = useState<Record<string, EnhancedLiveJobData>>({});
  const [expandedJobId, setExpandedJobId] = useState<string | null>(null);
  const [showDeleteModal, setShowDeleteModal] = useState(false);

  const queryClient = useQueryClient();
  const { processEvent, clearJob } = useJobEvents();

  // Fetch jobs
  const { data: jobsData, isLoading, isError } = useQuery({
    queryKey: ['jobs'],
    queryFn: () => indexingApi.listJobs({ limit: 50 }),
    refetchInterval: 5000,
    retry: 2,
  });

  // Cancel mutation
  const cancelMutation = useMutation({
    mutationFn: indexingApi.cancelJob,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['jobs'] });
    },
  });

  // SSE connection for real-time updates
  // In development, connect directly to backend to avoid Vite proxy SSE issues
  const sseUrl = import.meta.env.DEV
    ? 'http://localhost:3001/api/indexing/sse'
    : '/api/indexing/sse';

  const { isConnected, reconnect } = useSSE({
    url: sseUrl,
    onEvent: (event: SSEEvent) => {
      // Debug: log transformed events
      if (import.meta.env.DEV) {
        console.log('[Dashboard] Received event:', event.event_type, event.job_id, event);
      }

      // Process event through the job events hook
      const updatedJobData = processEvent(event);

      if (import.meta.env.DEV && updatedJobData) {
        console.log('[Dashboard] Updated job data:', event.job_id, updatedJobData);
      }

      if (updatedJobData) {
        setLiveJobData((prev) => ({
          ...prev,
          [event.job_id]: updatedJobData,
        }));
      }

      // Handle job completion/cancellation
      if (event.event_type === 'job_completed' || event.event_type === 'job_cancelled') {
        queryClient.invalidateQueries({ queryKey: ['jobs'] });
        clearJob(event.job_id);
        setLiveJobData((prev) => {
          const { [event.job_id]: _, ...rest } = prev;
          return rest;
        });
      }
    },
  });

  // Toggle expansion (accordion behavior - only one expanded at a time)
  const handleToggleExpand = useCallback((jobId: string) => {
    setExpandedJobId((prev) => (prev === jobId ? null : jobId));
  }, []);

  // Clear log for a job
  const handleClearLog = useCallback((jobId: string) => {
    setLiveJobData((prev) => {
      if (!prev[jobId]) return prev;
      return {
        ...prev,
        [jobId]: {
          ...prev[jobId],
          log: [],
        },
      };
    });
  }, []);

  // Handle delete all indexed data
  const handleDeleteAll = useCallback(async () => {
    const result = await indexingApi.deleteAllData();
    queryClient.invalidateQueries({ queryKey: ['stats'] });
    queryClient.invalidateQueries({ queryKey: ['jobs'] });
    return result;
  }, [queryClient]);

  const jobs = jobsData?.jobs || [];
  const activeJobs = jobs.filter((j) => j.status === 'running' || j.status === 'queued');
  const historyJobs = jobs.filter((j) => j.status !== 'running' && j.status !== 'queued');

  return (
    <div className="space-y-8">
      {/* Backend Connection Error Banner */}
      {isError && (
        <div className="bg-red-900/30 border border-red-700/50 rounded-xl p-4">
          <div className="flex items-center gap-3">
            <AlertCircle className="w-5 h-5 text-red-400 flex-shrink-0" />
            <div>
              <h3 className="text-red-400 font-semibold">Backend Connection Error</h3>
              <p className="text-red-300/70 text-sm mt-1">
                Unable to connect to the API server. Make sure the backend is running on port 3001.
              </p>
              <code className="text-xs text-slate-400 mt-2 block font-mono">
                Run: cargo run -- api --port 3001
              </code>
            </div>
          </div>
        </div>
      )}

      {/* Header */}
      <div className="flex items-start justify-between">
        <div>
          <h1 className="text-3xl font-bold text-white flex items-center gap-3">
            <div className="p-2 rounded-lg bg-gradient-to-br from-cyan-500 to-teal-500">
              <Zap className="w-6 h-6 text-white" />
            </div>
            Indexing Dashboard
          </h1>
          <p className="text-slate-400 mt-2">Index URLs and files into the knowledge system</p>
        </div>
        <div className="flex items-center gap-3">
          <div className={clsx(
            'flex items-center gap-2 px-3 py-1.5 rounded-full text-sm font-mono',
            isConnected
              ? 'bg-emerald-900/30 text-emerald-400 border border-emerald-700/50'
              : 'bg-red-900/30 text-red-400 border border-red-700/50'
          )}>
            <div className={clsx(
              'w-2 h-2 rounded-full',
              isConnected ? 'bg-emerald-400 animate-pulse' : 'bg-red-400'
            )} />
            {isConnected ? 'Live' : 'Disconnected'}
          </div>
          {!isConnected && (
            <button
              onClick={reconnect}
              className="p-2 text-slate-400 hover:text-cyan-400 hover:bg-slate-800 rounded-lg transition-colors"
              title="Reconnect"
            >
              <RefreshCw className="w-5 h-5" />
            </button>
          )}
          <button
            onClick={() => setShowDeleteModal(true)}
            className="flex items-center gap-2 px-3 py-1.5 text-sm text-red-400 hover:text-red-300 hover:bg-red-500/10 rounded-lg border border-red-500/20 hover:border-red-500/40 transition-all"
            title="Delete all indexed data"
          >
            <Trash2 className="w-4 h-4" />
            <span className="hidden sm:inline">Clear Index</span>
          </button>
        </div>
      </div>

      {/* Main Grid */}
      <div className="grid grid-cols-1 xl:grid-cols-3 gap-6">
        {/* Left Column - Input Forms */}
        <div className="xl:col-span-1">
          <div className="card p-6 sticky top-8">
            {/* Tab Switcher */}
            <div className="flex bg-slate-800/50 rounded-lg p-1 mb-6">
              <button
                onClick={() => setActiveTab('url')}
                className={clsx(
                  'flex-1 flex items-center justify-center gap-2 py-2.5 px-4 rounded-md text-sm font-medium transition-all',
                  activeTab === 'url'
                    ? 'bg-gradient-to-r from-cyan-600 to-teal-600 text-white shadow-lg'
                    : 'text-slate-400 hover:text-white'
                )}
              >
                <Globe className="w-4 h-4" />
                URL
              </button>
              <button
                onClick={() => setActiveTab('files')}
                className={clsx(
                  'flex-1 flex items-center justify-center gap-2 py-2.5 px-4 rounded-md text-sm font-medium transition-all',
                  activeTab === 'files'
                    ? 'bg-gradient-to-r from-violet-600 to-purple-600 text-white shadow-lg'
                    : 'text-slate-400 hover:text-white'
                )}
              >
                <Upload className="w-4 h-4" />
                Files
              </button>
            </div>

            {/* Forms */}
            {activeTab === 'url' ? <UrlIndexingForm /> : <FileUploadForm />}
          </div>
        </div>

        {/* Right Column - Jobs */}
        <div className="xl:col-span-2 space-y-6">
          {/* Active Jobs */}
          <div className="card p-6">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-xl font-semibold text-white flex items-center gap-2">
                <Radio className="w-5 h-5 text-cyan-400" />
                Active Jobs
              </h2>
              <span className="text-sm text-slate-500 font-mono">
                {activeJobs.length} running
              </span>
            </div>

            {activeJobs.length === 0 ? (
              <div className="text-center py-12 text-slate-500">
                <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-slate-800 flex items-center justify-center">
                  <Clock className="w-8 h-8 text-slate-600" />
                </div>
                <p className="font-medium">No active jobs</p>
                <p className="text-sm mt-1">Start a new indexing job to see progress here</p>
              </div>
            ) : (
              <div className="space-y-3">
                {activeJobs.map((job) => (
                  <ActiveJobCard
                    key={job.id}
                    job={job}
                    liveData={liveJobData[job.id]}
                    isExpanded={expandedJobId === job.id}
                    onToggleExpand={() => handleToggleExpand(job.id)}
                    onCancel={(id) => cancelMutation.mutate(id)}
                    onClearLog={handleClearLog}
                  />
                ))}
              </div>
            )}
          </div>

          {/* Job History */}
          <div className="card">
            <div className="flex items-center justify-between p-6 border-b border-slate-800">
              <h2 className="text-xl font-semibold text-white flex items-center gap-2">
                <Clock className="w-5 h-5 text-slate-400" />
                Job History
              </h2>
              <span className="text-sm text-slate-500 font-mono">
                {historyJobs.length} jobs
              </span>
            </div>

            {isLoading ? (
              <div className="flex items-center justify-center py-12">
                <Loader2 className="w-8 h-8 text-cyan-400 animate-spin" />
              </div>
            ) : historyJobs.length === 0 ? (
              <div className="text-center py-12 text-slate-500">
                <p>No completed jobs yet</p>
              </div>
            ) : (
              <div className="divide-y divide-slate-800">
                {/* Table Header */}
                <div className="grid grid-cols-5 gap-4 py-2 px-12 text-xs font-medium text-slate-500 uppercase tracking-wider">
                  <span>Job ID</span>
                  <span>Type</span>
                  <span>Status</span>
                  <span>Created</span>
                  <span>Progress</span>
                </div>
                {/* Table Rows */}
                {historyJobs.slice(0, 10).map((job) => (
                  <JobHistoryRow
                    key={job.id}
                    job={job}
                    onCancel={(id) => cancelMutation.mutate(id)}
                  />
                ))}
              </div>
            )}

            {historyJobs.length > 10 && (
              <div className="p-4 text-center border-t border-slate-800">
                <button className="text-sm text-cyan-400 hover:text-cyan-300 font-medium flex items-center gap-2 mx-auto">
                  View all {historyJobs.length} jobs
                  <ExternalLink className="w-4 h-4" />
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Delete All Data Modal */}
      <DeleteAllDataModal
        isOpen={showDeleteModal}
        onClose={() => setShowDeleteModal(false)}
        onConfirm={handleDeleteAll}
      />
    </div>
  );
}
