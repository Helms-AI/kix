import { BrowserRouter, Routes, Route, NavLink, useLocation, Navigate, useParams } from 'react-router-dom';
import { LayoutDashboard, Grid, BookOpen, Network, Menu, X, Zap, Settings, Plug2 } from 'lucide-react';
import { useState } from 'react';
import clsx from 'clsx';

import Dashboard from './pages/Dashboard';
import EntryBrowser from './pages/EntryBrowser';
import EntryDetail from './pages/EntryDetail';
import EntryGraph from './pages/EntryGraph';
import IndexingDashboard from './pages/IndexingDashboard';
import AdminPage from './pages/AdminPage';
import MCPDocs from './pages/MCPDocs';

// Redirect component for /patterns/:id to /entries/:id
function PatternRedirect() {
  const { id } = useParams<{ id: string }>();
  return <Navigate to={`/entries/${id}`} replace />;
}

const navigation = [
  { name: 'Dashboard', href: '/', icon: LayoutDashboard },
  { name: 'Indexing', href: '/indexing', icon: Zap },
  { name: 'Entries', href: '/entries', icon: Grid },
  { name: 'Graph', href: '/graph', icon: Network },
  { name: 'MCP', href: '/config/mcp', icon: Plug2 },
];

function Sidebar() {
  const location = useLocation();
  const [mobileOpen, setMobileOpen] = useState(false);

  return (
    <>
      {/* Mobile menu button */}
      <button
        className="lg:hidden fixed top-4 left-4 z-50 p-2 rounded-lg bg-slate-800 text-slate-400"
        onClick={() => setMobileOpen(!mobileOpen)}
      >
        {mobileOpen ? <X size={24} /> : <Menu size={24} />}
      </button>

      {/* Sidebar */}
      <aside
        className={clsx(
          'fixed inset-y-0 left-0 z-40 w-64 bg-slate-900/95 backdrop-blur-lg border-r border-slate-800 transform transition-transform duration-300 lg:translate-x-0',
          mobileOpen ? 'translate-x-0' : '-translate-x-full'
        )}
      >
        <div className="flex flex-col h-full">
          {/* Logo */}
          <div className="flex items-center gap-3 px-6 py-6 border-b border-slate-800">
            <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-cyan-500 to-teal-500 flex items-center justify-center">
              <BookOpen className="w-5 h-5 text-white" />
            </div>
            <div>
              <h1 className="text-lg font-bold text-white">Kix</h1>
              <p className="text-xs text-slate-500 font-mono">Knowledge Base</p>
            </div>
          </div>

          {/* Navigation */}
          <nav className="flex-1 px-4 py-6 space-y-1">
            {navigation.map((item) => {
              const isActive = location.pathname === item.href ||
                (item.href !== '/' && location.pathname.startsWith(item.href));
              return (
                <NavLink
                  key={item.name}
                  to={item.href}
                  onClick={() => setMobileOpen(false)}
                  className={clsx(
                    'nav-link',
                    isActive && 'nav-link-active'
                  )}
                >
                  <item.icon className="w-5 h-5" />
                  <span className="font-medium">{item.name}</span>
                </NavLink>
              );
            })}
          </nav>

          {/* Administration Link */}
          <nav className="mt-auto pt-4 pb-2 px-4 border-t border-slate-800">
            <NavLink
              to="/admin"
              onClick={() => setMobileOpen(false)}
              className={(isActive) => clsx(
                'nav-link',
                isActive && 'nav-link-active'
              )}
            >
              <Settings className="w-5 h-5" />
              <span className="font-medium">Administration</span>
            </NavLink>
          </nav>

          {/* Footer */}
          <div className="px-6 py-4 border-t border-slate-800">
            <p className="text-xs text-slate-500 font-mono">
              Knowledge Indexing System
            </p>
            <p className="text-xs text-slate-600 mt-1">
              v0.1.0
            </p>
          </div>
        </div>
      </aside>

      {/* Mobile overlay */}
      {mobileOpen && (
        <div
          className="fixed inset-0 z-30 bg-black/50 lg:hidden"
          onClick={() => setMobileOpen(false)}
        />
      )}
    </>
  );
}

function AppContent() {
  const location = useLocation();
  const isAdminPage = location.pathname.startsWith('/admin');

  return (
    <div className="min-h-screen">
      <Sidebar />
      <main className="lg:ml-64 min-h-screen">
        {isAdminPage ? (
          <Routes>
            <Route path="/admin/*" element={<AdminPage />} />
          </Routes>
        ) : (
          <div className="container mx-auto px-4 py-8 lg:px-8">
            <Routes>
              <Route path="/" element={<Dashboard />} />
              <Route path="/indexing" element={<IndexingDashboard />} />
              <Route path="/entries" element={<EntryBrowser />} />
              <Route path="/entries/:id" element={<EntryDetail />} />
              {/* Backward compatibility redirects for old URLs */}
              <Route path="/patterns" element={<Navigate to="/entries" replace />} />
              <Route path="/patterns/:id" element={<PatternRedirect />} />
              <Route path="/search" element={<Navigate to="/entries" replace />} />
              <Route path="/graph" element={<EntryGraph />} />
              <Route path="/mcp" element={<Navigate to="/config/mcp" replace />} />
              <Route path="/config/mcp" element={<MCPDocs />} />
            </Routes>
          </div>
        )}
      </main>
    </div>
  );
}

function App() {
  return (
    <BrowserRouter>
      <AppContent />
    </BrowserRouter>
  );
}

export default App;
