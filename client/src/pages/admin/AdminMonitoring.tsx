import clsx from 'clsx';
import {
  Activity,
  Server,
  Cpu,
  HardDrive,
  AlertTriangle,
  CheckCircle,
  XCircle,
  Clock,
  Zap,
  Database,
  Globe,
  Radio,
  Gauge,
  FileWarning,
  History,
  User
} from 'lucide-react';

interface ServiceStatus {
  name: string;
  status: 'healthy' | 'degraded' | 'unhealthy';
  latency: number;
  lastChecked: string;
  details?: string;
  icon: React.ComponentType<{ className?: string }>;
}

interface SystemResources {
  cpu: number;
  memory: {
    used: number;
    total: number;
    percentage: number;
  };
  disk: {
    used: number;
    total: number;
    percentage: number;
  };
}

interface AuditLogEntry {
  id: string;
  timestamp: string;
  user: string;
  action: string;
  details: string;
  status: 'success' | 'failure';
}

interface ErrorEntry {
  timestamp: string;
  service: string;
  error: string;
  severity: 'info' | 'warning' | 'error';
}

function ServiceCard({ service }: { service: ServiceStatus }) {
  const Icon = service.icon;

  return (
    <div className={clsx(
      'service-card',
      service.status === 'healthy' && 'service-card-healthy',
      service.status === 'degraded' && 'service-card-degraded',
      service.status === 'unhealthy' && 'service-card-unhealthy'
    )}>
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-3">
          <div className={clsx(
            'w-10 h-10 rounded-lg flex items-center justify-center',
            service.status === 'healthy' && 'bg-emerald-500/10',
            service.status === 'degraded' && 'bg-amber-500/10',
            service.status === 'unhealthy' && 'bg-red-500/10'
          )}>
            <Icon className={clsx(
              'w-5 h-5',
              service.status === 'healthy' && 'text-emerald-400',
              service.status === 'degraded' && 'text-amber-400',
              service.status === 'unhealthy' && 'text-red-400'
            )} />
          </div>
          <div>
            <p className="font-medium text-white">{service.name}</p>
            <p className="text-xs text-slate-400 mt-0.5">{service.details}</p>
          </div>
        </div>
        <span className={clsx(
          'status-orb',
          service.status === 'healthy' && 'status-orb-healthy',
          service.status === 'degraded' && 'status-orb-degraded',
          service.status === 'unhealthy' && 'status-orb-unhealthy'
        )} />
      </div>
      <div className="flex items-center justify-between text-sm">
        <span className="text-slate-400">Latency</span>
        <span className={clsx(
          'font-mono-data font-medium',
          service.latency < 50 && 'text-emerald-400',
          service.latency >= 50 && service.latency < 100 && 'text-amber-400',
          service.latency >= 100 && 'text-red-400'
        )}>
          {service.latency}ms
        </span>
      </div>
    </div>
  );
}

function ResourceGauge({
  label,
  value,
  max,
  unit,
  icon: Icon,
  thresholdWarning = 75,
  thresholdCritical = 90,
}: {
  label: string;
  value: number;
  max: number;
  unit: string;
  icon: React.ComponentType<{ className?: string }>;
  thresholdWarning?: number;
  thresholdCritical?: number;
}) {
  const percentage = (value / max) * 100;
  const status = percentage >= thresholdCritical ? 'critical' : percentage >= thresholdWarning ? 'warning' : 'normal';

  return (
    <div className="bg-slate-800/40 rounded-xl p-5 border border-slate-700/50">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <div className={clsx(
            'w-10 h-10 rounded-lg flex items-center justify-center',
            status === 'normal' && 'bg-emerald-500/10',
            status === 'warning' && 'bg-amber-500/10',
            status === 'critical' && 'bg-red-500/10'
          )}>
            <Icon className={clsx(
              'w-5 h-5',
              status === 'normal' && 'text-emerald-400',
              status === 'warning' && 'text-amber-400',
              status === 'critical' && 'text-red-400'
            )} />
          </div>
          <span className="font-medium text-white">{label}</span>
        </div>
        <span className="text-sm text-slate-400">
          <span className="font-mono-data font-bold text-white">{value.toFixed(1)}</span>
          {unit} / {max.toFixed(0)}{unit}
        </span>
      </div>
      <div className="gauge-track">
        <div
          className={clsx(
            'gauge-fill',
            status === 'warning' && 'gauge-fill-warning',
            status === 'critical' && 'gauge-fill-critical'
          )}
          style={{ width: `${percentage}%` }}
        />
      </div>
      <div className="flex items-center justify-between mt-2 text-xs">
        <span className="text-slate-500">0%</span>
        <span className={clsx(
          'font-medium',
          status === 'normal' && 'text-emerald-400',
          status === 'warning' && 'text-amber-400',
          status === 'critical' && 'text-red-400'
        )}>
          {percentage.toFixed(1)}% utilized
        </span>
        <span className="text-slate-500">100%</span>
      </div>
    </div>
  );
}

