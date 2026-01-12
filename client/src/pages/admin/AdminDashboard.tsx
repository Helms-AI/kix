import { useQuery } from '@tanstack/react-query';
import clsx from 'clsx';
import {
  FileText,
  Layers,
  HardDrive,
  Cpu,
  Clock,
  ArrowUpRight,
  ArrowDownRight,
  Zap,
  RefreshCw,
  AlertTriangle,
  CheckCircle,
  Database,
  Activity
} from 'lucide-react';

interface SystemStats {
  total_entries: number;
  total_chunks: number;
  total_pages: number;
  unique_sources: number;
  total_embeddings: number;
  store_size_bytes: number;
  last_indexed?: string;
}

interface HealthStatus {
  status: 'healthy' | 'degraded' | 'unhealthy';
  database: boolean;
  api: boolean;
  embeddings: boolean;
  version: string;
}

interface MetricCardProps {
  title: string;
  value: string | number;
  subtitle?: string;
  icon: React.ComponentType<{ className?: string }>;
  trend?: 'up' | 'down' | 'stable';
  trendValue?: string;
  accentColor?: 'cyan' | 'teal' | 'violet' | 'amber';
}

function MetricCard({
  title,
  value,
  subtitle,
  icon: Icon,
  trend,
  trendValue,
  accentColor = 'cyan'
}: MetricCardProps) {
  const colorMap = {
    cyan: 'from-cyan-500/20 to-cyan-600/5 border-cyan-500/20',
    teal: 'from-teal-500/20 to-teal-600/5 border-teal-500/20',
    violet: 'from-violet-500/20 to-violet-600/5 border-violet-500/20',
    amber: 'from-amber-500/20 to-amber-600/5 border-amber-500/20',
  };

  const iconColorMap = {
    cyan: 'text-cyan-400',
    teal: 'text-teal-400',
    violet: 'text-violet-400',
    amber: 'text-amber-400',
  };

  return (
    <div className="metric-card metric-card-animated group hover:border-slate-600/50 transition-all duration-500">
      <div className="relative z-10">
        <div className="flex items-start justify-between mb-4">
          <div className={clsx(
            'w-10 h-10 rounded-xl flex items-center justify-center',
            'bg-gradient-to-br border',
            colorMap[accentColor]
          )}>
            <Icon className={clsx('w-5 h-5', iconColorMap[accentColor])} />
          </div>
          {trend && trendValue && (
            <div className={clsx(
              'flex items-center gap-1 text-xs font-medium px-2 py-1 rounded-full',
              trend === 'up' && 'text-emerald-400 bg-emerald-400/10',
              trend === 'down' && 'text-red-400 bg-red-400/10',
              trend === 'stable' && 'text-slate-400 bg-slate-400/10'
            )}>
              {trend === 'up' && <ArrowUpRight className="w-3 h-3" />}
              {trend === 'down' && <ArrowDownRight className="w-3 h-3" />}
              <span>{trendValue}</span>
            </div>
          )}
        </div>
        <p className="text-sm text-slate-400 font-medium mb-1">{title}</p>
        <p className="text-3xl font-bold text-white font-mono-data tracking-tight animate-count">
          {value}
        </p>
        {subtitle && (
          <p className="text-xs text-slate-500 mt-2">{subtitle}</p>
        )}
      </div>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function formatNumber(num: number): string {
  if (num >= 1000000) {
    return `${(num / 1000000).toFixed(1)}M`;
  } else if (num >= 1000) {
    return `${(num / 1000).toFixed(1)}K`;
  }
  return num.toLocaleString();
}

export default function AdminDashboard() {
  const { data: stats, isLoading: statsLoading } = useQuery<SystemStats>({
    queryKey: ['admin', 'stats'],
    queryFn: async () => {
      const response = await fetch('/api/stats');
      if (!response.ok) throw new Error('Failed to fetch stats');
      return response.json();
    },
    refetchInterval: 30000,
  });

  const { data: health, isLoading: healthLoading } = useQuery<HealthStatus>({
    queryKey: ['admin', 'health'],
    queryFn: async () => {
      const response = await fetch('/api/health');
      if (!response.ok) throw new Error('Failed to fetch health');
      return response.json();
    },
    refetchInterval: 10000,
  });

  // Mock cache stats - would come from API in production
  const cacheStats = {
    searchHitRate: 78.5,
    embeddingHitRate: 92.3,
  };

  const recentActivity = [
    { time: '2 min ago', action: 'URL indexed', detail: 'https://example.com/docs', type: 'success' },
    { time: '15 min ago', action: 'Cache cleared', detail: 'Manual trigger', type: 'info' },
    { time: '1 hour ago', action: 'Bulk reindex', detail: '45 documents', type: 'success' },
    { time: '3 hours ago', action: 'Rate limit hit', detail: 'API throttled', type: 'warning' },
  ];

  if (statsLoading || healthLoading) {
    return (
      <div className="flex items-center justify-center h-96">
        <div className="text-center">
          <div className="relative w-16 h-16 mx-auto mb-4">
            <div className="absolute inset-0 rounded-full border-2 border-slate-700" />
            <div className="absolute inset-0 rounded-full border-2 border-cyan-500 border-t-transparent animate-spin" />
            <Zap className="absolute inset-0 m-auto w-6 h-6 text-cyan-400" />
          </div>
          <p className="text-slate-400 font-medium">Initializing Control Center...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      {/* System Health Banner */}
      <div className={clsx(
        'admin-card p-6',
        health?.status === 'healthy' && 'border-emerald-500/30',
        health?.status === 'degraded' && 'border-amber-500/30',
        health?.status === 'unhealthy' && 'border-red-500/30'
      )}>
        <div className="relative z-10 flex items-center justify-between flex-wrap gap-4">
          <div className="flex items-center gap-4">
            <div className={clsx(
              'w-14 h-14 rounded-2xl flex items-center justify-center',
              health?.status === 'healthy' && 'bg-emerald-500/20',
              health?.status === 'degraded' && 'bg-amber-500/20',
              health?.status === 'unhealthy' && 'bg-red-500/20'
            )}>
              {health?.status === 'healthy' ? (
                <CheckCircle className="w-7 h-7 text-emerald-400" />
              ) : (
                <AlertTriangle className="w-7 h-7 text-amber-400" />
              )}
            </div>
            <div>
              <div className="flex items-center gap-3">
                <h2 className="text-xl font-bold text-white">
                  System {health?.status === 'healthy' ? 'Operational' : health?.status?.toUpperCase()}
                </h2>
                <span className={clsx(
                  'status-orb',
                  health?.status === 'healthy' && 'status-orb-healthy',
                  health?.status === 'degraded' && 'status-orb-degraded',
                  health?.status === 'unhealthy' && 'status-orb-unhealthy'
                )} />
              </div>
              <p className="text-sm text-slate-400 mt-1">
                All core services running • Version {health?.version || 'v0.1.0'}
              </p>
            </div>
          </div>

          {/* Service Status Pills */}
          <div className="flex items-center gap-2 flex-wrap">
            {[
              { label: 'Database', ok: health?.database },
              { label: 'API', ok: health?.api },
              { label: 'Embeddings', ok: health?.embeddings },
            ].map((service) => (
              <div
                key={service.label}
                className={clsx(
                  'flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium',
                  service.ok
                    ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
                    : 'bg-red-500/10 text-red-400 border border-red-500/20'
                )}
              >
                <span className={clsx(
                  'w-1.5 h-1.5 rounded-full',
                  service.ok ? 'bg-emerald-400' : 'bg-red-400'
                )} />
                {service.label}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Metrics Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-5">
        <MetricCard
          title="Total Documents"
          value={formatNumber(stats?.total_entries || 0)}
          subtitle={`${stats?.unique_sources || 0} unique sources`}
          icon={FileText}
          trend="up"
          trendValue="+12%"
          accentColor="cyan"
        />
        <MetricCard
          title="Total Chunks"
          value={formatNumber(stats?.total_chunks || 0)}
          subtitle="Searchable segments"
          icon={Layers}
          trend="up"
          trendValue="+8%"
          accentColor="teal"
        />
        <MetricCard
          title="Storage Used"
          value={formatBytes(stats?.store_size_bytes || 0)}
          subtitle={`${stats?.total_pages || 0} pages stored`}
          icon={HardDrive}
          trend="stable"
          trendValue="Stable"
          accentColor="violet"
        />
        <MetricCard
          title="Embeddings"
          value={formatNumber(stats?.total_embeddings || 0)}
          subtitle="Vector embeddings"
          icon={Cpu}
          trend="up"
          trendValue="+15%"
          accentColor="amber"
        />
      </div>

      {/* Cache Performance & Activity */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Cache Performance */}
        <div className="admin-card admin-card-glow p-6">
          <div className="relative z-10">
            <div className="section-header">
              <div className="section-header-icon">
                <Database className="w-5 h-5 text-cyan-400" />
              </div>
              <h3 className="text-lg font-semibold text-white">Cache Performance</h3>
            </div>

            <div className="space-y-6">
              {/* Search Cache */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <span className="text-sm text-slate-400">Search Cache Hit Rate</span>
                  <span className="text-sm font-mono-data font-bold text-white">
                    {cacheStats.searchHitRate}%
                  </span>
                </div>
                <div className="gauge-track">
                  <div
                    className="gauge-fill"
                    style={{ width: `${cacheStats.searchHitRate}%` }}
                  />
                </div>
              </div>

              {/* Embedding Cache */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <span className="text-sm text-slate-400">Embedding Cache Hit Rate</span>
                  <span className="text-sm font-mono-data font-bold text-white">
                    {cacheStats.embeddingHitRate}%
                  </span>
                </div>
                <div className="gauge-track">
                  <div
                    className="gauge-fill"
                    style={{ width: `${cacheStats.embeddingHitRate}%` }}
                  />
                </div>
              </div>

              <button className="w-full btn-admin btn-admin-ghost flex items-center justify-center gap-2 mt-4">
                <RefreshCw className="w-4 h-4" />
                <span>Clear Cache</span>
              </button>
            </div>
          </div>
        </div>

        {/* Recent Activity */}
        <div className="lg:col-span-2 admin-card admin-card-glow p-6">
          <div className="relative z-10">
            <div className="section-header">
              <div className="section-header-icon">
                <Activity className="w-5 h-5 text-cyan-400" />
              </div>
              <h3 className="text-lg font-semibold text-white">Recent Activity</h3>
            </div>

            <div className="space-y-1">
              {recentActivity.map((activity, index) => (
                <div
                  key={index}
                  className={clsx(
                    'log-entry',
                    activity.type === 'warning' && 'log-entry-warning',
                    activity.type === 'error' && 'log-entry-error'
                  )}
                  style={{ animationDelay: `${index * 100}ms` }}
                >
                  <div className="flex items-center gap-3">
                    <Clock className="w-4 h-4 text-slate-500 flex-shrink-0" />
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center justify-between gap-4">
                        <p className="text-sm font-medium text-white truncate">
                          {activity.action}
                        </p>
                        <span className="text-xs text-slate-500 flex-shrink-0">
                          {activity.time}
                        </span>
                      </div>
                      <p className="text-xs text-slate-400 truncate mt-0.5">
                        {activity.detail}
                      </p>
                    </div>
                  </div>
                </div>
              ))}
            </div>

            <button className="w-full text-center text-sm text-cyan-400 hover:text-cyan-300 mt-4 py-2 transition-colors">
              View All Activity →
            </button>
          </div>
        </div>
      </div>

      {/* Quick Actions */}
      <div className="admin-card admin-card-glow p-6">
        <div className="relative z-10">
          <div className="section-header">
            <div className="section-header-icon">
              <Zap className="w-5 h-5 text-cyan-400" />
            </div>
            <h3 className="text-lg font-semibold text-white">Quick Actions</h3>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            {[
              {
                label: 'Clear All Caches',
                desc: 'Invalidate search and embedding caches',
                icon: RefreshCw,
                variant: 'default'
              },
              {
                label: 'Run Diagnostics',
                desc: 'Check database and service health',
                icon: Activity,
                variant: 'default'
              },
              {
                label: 'Export Report',
                desc: 'Download detailed system metrics',
                icon: FileText,
                variant: 'default'
              },
              {
                label: 'Emergency Stop',
                desc: 'Cancel all running jobs',
                icon: AlertTriangle,
                variant: 'danger'
              },
            ].map((action, index) => (
              <button
                key={index}
                className={clsx(
                  'group text-left p-4 rounded-xl border transition-all duration-300',
                  action.variant === 'danger'
                    ? 'bg-red-500/5 border-red-500/20 hover:bg-red-500/10 hover:border-red-500/40'
                    : 'bg-slate-800/40 border-slate-700/50 hover:bg-slate-800/60 hover:border-slate-600/50'
                )}
              >
                <action.icon className={clsx(
                  'w-5 h-5 mb-3 transition-colors',
                  action.variant === 'danger'
                    ? 'text-red-400'
                    : 'text-slate-400 group-hover:text-cyan-400'
                )} />
                <p className={clsx(
                  'font-medium mb-1',
                  action.variant === 'danger' ? 'text-red-400' : 'text-white'
                )}>
                  {action.label}
                </p>
                <p className={clsx(
                  'text-xs',
                  action.variant === 'danger' ? 'text-red-400/70' : 'text-slate-400'
                )}>
                  {action.desc}
                </p>
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
