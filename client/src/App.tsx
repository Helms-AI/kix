import { BrowserRouter, Routes, Route, NavLink, useLocation, Navigate, useParams } from 'react-router-dom';
import { LayoutDashboard, Grid, Network, Menu, X, Zap, Settings, Plug2, BookOpen, Sparkles, FolderKanban } from 'lucide-react';
import { useState } from 'react';
import clsx from 'clsx';

import Dashboard from './pages/Dashboard';
import EntryBrowser from './pages/EntryBrowser';
import EntryDetail from './pages/EntryDetail';
import EntryGraph from './pages/EntryGraph';
import IndexingDashboard from './pages/IndexingDashboard';
import AdminPage from './pages/AdminPage';
import MCPDocs from './pages/MCPDocs';
import { Header } from './components/Header';
import { Footer } from './components/Footer';

// Redirect component for /patterns/:id to /entries/:id
function PatternRedirect() {
  const { id } = useParams<{ id: string }>();
  return <Navigate to={`/entries/${id}`} replace />;
}

// Navigation organized by sections
const navSections = [
  {
    name: 'Projects',
    icon: FolderKanban,
    items: [],
  },
  {
    name: 'Knowledge',
    icon: BookOpen,
    items: [
      { name: 'Dashboard', href: '/', icon: LayoutDashboard },
      { name: 'Indexing', href: '/indexing', icon: Zap },
      { name: 'Entries', href: '/entries', icon: Grid },
      { name: 'Graph', href: '/graph', icon: Network },
    ],
  },
  {
    name: 'AI',
    icon: Sparkles,
    items: [
      { name: 'MCP', href: '/config/mcp', icon: Plug2 },
    ],
  },
];

function Sidebar() {
  const location = useLocation();
  const [mobileOpen, setMobileOpen] = useState(false);

  return (
    <>
      {/* Mobile menu button - positioned within header area */}
      <button
        className="lg:hidden fixed top-[18px] left-4 z-[60] p-2 rounded-lg bg-slate-800/80 backdrop-blur text-slate-400 hover:text-white transition-colors"
        onClick={() => setMobileOpen(!mobileOpen)}
      >
        {mobileOpen ? <X size={24} /> : <Menu size={24} />}
      </button>

      {/* Sidebar - positioned between header and footer */}
      <aside
        className={clsx(
          'fixed top-16 bottom-12 left-0 z-40 w-64 bg-slate-900/95 backdrop-blur-lg border-r border-slate-800 transform transition-transform duration-300 lg:translate-x-0',
          mobileOpen ? 'translate-x-0' : '-translate-x-full'
        )}
      >
        <div className="flex flex-col h-full">
          {/* Sectioned Navigation */}
          <nav className="flex-1 px-3 py-4 space-y-6 overflow-y-auto scrollbar-thin">
            {navSections.map((section) => (
              <div key={section.name}>
                {/* Section Header */}
                <div className="flex items-center gap-2 px-2 mb-2">
                  <section.icon className="w-3.5 h-3.5 text-slate-500" />
                  <span className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">
                    {section.name}
                  </span>
                  <div className="flex-1 h-px bg-gradient-to-r from-slate-700/50 to-transparent ml-2" />
                </div>
                {/* Section Items */}
                <div className="space-y-0.5">
                  {section.items.length > 0 ? (
                    section.items.map((item) => {
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
                    })
                  ) : (
                    <div className="px-4 py-2 text-xs text-slate-600 italic">
                      No items yet
                    </div>
                  )}
                </div>
              </div>
            ))}
          </nav>

          {/* Administration Link */}
          <nav className="mt-auto pt-4 pb-4 px-4 border-t border-slate-800">
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
        </div>
      </aside>

      {/* Mobile overlay - positioned between header and footer */}
      {mobileOpen && (
        <div
          className="fixed top-16 bottom-12 inset-x-0 z-30 bg-black/50 lg:hidden"
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
    <div className="min-h-screen pt-16 pb-12">
      <Header />
      <Sidebar />
      <main className="lg:ml-64 min-h-[calc(100vh-64px-48px)]">
        {isAdminPage ? (
          <Routes>
            <Route path="/admin/*" element={<AdminPage />} />
          </Routes>
        ) : (
          <div className="px-4 py-8 lg:px-8">
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
      <Footer />
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
