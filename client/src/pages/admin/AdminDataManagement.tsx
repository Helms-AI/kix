import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import clsx from 'clsx';
import {
  Trash2,
  RefreshCw,
  Database,
  AlertTriangle,
  Download,
  Upload,
  X,
  FileText,
  Layers,
  HardDrive,
  Globe,
  Calendar,
  Filter,
  Loader2,
  Shield,
  LayoutDashboard,
  Search
} from 'lucide-react';
import DataExplorer from './DataExplorer';

interface DataStats {
  total_documents: number;
  total_chunks: number;
  total_pages: number;
  unique_domains: number;
  storage_size_mb: number;
  last_updated: string;
}

interface ConfirmModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  message: string;
  confirmText?: string;
  isDangerous?: boolean;
  isLoading?: boolean;
}

function ConfirmModal({
  isOpen,
  onClose,
  onConfirm,
  title,
  message,
  confirmText = 'Confirm',
  isDangerous = false,
  isLoading = false,
}: ConfirmModalProps) {
  if (!isOpen) return null;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className={clsx('modal-content', isDangerous && 'modal-danger')}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Icon Header */}
        <div className={clsx(
          'px-6 py-5 flex items-center gap-4',
          isDangerous ? 'bg-red-500/5' : 'bg-cyan-500/5'
        )}>
          <div className={clsx(
            'w-12 h-12 flex items-center justify-center',
            isDangerous ? 'bg-red-500/20' : 'bg-cyan-500/20'
          )}>
            {isDangerous ? (
              <AlertTriangle className="w-6 h-6 text-red-400" />
            ) : (
              <RefreshCw className="w-6 h-6 text-cyan-400" />
            )}
          </div>
          <div className="flex-1">
            <h3 className="text-lg font-semibold text-white">{title}</h3>
          </div>
          <button
            onClick={onClose}
            className="p-2 text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Body */}
        <div className="px-6 py-5">
          <p className="text-slate-300 leading-relaxed">{message}</p>
        </div>

        {/* Actions */}
        <div className="px-6 py-4 bg-slate-800/50 flex justify-end gap-3">
          <button
            onClick={onClose}
            disabled={isLoading}
            className="btn-admin btn-admin-ghost"
          >
            Cancel
          </button>
          <button
            onClick={() => {
              onConfirm();
            }}
            disabled={isLoading}
            className={clsx(
              'btn-admin flex items-center gap-2',
              isDangerous ? 'btn-admin-danger' : 'btn-admin-primary'
            )}
          >
            {isLoading && <Loader2 className="w-4 h-4 animate-spin" />}
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}

interface StatCardProps {
  label: string;
  value: string | number;
  icon: React.ComponentType<{ className?: string }>;
}

function StatCard({ label, value, icon: Icon }: StatCardProps) {
  return (
    <div className="bg-slate-800/40 p-4 border border-slate-700/50">
      <div className="flex items-center gap-3 mb-2">
        <Icon className="w-4 h-4 text-slate-400" />
        <span className="text-sm text-slate-400">{label}</span>
      </div>
      <p className="text-2xl font-bold text-white font-mono-data">{value}</p>
    </div>
  );
}