export default function AdminMonitoring() {
  const services: ServiceStatus[] = [
    {
      name: 'LanceDB',
      status: 'healthy',
      latency: 12,
      lastChecked: new Date().toISOString(),
      details: 'All tables accessible',
      icon: Database,
    },
    {
      name: 'Embedding Service',
      status: 'healthy',
      latency: 45,
      lastChecked: new Date().toISOString(),
      details: 'Model loaded, GPU enabled',
      icon: Cpu,
    },
    {
      name: 'Search API',
      status: 'healthy',
      latency: 23,
      lastChecked: new Date().toISOString(),
      details: 'Cache hit rate: 78%',
      icon: Globe,
    },
    {
      name: 'Crawler Service',
      status: 'degraded',
      latency: 156,
      lastChecked: new Date().toISOString(),
      details: 'High latency detected',
      icon: Activity,
    },
    {
      name: 'Job Queue',
      status: 'healthy',
      latency: 8,
      lastChecked: new Date().toISOString(),
      details: '3 workers active',
      icon: Server,
    },
    {
      name: 'SSE Stream',
      status: 'healthy',
      latency: 5,
      lastChecked: new Date().toISOString(),
      details: '12 active connections',
      icon: Radio,
    },
  ];

  const resources: SystemResources = {
    cpu: 45.2,
    memory: {
      used: 4.2,
      total: 8.0,
      percentage: 52.5,
    },
    disk: {
      used: 123.4,
      total: 500.0,
      percentage: 24.7,
    },
  };

  const recentErrors: ErrorEntry[] = [
    {
      timestamp: '2024-01-12 10:23:15',
      service: 'Crawler',
      error: 'Connection timeout to https://example.com',
      severity: 'warning',
    },
    {
      timestamp: '2024-01-12 09:45:32',
      service: 'Embeddings',
      error: 'GPU memory allocation failed, falling back to CPU',
      severity: 'warning',
    },
    {
      timestamp: '2024-01-12 08:12:45',
      service: 'API',
      error: 'Rate limit exceeded for IP 192.168.1.1',
      severity: 'info',
    },
  ];

  const auditLog: AuditLogEntry[] = [
    {
      id: '1',
      timestamp: '2024-01-12 10:30:00',
      user: 'admin',
      action: 'Clear Cache',
      details: 'Manually invalidated all caches',
      status: 'success',
    },
    {
      id: '2',
      timestamp: '2024-01-12 09:15:00',
      user: 'admin',
      action: 'Update Settings',
      details: 'Changed cache TTL from 300 to 30 seconds',
      status: 'success',
    },
    {
      id: '3',
      timestamp: '2024-01-12 08:00:00',
      user: 'system',
      action: 'Auto Cleanup',
      details: 'Removed 45 expired cache entries',
      status: 'success',
    },
  ];

  const healthyServices = services.filter((s) => s.status === 'healthy').length;
  const overallHealth =
    healthyServices === services.length
      ? 'healthy'
      : healthyServices >= services.length * 0.75
      ? 'degraded'
      : 'unhealthy';

  return (
    <div className="space-y-8">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-cyan-500/20 to-teal-500/20 border border-cyan-500/20 flex items-center justify-center">
            <Gauge className="w-6 h-6 text-cyan-400" />
          </div>
          <div>
            <h2 className="text-xl font-bold text-white">System Monitoring</h2>
            <p className="text-sm text-slate-400 mt-0.5">
              Real-time service health and performance metrics
            </p>
          </div>
        </div>

        {/* Overall Status */}
        <div className={clsx(
          'flex items-center gap-3 px-4 py-2 rounded-full',
          overallHealth === 'healthy' && 'bg-emerald-500/10 border border-emerald-500/20',
          overallHealth === 'degraded' && 'bg-amber-500/10 border border-amber-500/20',
          overallHealth === 'unhealthy' && 'bg-red-500/10 border border-red-500/20'
        )}>
          {overallHealth === 'healthy' ? (
            <CheckCircle className="w-5 h-5 text-emerald-400" />
          ) : (
            <AlertTriangle className="w-5 h-5 text-amber-400" />
          )}
          <span className={clsx(
            'font-medium',
            overallHealth === 'healthy' && 'text-emerald-400',
            overallHealth === 'degraded' && 'text-amber-400',
            overallHealth === 'unhealthy' && 'text-red-400'
          )}>
            {healthyServices}/{services.length} Services Online
          </span>
        </div>
      </div>

      {/* Service Status Grid */}
      <div className="admin-card admin-card-glow p-6">
        <div className="relative z-10">
          <div className="section-header">
            <div className="section-header-icon">
              <Zap className="w-5 h-5 text-cyan-400" />
            </div>
            <h3 className="text-lg font-semibold text-white">Service Status</h3>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {services.map((service, index) => (
              <ServiceCard key={index} service={service} />
            ))}
          </div>
        </div>
      </div>

      {/* Resource Usage */}
      <div className="admin-card admin-card-glow p-6">
        <div className="relative z-10">
          <div className="section-header">
            <div className="section-header-icon">
              <Server className="w-5 h-5 text-cyan-400" />
            </div>
            <h3 className="text-lg font-semibold text-white">Resource Usage</h3>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <ResourceGauge
              label="CPU Usage"
              value={resources.cpu}
              max={100}
              unit="%"
              icon={Cpu}
              thresholdWarning={70}
              thresholdCritical={90}
            />
            <ResourceGauge
              label="Memory"
              value={resources.memory.used}
              max={resources.memory.total}
              unit="GB"
              icon={Server}
              thresholdWarning={75}
              thresholdCritical={90}
            />
            <ResourceGauge
              label="Disk Storage"
              value={resources.disk.used}
              max={resources.disk.total}
              unit="GB"
              icon={HardDrive}
              thresholdWarning={80}
              thresholdCritical={95}
            />
          </div>
        </div>
      </div>

      {/* Errors & Audit Log */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Recent Errors */}
        <div className="admin-card admin-card-glow p-6">
          <div className="relative z-10">
            <div className="section-header">
              <div className="section-header-icon">
                <FileWarning className="w-5 h-5 text-cyan-400" />
              </div>
              <h3 className="text-lg font-semibold text-white">Recent Errors</h3>
            </div>
            <div className="space-y-1">
              {recentErrors.map((error, index) => (
                <div
                  key={index}
                  className={clsx(
                    'log-entry',
                    error.severity === 'warning' && 'log-entry-warning',
                    error.severity === 'error' && 'log-entry-error'
                  )}
                >
                  <div className="flex items-start gap-3">
                    <AlertTriangle className={clsx(
                      'w-4 h-4 mt-0.5 flex-shrink-0',
                      error.severity === 'warning' && 'text-amber-400',
                      error.severity === 'error' && 'text-red-400',
                      error.severity === 'info' && 'text-cyan-400'
                    )} />
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-1">
                        <span className="text-sm font-medium text-white">{error.service}</span>
                        <span className="text-xs text-slate-500">•</span>
                        <span className="text-xs text-slate-500">{error.timestamp}</span>
                      </div>
                      <p className="text-sm text-slate-400 break-words">{error.error}</p>
                    </div>
                  </div>
                </div>
              ))}
            </div>
            <button className="w-full text-center text-sm text-cyan-400 hover:text-cyan-300 mt-4 py-2 transition-colors">
              View All Errors →
            </button>
          </div>
        </div>

        {/* Audit Log */}
        <div className="admin-card admin-card-glow p-6">
          <div className="relative z-10">
            <div className="section-header">
              <div className="section-header-icon">
                <History className="w-5 h-5 text-cyan-400" />
              </div>
              <h3 className="text-lg font-semibold text-white">Audit Log</h3>
            </div>
            <div className="space-y-1">
              {auditLog.map((entry) => (
                <div key={entry.id} className="log-entry">
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex items-start gap-3 min-w-0 flex-1">
                      <div className="w-8 h-8 rounded-lg bg-slate-800 flex items-center justify-center flex-shrink-0">
                        <User className="w-4 h-4 text-slate-400" />
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2 mb-1">
                          <span className="text-sm font-medium text-white">{entry.action}</span>
                        </div>
                        <p className="text-sm text-slate-400 truncate">{entry.details}</p>
                        <div className="flex items-center gap-2 mt-1.5 text-xs text-slate-500">
                          <span>{entry.user}</span>
                          <span>•</span>
                          <span>{entry.timestamp}</span>
                        </div>
                      </div>
                    </div>
                    {entry.status === 'success' ? (
                      <CheckCircle className="w-4 h-4 text-emerald-400 flex-shrink-0" />
                    ) : (
                      <XCircle className="w-4 h-4 text-red-400 flex-shrink-0" />
                    )}
                  </div>
                </div>
              ))}
            </div>
            <button className="w-full text-center text-sm text-cyan-400 hover:text-cyan-300 mt-4 py-2 transition-colors">
              View Full Audit Log →
            </button>
          </div>
        </div>
      </div>

      {/* Performance Metrics Placeholder */}
      <div className="admin-card admin-card-glow p-6">
        <div className="relative z-10">
          <div className="section-header">
            <div className="section-header-icon">
              <Activity className="w-5 h-5 text-cyan-400" />
            </div>
            <h3 className="text-lg font-semibold text-white">Performance Metrics</h3>
          </div>
          <div className="h-64 flex items-center justify-center rounded-xl border border-dashed border-slate-700">
            <div className="text-center">
              <div className="w-16 h-16 rounded-2xl bg-slate-800/50 flex items-center justify-center mx-auto mb-4">
                <Activity className="w-8 h-8 text-slate-600" />
              </div>
              <p className="text-slate-400 font-medium">Performance Charts</p>
              <p className="text-sm text-slate-500 mt-1">Coming soon with real-time visualization</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
