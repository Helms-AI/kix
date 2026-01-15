import { useState, useCallback, useRef, useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Globe,
  Upload,
  Play,
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
  Plus,
  Activity,
  History,
} from 'lucide-react';
import clsx from 'clsx';
import { useSSEContext } from '../contexts/SSEContext';
import {
  indexingApi,
  fileToBase64,
  formatFileSize,
  FileUpload,
  PersistedJob,
  Job,
  EnhancedLiveJobData,
} from '../api/indexingClient';
import { ActiveJobCard } from '../components/indexing/ActiveJobCard';
import { HistoryJobCard } from '../components/indexing/HistoryJobCard';
import { DeleteAllDataModal } from '../components/DeleteAllDataModal';

// ============================================================================
// Types
// ============================================================================
type MainTab = 'index' | 'active' | 'history';
type IndexSubTab = 'url' | 'files';

const STORAGE_KEY = 'kix-indexing-tab';

// ============================================================================
// URL Indexing Form
// ============================================================================
function UrlIndexingForm({ onSuccess }: { onSuccess?: () => void }) {
  const [url, setUrl] = useState('');
  const [levels, setLevels] = useState(1);
  const [respectRobots, setRespectRobots] = useState(true);
  const [skipRender, setSkipRender] = useState(false);
  const [timeoutSecs, setTimeoutSecs] = useState(30);
  const [priority, setPriority] = useState(5);
  const [urlError, setUrlError] = useState('');
  const [maxPages, setMaxPages] = useState(0);
  const [showAdvanced, setShowAdvanced] = useState(false);

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
      skip_render: skipRender,
      timeout_secs: timeoutSecs,
      priority,
      max_pages: maxPages,
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

      {/* Advanced Settings Toggle */}
      <button
        type="button"
        onClick={() => setShowAdvanced(!showAdvanced)}
        className="flex items-center gap-2 text-sm text-slate-400 hover:text-cyan-400 transition-colors"
      >
        {showAdvanced ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
        Advanced Settings
      </button>

      {/* Advanced Settings Panel */}
      {showAdvanced && (
        <div className="space-y-4 p-4 bg-slate-800/30 rounded-lg border border-slate-700/50">
          {/* Options Grid */}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <label className="flex items-center gap-3 p-3 bg-slate-800/50 rounded-lg border border-slate-700/50 cursor-pointer hover:border-slate-600 transition-colors">
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

            <label className="flex items-center gap-3 p-3 bg-slate-800/50 rounded-lg border border-slate-700/50 cursor-pointer hover:border-slate-600 transition-colors">
              <input
                type="checkbox"
                checked={skipRender}
                onChange={(e) => setSkipRender(e.target.checked)}
                className="w-4 h-4 rounded border-slate-600 bg-slate-800 text-cyan-500 focus:ring-cyan-500 focus:ring-offset-0"
              />
              <div>
                <span className="text-sm text-slate-300 block">Skip JS rendering</span>
                <span className="text-xs text-slate-500">Faster, for static pages</span>
              </div>
            </label>
          </div>

          {/* Render Timeout (only show when JS rendering is enabled) */}
          {!skipRender && (
            <div>
              <div className="flex justify-between items-center mb-2">
                <label className="text-sm font-medium text-slate-300">
                  Render Timeout
                </label>
                <span className="text-sm font-mono text-cyan-400">{timeoutSecs}s</span>
              </div>
              <input
                type="range"
                min="10"
                max="120"
                step="5"
                value={timeoutSecs}
                onChange={(e) => setTimeoutSecs(parseInt(e.target.value))}
                className="w-full h-2 bg-slate-800 rounded-lg appearance-none cursor-pointer slider-thumb"
              />
              <div className="flex justify-between text-xs text-slate-600 mt-1 font-mono">
                <span>10s</span>
                <span>120s</span>
              </div>
            </div>
          )}

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

          {/* Max Pages */}
          <div>
            <div className="flex justify-between items-center mb-2">
              <label className="text-sm font-medium text-slate-300">
                Max Pages
              </label>
              <span className="text-sm font-mono text-cyan-400">
                {maxPages === 0 ? 'Unlimited' : maxPages}
              </span>
            </div>
            <input
              type="number"
              value={maxPages}
              onChange={(e) => setMaxPages(Math.max(0, parseInt(e.target.value) || 0))}
              min={0}
              max={10000}
              placeholder="1000"
              className="w-full px-4 py-2 bg-slate-800/50 border border-slate-700 rounded-lg text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 focus:border-cyan-500 font-mono text-sm"
            />
            <p className="text-xs text-slate-500 mt-1">
              Maximum pages to crawl. Set to 0 for unlimited.
            </p>
          </div>
        </div>
      )}

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
// Index Tab Content
// ============================================================================
function IndexTabContent() {
  const [subTab, setSubTab] = useState<IndexSubTab>('url');

  return (
    <div className="h-full flex flex-col">
      <div className="max-w-2xl mx-auto w-full px-4 sm:px-6 py-8">
        {/* Sub-tab Switcher (URL / Files) */}
        <div className="flex bg-slate-800/50 rounded-lg p-1 mb-8">
          <button
            onClick={() => setSubTab('url')}
            className={clsx(
              'flex-1 flex items-center justify-center gap-2 py-2.5 px-4 rounded-md text-sm font-medium transition-all',
              subTab === 'url'
                ? 'bg-gradient-to-r from-cyan-600 to-teal-600 text-white shadow-lg'
                : 'text-slate-400 hover:text-white'
            )}
          >
            <Globe className="w-4 h-4" />
            URL
          </button>
          <button
            onClick={() => setSubTab('files')}
            className={clsx(
              'flex-1 flex items-center justify-center gap-2 py-2.5 px-4 rounded-md text-sm font-medium transition-all',
              subTab === 'files'
                ? 'bg-gradient-to-r from-violet-600 to-purple-600 text-white shadow-lg'
                : 'text-slate-400 hover:text-white'
            )}
          >
            <Upload className="w-4 h-4" />
            Files
          </button>
        </div>

        {/* Form Content */}
        <div className="card p-6">
          {subTab === 'url' ? <UrlIndexingForm /> : <FileUploadForm />}
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Active Jobs Tab Content
// ============================================================================
function ActiveJobsTabContent({
  jobs,
  liveJobData,
  expandedJobId,
  onToggleExpand,
  onCancel,
  onClearLog,
}: {
  jobs: Job[];
  liveJobData: Record<string, EnhancedLiveJobData>;
  expandedJobId: string | null;
  onToggleExpand: (id: string) => void;
  onCancel: (id: string) => void;
  onClearLog: (id: string) => void;
}) {
  const activeJobs = jobs.filter((j) => j.status === 'running' || j.status === 'queued');

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Header */}
      <div className="flex-shrink-0 px-4 sm:px-6 py-4 border-b border-slate-800/50">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold text-white flex items-center gap-2">
            <Radio className="w-5 h-5 text-cyan-400" />
            Active Jobs
          </h2>
          <span className="text-sm text-slate-500 font-mono tabular-nums">
            {activeJobs.length} running
          </span>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-4 sm:px-6 py-4">
        {activeJobs.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full min-h-[300px] text-slate-500">
            <div className="w-20 h-20 mb-6 rounded-2xl bg-slate-800/80 flex items-center justify-center">
              <Clock className="w-10 h-10 text-slate-600" />
            </div>
            <p className="font-medium text-lg">No active jobs</p>
            <p className="text-sm mt-2 text-slate-600">Start a new indexing job to see progress here</p>
          </div>
        ) : (
          <div className="space-y-3 max-w-4xl mx-auto">
            {activeJobs.map((job) => (
              <ActiveJobCard
                key={job.id}
                job={job}
                liveData={liveJobData[job.id]}
                isExpanded={expandedJobId === job.id}
                onToggleExpand={() => onToggleExpand(job.id)}
                onCancel={(id) => onCancel(id)}
                onClearLog={onClearLog}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// History Tab Content
// ============================================================================
function HistoryTabContent({
  persistedJobs,
  isLoading,
  total,
  expandedJobId,
  onToggleExpand,
}: {
  persistedJobs: PersistedJob[];
  isLoading: boolean;
  total: number;
  expandedJobId: string | null;
  onToggleExpand: (id: string) => void;
}) {
  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Header */}
      <div className="flex-shrink-0 px-4 sm:px-6 py-4 border-b border-slate-800/50">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold text-white flex items-center gap-2">
            <History className="w-5 h-5 text-slate-400" />
            Job History
          </h2>
          <span className="text-sm text-slate-500 font-mono tabular-nums">
            {total} jobs
          </span>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-4 sm:px-6 py-4">
        {isLoading ? (
          <div className="flex items-center justify-center h-full min-h-[300px]">
            <Loader2 className="w-10 h-10 text-cyan-400 animate-spin" />
          </div>
        ) : persistedJobs.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full min-h-[300px] text-slate-500">
            <div className="w-20 h-20 mb-6 rounded-2xl bg-slate-800/80 flex items-center justify-center">
              <CheckCircle className="w-10 h-10 text-slate-600" />
            </div>
            <p className="font-medium text-lg">No completed jobs yet</p>
            <p className="text-sm mt-2 text-slate-600">Completed jobs will appear here</p>
          </div>
        ) : (
          <div className="space-y-3 max-w-4xl mx-auto">
            {persistedJobs.map((job) => (
              <HistoryJobCard
                key={job.job_id}
                job={job}
                isExpanded={expandedJobId === job.job_id}
                onToggleExpand={() => onToggleExpand(job.job_id)}
              />
            ))}

            {persistedJobs.length > 10 && (
              <div className="pt-4 text-center border-t border-slate-800 mt-4">
                <button className="text-sm text-cyan-400 hover:text-cyan-300 font-medium flex items-center gap-2 mx-auto">
                  View all {total} jobs
                  <ExternalLink className="w-4 h-4" />
                </button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Main Dashboard Component
// ============================================================================
export default function IndexingDashboard() {
  // Load initial tab from localStorage
  const [activeTab, setActiveTab] = useState<MainTab>(() => {
    if (typeof window === 'undefined') return 'index';
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === 'index' || stored === 'active' || stored === 'history') {
      return stored;
    }
    return 'index';
  });

  const [expandedJobId, setExpandedJobId] = useState<string | null>(null);
  const [expandedHistoryJobId, setExpandedHistoryJobId] = useState<string | null>(null);
  const [showDeleteModal, setShowDeleteModal] = useState(false);

  const queryClient = useQueryClient();

  // Persist tab selection
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, activeTab);
  }, [activeTab]);

  // Use global SSE context for real-time updates
  const { isConnected, reconnect, jobs: liveJobData, clearJobLog } = useSSEContext();

  // Fetch active jobs
  const { data: jobsData, isError } = useQuery({
    queryKey: ['jobs'],
    queryFn: () => indexingApi.listJobs({ limit: 50 }),
    refetchInterval: 5000,
    retry: 2,
  });

  // Fetch persisted job history
  const { data: historyData, isLoading: isHistoryLoading } = useQuery({
    queryKey: ['job-history'],
    queryFn: () => indexingApi.getJobHistory({ limit: 50 }),
    refetchInterval: 30000,
    retry: 2,
  });

  // Cancel mutation
  const cancelMutation = useMutation({
    mutationFn: indexingApi.cancelJob,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['jobs'] });
    },
  });

  // Toggle expansion (accordion behavior)
  const handleToggleExpand = useCallback((jobId: string) => {
    setExpandedJobId((prev) => (prev === jobId ? null : jobId));
  }, []);

  const handleToggleHistoryExpand = useCallback((jobId: string) => {
    setExpandedHistoryJobId((prev) => (prev === jobId ? null : jobId));
  }, []);

  // Handle delete all indexed data
  const handleDeleteAll = useCallback(async () => {
    const result = await indexingApi.deleteAllData();
    queryClient.invalidateQueries({ queryKey: ['stats'] });
    queryClient.invalidateQueries({ queryKey: ['jobs'] });
    queryClient.invalidateQueries({ queryKey: ['job-history'] });
    return result;
  }, [queryClient]);

  const jobs = jobsData?.jobs || [];
  const activeJobs = jobs.filter((j) => j.status === 'running' || j.status === 'queued');
  const persistedJobs: PersistedJob[] = historyData?.jobs || [];

  // Tab configuration
  const tabs: Array<{
    id: MainTab;
    label: string;
    icon: React.ElementType;
    badge?: number;
  }> = [
    { id: 'index', label: 'Index', icon: Plus },
    { id: 'active', label: 'Active', icon: Activity, badge: activeJobs.length > 0 ? activeJobs.length : undefined },
    { id: 'history', label: 'History', icon: History },
  ];

  return (
    <div className="h-full flex flex-col">
      {/* Backend Connection Error Banner */}
      {isError && (
        <div className="flex-shrink-0 mx-4 mt-4 bg-red-900/30 border border-red-700/50 rounded-xl p-4">
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
      <div className="flex-shrink-0 px-4 sm:px-6 py-6">
        <div className="flex items-start justify-between">
          <div>
            <h1 className="text-2xl sm:text-3xl font-bold text-white flex items-center gap-3">
              <div className="p-2 rounded-lg bg-gradient-to-br from-cyan-500 to-teal-500">
                <Zap className="w-5 h-5 sm:w-6 sm:h-6 text-white" />
              </div>
              Indexing Dashboard
            </h1>
            <p className="text-slate-400 mt-2 text-sm sm:text-base">Index URLs and files into the knowledge system</p>
          </div>
          <div className="flex items-center gap-2 sm:gap-3">
            <div className={clsx(
              'flex items-center gap-2 px-2.5 sm:px-3 py-1.5 rounded-full text-xs sm:text-sm font-mono',
              isConnected
                ? 'bg-emerald-900/30 text-emerald-400 border border-emerald-700/50'
                : 'bg-red-900/30 text-red-400 border border-red-700/50'
            )}>
              <div className={clsx(
                'w-2 h-2 rounded-full',
                isConnected ? 'bg-emerald-400 animate-pulse' : 'bg-red-400'
              )} />
              <span className="hidden sm:inline">{isConnected ? 'Live' : 'Disconnected'}</span>
            </div>
            {!isConnected && (
              <button
                onClick={reconnect}
                className="p-2 text-slate-400 hover:text-cyan-400 hover:bg-slate-800 rounded-lg transition-colors"
                title="Reconnect"
              >
                <RefreshCw className="w-4 h-4 sm:w-5 sm:h-5" />
              </button>
            )}
            <button
              onClick={() => setShowDeleteModal(true)}
              className="flex items-center gap-2 px-2.5 sm:px-3 py-1.5 text-xs sm:text-sm text-red-400 hover:text-red-300 hover:bg-red-500/10 rounded-lg border border-red-500/20 hover:border-red-500/40 transition-all"
              title="Delete all indexed data"
            >
              <Trash2 className="w-4 h-4" />
              <span className="hidden sm:inline">Clear Index</span>
            </button>
          </div>
        </div>
      </div>

      {/* Tab Navigation */}
      <div className="flex-shrink-0 px-4 sm:px-6">
        <nav className="flex flex-col sm:flex-row border-b border-slate-700/50">
          {tabs.map((tab) => {
            const Icon = tab.icon;
            const isActive = activeTab === tab.id;

            return (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={clsx(
                  'group relative flex items-center justify-center sm:justify-start gap-2 px-5 py-3.5 text-sm font-medium transition-all duration-200',
                  'border-b-2 sm:border-b-2 -mb-px',
                  isActive
                    ? 'text-cyan-400 border-cyan-400'
                    : 'text-slate-400 border-transparent hover:text-white hover:border-slate-600'
                )}
              >
                <Icon className={clsx(
                  'w-4 h-4 transition-colors',
                  isActive ? 'text-cyan-400' : 'text-slate-500 group-hover:text-slate-300'
                )} />
                <span>{tab.label}</span>
                {tab.badge !== undefined && (
                  <span className={clsx(
                    'ml-1.5 px-1.5 py-0.5 text-xs font-mono rounded-full',
                    isActive
                      ? 'bg-cyan-400/20 text-cyan-300'
                      : 'bg-slate-700 text-slate-400'
                  )}>
                    {tab.badge}
                  </span>
                )}
                {/* Active indicator glow */}
                {isActive && (
                  <span className="absolute bottom-0 left-0 right-0 h-0.5 bg-cyan-400 shadow-[0_0_8px_rgba(34,211,238,0.6)]" />
                )}
              </button>
            );
          })}
        </nav>
      </div>

      {/* Tab Content */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {activeTab === 'index' && <IndexTabContent />}
        {activeTab === 'active' && (
          <ActiveJobsTabContent
            jobs={jobs}
            liveJobData={liveJobData}
            expandedJobId={expandedJobId}
            onToggleExpand={handleToggleExpand}
            onCancel={(id) => cancelMutation.mutate(id)}
            onClearLog={clearJobLog}
          />
        )}
        {activeTab === 'history' && (
          <HistoryTabContent
            persistedJobs={persistedJobs}
            isLoading={isHistoryLoading}
            total={historyData?.total ?? persistedJobs.length}
            expandedJobId={expandedHistoryJobId}
            onToggleExpand={handleToggleHistoryExpand}
          />
        )}
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