export default function AdminDataManagement() {
  const queryClient = useQueryClient();
  const [activeTab, setActiveTab] = useState<'overview' | 'explorer'>('overview');
  const [confirmModal, setConfirmModal] = useState<{
    isOpen: boolean;
    action: 'clearAll' | 'clearCache' | 'reindex' | null;
  }>({ isOpen: false, action: null });
  const [domainToDelete, setDomainToDelete] = useState('');
  const [dateRange, setDateRange] = useState({ start: '', end: '' });
  const [sourceType, setSourceType] = useState('');

  // Mock data stats
  const dataStats: DataStats = {
    total_documents: 1234,
    total_chunks: 5678,
    total_pages: 890,
    unique_domains: 15,
    storage_size_mb: 234.5,
    last_updated: new Date().toLocaleString(),
  };

  const clearAllData = useMutation({
    mutationFn: async () => {
      await new Promise((resolve) => setTimeout(resolve, 2000));
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin'] });
      setConfirmModal({ isOpen: false, action: null });
    },
  });

  const invalidateCaches = useMutation({
    mutationFn: async () => {
      const response = await fetch('/api/admin/cache/invalidate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
      });
      if (!response.ok) throw new Error('Failed to invalidate caches');
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [] });
      setConfirmModal({ isOpen: false, action: null });
    },
  });

  const handleAction = (action: 'clearAll' | 'clearCache' | 'reindex') => {
    setConfirmModal({ isOpen: true, action });
  };

  const executeAction = async () => {
    switch (confirmModal.action) {
      case 'clearAll':
        await clearAllData.mutateAsync();
        break;
      case 'clearCache':
        await invalidateCaches.mutateAsync();
        break;
      case 'reindex':
        // Would implement reindex logic here
        setConfirmModal({ isOpen: false, action: null });
        break;
    }
  };

  const tabs = [
    { id: 'overview' as const, label: 'Overview', icon: LayoutDashboard },
    { id: 'explorer' as const, label: 'Explorer', icon: Search },
  ];

  // Calculate explorer height: viewport - admin header (~180px) - page header (~100px) - tabs (~50px) - padding (top + bottom)
  const explorerHeight = 'calc(100vh - 362px)';

  return (
    <div style={{ fontSize: 'calc(1rem + 2px)' }}>
      {/* Header */}
      <div className="flex items-center gap-4 mb-8">
        <div className="w-12 h-12 bg-gradient-to-br from-cyan-500/20 to-teal-500/20 border border-cyan-500/20 flex items-center justify-center">
          <Database className="w-6 h-6 text-cyan-400" />
        </div>
        <div>
          <h2 className="text-xl font-bold text-white">Data Management</h2>
          <p className="text-sm text-slate-400 mt-0.5">
            Manage your indexed content and storage
          </p>
        </div>
      </div>

      {/* Tab Navigation */}
      <div className="flex gap-2 p-1 bg-slate-800/40 backdrop-blur-sm border border-slate-700/50 border-b-0">
        {tabs.map(tab => {
          const Icon = tab.icon;
          return (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={clsx(
                'flex items-center gap-2 px-4 py-2 transition-all duration-300',
                activeTab === tab.id
                  ? 'bg-gradient-to-r from-cyan-500/20 to-teal-500/20 text-cyan-400 border border-cyan-500/30 shadow-lg shadow-cyan-950/30'
                  : 'text-slate-400 hover:text-white hover:bg-slate-700/50'
              )}
            >
              <Icon className="w-4 h-4" />
              <span className="font-medium font-ui">{tab.label}</span>
            </button>
          );
        })}
      </div>

      {/* Tab Content */}
      {activeTab === 'overview' ? (
        <div className="space-y-8 mt-8">
          {/* Statistics */}
          <div className="admin-card admin-card-glow p-6">
            <div className="relative z-10">
              <div className="section-header">
                <div className="section-header-icon">
                  <HardDrive className="w-5 h-5 text-cyan-400" />
                </div>
                <h3 className="text-lg font-semibold text-white">Index Statistics</h3>
              </div>

              <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
                <StatCard label="Documents" value={dataStats.total_documents.toLocaleString()} icon={FileText} />
                <StatCard label="Chunks" value={dataStats.total_chunks.toLocaleString()} icon={Layers} />
                <StatCard label="Pages" value={dataStats.total_pages.toLocaleString()} icon={FileText} />
                <StatCard label="Domains" value={dataStats.unique_domains} icon={Globe} />
                <StatCard label="Storage" value={`${dataStats.storage_size_mb} MB`} icon={HardDrive} />
                <StatCard label="Updated" value="Just now" icon={Calendar} />
              </div>
            </div>
          </div>

          {/* Index Operations */}
          <div className="admin-card admin-card-glow p-6">
            <div className="relative z-10">
              <div className="section-header">
                <div className="section-header-icon">
                  <RefreshCw className="w-5 h-5 text-cyan-400" />
                </div>
                <h3 className="text-lg font-semibold text-white">Index Operations</h3>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <button
                  onClick={() => handleAction('clearCache')}
                  className="group flex items-center gap-4 p-5 bg-slate-800/40 hover:bg-slate-800/60 border border-slate-700/50 hover:border-cyan-500/30 transition-all duration-300"
                >
                  <div className="w-12 h-12 bg-cyan-500/10 flex items-center justify-center group-hover:bg-cyan-500/20 transition-colors">
                    <RefreshCw className="w-6 h-6 text-cyan-400" />
                  </div>
                  <div className="text-left">
                    <p className="font-semibold text-white">Invalidate Caches</p>
                    <p className="text-sm text-slate-400 mt-0.5">Clear search and embedding caches</p>
                  </div>
                </button>

                <button
                  onClick={() => handleAction('reindex')}
                  className="group flex items-center gap-4 p-5 bg-slate-800/40 hover:bg-slate-800/60 border border-slate-700/50 hover:border-cyan-500/30 transition-all duration-300"
                >
                  <div className="w-12 h-12 bg-teal-500/10 flex items-center justify-center group-hover:bg-teal-500/20 transition-colors">
                    <Database className="w-6 h-6 text-teal-400" />
                  </div>
                  <div className="text-left">
                    <p className="font-semibold text-white">Reindex All</p>
                    <p className="text-sm text-slate-400 mt-0.5">Rebuild search index from scratch</p>
                  </div>
                </button>

                <button
                  disabled
                  className="group flex items-center gap-4 p-5 bg-slate-800/20 border border-slate-700/30 opacity-50 cursor-not-allowed"
                >
                  <div className="w-12 h-12 bg-slate-700/30 flex items-center justify-center">
                    <Download className="w-6 h-6 text-slate-500" />
                  </div>
                  <div className="text-left">
                    <p className="font-semibold text-slate-400">Export Data</p>
                    <p className="text-sm text-slate-500 mt-0.5">Coming soon</p>
                  </div>
                </button>

                <button
                  disabled
                  className="group flex items-center gap-4 p-5 bg-slate-800/20 border border-slate-700/30 opacity-50 cursor-not-allowed"
                >
                  <div className="w-12 h-12 bg-slate-700/30 flex items-center justify-center">
                    <Upload className="w-6 h-6 text-slate-500" />
                  </div>
                  <div className="text-left">
                    <p className="font-semibold text-slate-400">Import Data</p>
                    <p className="text-sm text-slate-500 mt-0.5">Coming soon</p>
                  </div>
                </button>
              </div>
            </div>
          </div>

          {/* Bulk Operations */}
          <div className="admin-card admin-card-glow p-6">
            <div className="relative z-10">
              <div className="section-header">
                <div className="section-header-icon">
                  <Filter className="w-5 h-5 text-cyan-400" />
                </div>
                <h3 className="text-lg font-semibold text-white">Bulk Operations</h3>
              </div>

              <div className="space-y-6">
                {/* Delete by Domain */}
                <div>
                  <label className="block text-sm font-medium text-white mb-3">
                    Delete by Domain
                  </label>
                  <div className="flex gap-3">
                    <input
                      type="text"
                      value={domainToDelete}
                      onChange={(e) => setDomainToDelete(e.target.value)}
                      placeholder="example.com"
                      className="admin-input flex-1"
                    />
                    <button
                      disabled={!domainToDelete}
                      className="btn-admin btn-admin-primary whitespace-nowrap disabled:opacity-50"
                    >
                      Delete Domain
                    </button>
                  </div>
                </div>

                {/* Delete by Date Range */}
                <div>
                  <label className="block text-sm font-medium text-white mb-3">
                    Delete by Date Range
                  </label>
                  <div className="flex flex-wrap gap-3 items-center">
                    <input
                      type="date"
                      value={dateRange.start}
                      onChange={(e) => setDateRange({ ...dateRange, start: e.target.value })}
                      className="admin-input"
                    />
                    <span className="text-slate-500">to</span>
                    <input
                      type="date"
                      value={dateRange.end}
                      onChange={(e) => setDateRange({ ...dateRange, end: e.target.value })}
                      className="admin-input"
                    />
                    <button
                      disabled={!dateRange.start || !dateRange.end}
                      className="btn-admin btn-admin-primary whitespace-nowrap disabled:opacity-50"
                    >
                      Delete Range
                    </button>
                  </div>
                </div>

                {/* Delete by Source Type */}
                <div>
                  <label className="block text-sm font-medium text-white mb-3">
                    Delete by Source Type
                  </label>
                  <div className="flex gap-3">
                    <select
                      value={sourceType}
                      onChange={(e) => setSourceType(e.target.value)}
                      className="admin-input flex-1"
                    >
                      <option value="">Select source type</option>
                      <option value="url">URL</option>
                      <option value="file">File Upload</option>
                      <option value="api">API</option>
                    </select>
                    <button
                      disabled={!sourceType}
                      className="btn-admin btn-admin-primary whitespace-nowrap disabled:opacity-50"
                    >
                      Delete by Type
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>

          {/* Danger Zone */}
          <div className="danger-zone">
            <div className="relative z-10">
              <div className="flex items-center gap-3 mb-4">
                <div className="w-10 h-10 bg-red-500/20 flex items-center justify-center">
                  <Shield className="w-5 h-5 text-red-400" />
                </div>
                <div>
                  <h3 className="text-lg font-semibold text-red-400">Danger Zone</h3>
                  <p className="text-sm text-red-400/70">
                    Irreversible actions that permanently delete data
                  </p>
                </div>
              </div>

              <div className="flex items-center justify-between p-4 bg-red-500/5 border border-red-500/20">
                <div>
                  <p className="font-medium text-white">Clear All Index Data</p>
                  <p className="text-sm text-slate-400 mt-0.5">
                    Permanently delete all documents, chunks, and embeddings
                  </p>
                </div>
                <button
                  onClick={() => handleAction('clearAll')}
                  className="btn-admin btn-admin-danger flex items-center gap-2"
                >
                  <Trash2 className="w-4 h-4" />
                  Clear All Data
                </button>
              </div>
            </div>
          </div>
        </div>
      ) : (
        <div style={{ height: explorerHeight }}>
          <DataExplorer />
        </div>
      )}

      {/* Confirmation Modal */}
      <ConfirmModal
        isOpen={confirmModal.isOpen}
        onClose={() => setConfirmModal({ isOpen: false, action: null })}
        onConfirm={executeAction}
        isLoading={clearAllData.isPending || invalidateCaches.isPending}
        title={
          confirmModal.action === 'clearAll'
            ? 'Clear All Data'
            : confirmModal.action === 'clearCache'
            ? 'Invalidate Caches'
            : 'Reindex Data'
        }
        message={
          confirmModal.action === 'clearAll'
            ? 'This will permanently delete all indexed documents, chunks, and embeddings. This action cannot be undone. Are you absolutely sure?'
            : confirmModal.action === 'clearCache'
            ? 'This will clear all cached search results and embeddings. The cache will be rebuilt automatically as new queries are made.'
            : 'This will rebuild the entire search index from source documents. This may take some time depending on the amount of data.'
        }
        confirmText={
          confirmModal.action === 'clearAll' ? 'Delete Everything' : 'Proceed'
        }
        isDangerous={confirmModal.action === 'clearAll'}
      />
    </div>
  );
}
