import { BrowserRouter, Routes, Route, NavLink, useLocation, Navigate, useParams } from 'react-router-dom';
import { LayoutDashboard, Grid, Network, Menu, X, Zap, Settings, Plug2, BookOpen, Sparkles, FolderKanban, Plus } from 'lucide-react';
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import clsx from 'clsx';

import Dashboard from './pages/Dashboard';
import EntryBrowser from './pages/EntryBrowser';
import EntryDetail from './pages/EntryDetail';
import EntryGraph from './pages/EntryGraph';
import IndexingDashboard from './pages/IndexingDashboard';
import AdminPage from './pages/AdminPage';
import MCPDocs from './pages/MCPDocs';
import { ProjectList, ProjectDetail } from './pages/projects';
import { Header } from './components/Header';
import { Footer } from './components/Footer';
import { projectApi } from './api/projectClient';

// Redirect component for /patterns/:id to /entries/:id
function PatternRedirect() {
  const { id } = useParams<{ id: string }>();
  return <Navigate to={`/entries/${id}`} replace />;
}

// Static navigation sections (Projects section is dynamic)
const knowledgeSection = {
  name: 'Knowledge',
  icon: BookOpen,
  items: [
    { name: 'Dashboard', href: '/', icon: LayoutDashboard },
    { name: 'Indexing', href: '/indexing', icon: Zap },
    { name: 'Entries', href: '/entries', icon: Grid },
    { name: 'Graph', href: '/graph', icon: Network },
  ],
};

const aiSection = {
  name: 'AI',
  icon: Sparkles,
  items: [
    { name: 'MCP', href: '/config/mcp', icon: Plug2 },
  ],
};

function Sidebar() {
  const location = useLocation();
  const [mobileOpen, setMobileOpen] = useState(false);

  // Fetch projects for dynamic navigation
  const { data: projectsData } = useQuery({
    queryKey: ['projects'],
    queryFn: () => projectApi.listProjects({ limit: 10 }),
    staleTime: 30000, // 30 seconds
  });

  const projects = projectsData?.projects || [];

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
            {/* Projects Section (Dynamic) */}
            <div>
              <div className="flex items-center gap-2 px-2 mb-2">
                <FolderKanban className="w-3.5 h-3.5 text-slate-500" />
                <span className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">
                  Projects
                </span>
                <div className="flex-1 h-px bg-gradient-to-r from-slate-700/50 to-transparent ml-2" />
              </div>
              <div className="space-y-0.5">
                {projects.length > 0 ? (
                  <>
                    {projects.slice(0, 5).map((project) => {
                      const isActive = location.pathname === `/projects/${project.id}`;
                      return (
                        <NavLink
                          key={project.id}
                          to={`/projects/${project.id}`}
                          onClick={() => setMobileOpen(false)}
                          className={clsx(
                            'nav-link',
                            isActive && 'nav-link-active'
                          )}
                        >
                          <div
                            className="w-2 h-2 rounded-full flex-shrink-0"
                            style={{ backgroundColor: project.color }}
                          />
                          <span className="font-medium truncate">{project.name}</span>
                        </NavLink>
                      );
                    })}
                    {projects.length > 5 && (
                      <NavLink
                        to="/projects"
                        onClick={() => setMobileOpen(false)}
                        className="nav-link text-slate-500 hover:text-slate-300"
                      >
                        <span className="text-xs">+{projects.length - 5} more</span>
                      </NavLink>
                    )}
                  </>
                ) : (
                  <NavLink
                    to="/projects"
                    onClick={() => setMobileOpen(false)}
                    className="nav-link text-slate-500 hover:text-cyan-400"
                  >
                    <Plus className="w-4 h-4" />
                    <span className="text-sm">Create project</span>
                  </NavLink>
                )}
                <NavLink
                  to="/projects"
                  onClick={() => setMobileOpen(false)}
                  className={clsx(
                    'nav-link text-xs',
                    location.pathname === '/projects' ? 'nav-link-active' : 'text-slate-500'
                  )}
                >
                  <Grid className="w-3.5 h-3.5" />
                  <span>All projects</span>
                </NavLink>
              </div>
            </div>

            {/* Knowledge Section */}
            <div>
              <div className="flex items-center gap-2 px-2 mb-2">
                <knowledgeSection.icon className="w-3.5 h-3.5 text-slate-500" />
                <span className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">
                  {knowledgeSection.name}
                </span>
                <div className="flex-1 h-px bg-gradient-to-r from-slate-700/50 to-transparent ml-2" />
              </div>
              <div className="space-y-0.5">
                {knowledgeSection.items.map((item) => {
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
              </div>
            </div>

            {/* AI Section */}
            <div>
              <div className="flex items-center gap-2 px-2 mb-2">
                <aiSection.icon className="w-3.5 h-3.5 text-slate-500" />
                <span className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">
                  {aiSection.name}
                </span>
                <div className="flex-1 h-px bg-gradient-to-r from-slate-700/50 to-transparent ml-2" />
              </div>
              <div className="space-y-0.5">
                {aiSection.items.map((item) => {
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
              </div>
            </div>
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
              {/* Project Management Routes */}
              <Route path="/projects" element={<ProjectList />} />
              <Route path="/projects/:id" element={<ProjectDetail />} />
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
