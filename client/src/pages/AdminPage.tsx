import { Routes, Route, NavLink, Navigate, useLocation } from 'react-router-dom';
import clsx from 'clsx';
import {
  LayoutDashboard,
  Settings,
  Database,
  Activity,
  Shield,
  Sparkles,
  Plug,
} from 'lucide-react';

// Import admin tab components
import AdminDashboard from './admin/AdminDashboard';
import AdminConfiguration from './admin/AdminConfiguration';
import AdminDataManagement from './admin/AdminDataManagement';
import AdminMonitoring from './admin/AdminMonitoring';
import AdminIntegrations from './admin/AdminIntegrations';

interface AdminTab {
  id: string;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  component: React.ComponentType;
  path: string;
}

const tabs: AdminTab[] = [
  {
    id: 'dashboard',
    label: 'Dashboard',
    icon: LayoutDashboard,
    component: AdminDashboard,
    path: 'dashboard'
  },
  {
    id: 'integrations',
    label: 'Integrations',
    icon: Plug,
    component: AdminIntegrations,
    path: 'integrations'
  },
  {
    id: 'config',
    label: 'Configuration',
    icon: Settings,
    component: AdminConfiguration,
    path: 'configuration'
  },
  {
    id: 'data',
    label: 'Data Management',
    icon: Database,
    component: AdminDataManagement,
    path: 'data'
  },
  {
    id: 'monitoring',
    label: 'Monitoring',
    icon: Activity,
    component: AdminMonitoring,
    path: 'monitoring'
  },
];

export default function AdminPage() {
  const location = useLocation();

  return (
    <div className="admin-container font-ui">
      {/* Header with glassmorphism effect */}
      <div className="relative z-10 flex-shrink-0">
        <div className="bg-slate-900/80 backdrop-blur-xl border-b border-slate-700/50">
          {/* Top Header */}
          <div className="px-8 py-6">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-4">
                <div className="relative">
                  <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-cyan-500 to-teal-500 flex items-center justify-center shadow-lg shadow-cyan-500/25">
                    <Shield className="w-6 h-6 text-white" />
                  </div>
                  <div className="absolute -top-1 -right-1 w-3 h-3 rounded-full bg-emerald-400 border-2 border-slate-900">
                    <span className="absolute inset-0 rounded-full bg-emerald-400 animate-ping opacity-75" />
                  </div>
                </div>
                <div>
                  <div className="flex items-center gap-2">
                    <h1 className="text-2xl font-bold text-white tracking-tight">
                      Administration
                    </h1>
                    <Sparkles className="w-5 h-5 text-cyan-400" />
                  </div>
                  <p className="text-sm text-slate-400 mt-0.5 font-mono-data">
                    KIX Control Center • v0.1.0
                  </p>
                </div>
              </div>

              {/* System Status Badge */}
              <div className="hidden md:flex items-center gap-3 px-4 py-2 bg-emerald-500/10 border border-emerald-500/20 rounded-full">
                <span className="status-orb status-orb-healthy" />
                <span className="text-sm font-medium text-emerald-400">System Operational</span>
              </div>
            </div>
          </div>

          {/* Tab Navigation */}
          <div className="px-8">
            <nav className="flex gap-1">
              {tabs.map((tab) => {
                const isActive = location.pathname.includes(tab.path);
                return (
                  <NavLink
                    key={tab.id}
                    to={`/admin/${tab.path}`}
                    className={clsx(
                      'admin-tab group flex items-center gap-2',
                      isActive && 'admin-tab-active'
                    )}
                  >
                    <tab.icon className={clsx(
                      'w-4 h-4 transition-colors duration-300',
                      isActive ? 'text-cyan-400' : 'text-slate-500 group-hover:text-slate-300'
                    )} />
                    <span>{tab.label}</span>
                  </NavLink>
                );
              })}
            </nav>
          </div>
        </div>
      </div>

      {/* Content Area */}
      <div className="relative z-10 px-8 py-8 flex-1 min-h-0">
        <div className="h-full">
          <Routes>
            <Route path="/" element={<Navigate to="/admin/dashboard" replace />} />
            {tabs.map((tab) => (
              <Route
                key={tab.id}
                path={tab.path}
                element={<tab.component />}
              />
            ))}
          </Routes>
        </div>
      </div>
    </div>
  );
}
