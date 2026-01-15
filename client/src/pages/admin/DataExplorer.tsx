import { useState, useEffect, useRef, useCallback } from 'react';
import clsx from 'clsx';
import {
  Database,
  Search,
  ChevronRight,
  ChevronDown,
  Table2,
  ChevronLeft,
  Play,
  History,
  BookTemplate,
  Loader2,
  RefreshCw,
  AlertCircle,
  Shield,
  Copy,
  Maximize2,
  Rows3,
  Rows4,
} from 'lucide-react';
import {
  listDatabases,
  refreshDatabases,
  getDatabaseSchema,
  executeQuery,
  getQueryTemplates,
  DatabaseInfo as ApiDatabaseInfo,
  TableSchema,
  QueryResult,
  QueryTemplate,
} from '../../api/explorerClient';

// Extended type for UI state
interface DatabaseInfo extends ApiDatabaseInfo {
  isExpanded?: boolean;
  tableSchemas?: TableSchema[];
}

type DensityMode = 'compact' | 'normal';

const densityConfig = {
  compact: {
    sidebar: { expanded: 'w-52', collapsed: 'w-10' },
    rightSidebar: { expanded: 'w-56', collapsed: 'w-10' },
    card: 'p-2.5',
    gap: 'gap-2',
    text: { header: 'text-xs', body: 'text-[11px]', meta: 'text-[10px]' },
    icon: { sm: 'w-3 h-3', md: 'w-3.5 h-3.5' },
    input: 'px-2 py-1',
    tableCell: 'px-2.5 py-1.5',
  },
  normal: {
    sidebar: { expanded: 'w-60', collapsed: 'w-12' },
    rightSidebar: { expanded: 'w-64', collapsed: 'w-12' },
    card: 'p-3',
    gap: 'gap-3',
    text: { header: 'text-sm', body: 'text-xs', meta: 'text-[11px]' },
    icon: { sm: 'w-3.5 h-3.5', md: 'w-4 h-4' },
    input: 'px-2.5 py-1.5',
    tableCell: 'px-3 py-2',
  },
};

export default function DataExplorer() {
  const [databases, setDatabases] = useState<DatabaseInfo[]>([]);
  const [selectedDatabase, setSelectedDatabase] = useState<DatabaseInfo | null>(null);
  const [selectedTable, setSelectedTable] = useState<string | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [leftSidebarCollapsed, setLeftSidebarCollapsed] = useState(false);
  const [rightSidebarCollapsed, setRightSidebarCollapsed] = useState(false);
  const [queryBuilderCollapsed, setQueryBuilderCollapsed] = useState(true);
  const [queryEditorCollapsed, setQueryEditorCollapsed] = useState(false);
  const [safeMode, setSafeMode] = useState(true);
  const [query, setQuery] = useState('');
  const [queryResult, setQueryResult] = useState<QueryResult | null>(null);
  const [isExecuting, setIsExecuting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hoveredRow, setHoveredRow] = useState<number | null>(null);
  const [density, setDensity] = useState<DensityMode>('normal');
  const [templates, setTemplates] = useState<QueryTemplate[]>([]);
  const [queryHistory, setQueryHistory] = useState<{ query: string; time: Date }[]>([]);

  const searchInputRef = useRef<HTMLInputElement>(null);
  const queryEditorRef = useRef<HTMLTextAreaElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const d = densityConfig[density];

  // Keyboard shortcuts - scoped to explorer container
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const handleKeyboard = (e: KeyboardEvent) => {
      // Only handle if focus is within the explorer
      if (!container.contains(document.activeElement) && document.activeElement !== document.body) {
        return;
      }

      const isMod = e.metaKey || e.ctrlKey;

      // Execute query: Cmd/Ctrl + Enter
      if (isMod && e.key === 'Enter') {
        e.preventDefault();
        runQuery();
      }

      // Focus search: Cmd/Ctrl + K
      if (isMod && e.key === 'k') {
        e.preventDefault();
        searchInputRef.current?.focus();
      }

      // Toggle left sidebar: Cmd/Ctrl + B
      if (isMod && e.key === 'b' && !e.shiftKey) {
        e.preventDefault();
        setLeftSidebarCollapsed(prev => !prev);
      }

      // Toggle right sidebar: Cmd/Ctrl + Shift + B
      if (isMod && e.key === 'b' && e.shiftKey) {
        e.preventDefault();
        setRightSidebarCollapsed(prev => !prev);
      }

      // Focus query editor: Cmd/Ctrl + E
      if (isMod && e.key === 'e') {
        e.preventDefault();
        queryEditorRef.current?.focus();
      }
    };

    window.addEventListener('keydown', handleKeyboard);
    return () => window.removeEventListener('keydown', handleKeyboard);
  }, [query, selectedDatabase]);

  // Discover databases on mount
  useEffect(() => {
    discoverDatabasesHandler();
    loadTemplates();
  }, []);

  const discoverDatabasesHandler = async () => {
    setIsScanning(true);
    setError(null);
    try {
      const dbs = await listDatabases();
      setDatabases(dbs.map(db => ({ ...db, isExpanded: false })));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to discover databases');
    } finally {
      setIsScanning(false);
    }
  };

  const handleRefreshDatabases = async () => {
    setIsScanning(true);
    setError(null);
    try {
      const dbs = await refreshDatabases();
      setDatabases(dbs.map(db => ({ ...db, isExpanded: false })));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to refresh databases');
    } finally {
      setIsScanning(false);
    }
  };

  const loadTemplates = async () => {
    try {
      const t = await getQueryTemplates();
      setTemplates(t);
    } catch (e) {
      console.error('Failed to load templates:', e);
    }
  };

  const loadDatabaseSchema = async (db: DatabaseInfo) => {
    if (db.db_type !== 'sqlite' || db.tableSchemas) return;

    try {
      const schema = await getDatabaseSchema(db.id);
      setDatabases(prev =>
        prev.map(d =>
          d.id === db.id
            ? { ...d, tableSchemas: schema.tables }
            : d
        )
      );
    } catch (e) {
      console.error('Failed to load schema:', e);
    }
  };

  const toggleDatabaseExpand = async (dbId: string) => {
    const db = databases.find(d => d.id === dbId);
    if (db && !db.isExpanded && !db.tableSchemas) {
      await loadDatabaseSchema(db);
    }
    setDatabases(prev =>
      prev.map(d =>
        d.id === dbId ? { ...d, isExpanded: !d.isExpanded } : d
      )
    );
  };

  const selectDatabase = async (db: DatabaseInfo) => {
    setSelectedDatabase(db);
    setSelectedTable(null);
    setQuery('');
    setQueryResult(null);
    setError(null);
    // Load schema if not already loaded
    if (db.db_type === 'sqlite' && !db.tableSchemas) {
      await loadDatabaseSchema(db);
    }
  };

  const selectTable = (db: DatabaseInfo, tableName: string) => {
    setSelectedDatabase(db);
    setSelectedTable(tableName);
    if (db.db_type === 'sqlite') {
      setQuery(`SELECT * FROM "${tableName}" LIMIT 50`);
    } else {
      setQuery(`*`);
    }
    setQueryResult(null);
    setError(null);
  };

  const runQuery = useCallback(async () => {
    if (!query.trim() || !selectedDatabase) return;

    setIsExecuting(true);
    setError(null);
    setQueryResult(null);

    try {
      const response = await executeQuery(selectedDatabase.id, query, {
        safe_mode: safeMode,
        limit: 1000,
      });

      if (response.success && response.result) {
        setQueryResult(response.result);
        // Add to history
        setQueryHistory(prev => [
          { query, time: new Date() },
          ...prev.slice(0, 19), // Keep last 20
        ]);
      } else {
        setError(response.error || 'Query failed');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Query execution failed');
    } finally {
      setIsExecuting(false);
    }
  }, [query, selectedDatabase, safeMode]);

  const applyTemplate = (template: QueryTemplate) => {
    setQuery(template.query);
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
  };

  return (
    <div ref={containerRef} className={clsx('explorer-container flex h-full', d.gap)}>
      {/* Left Sidebar - Database Navigation */}
      <div className={clsx(
        'transition-all duration-200 flex-shrink-0 h-full',
        leftSidebarCollapsed ? d.sidebar.collapsed : d.sidebar.expanded
      )}>
        <div className={clsx('admin-card admin-card-glow h-full relative', d.card)}>
          {/* Collapse Toggle */}
          <button
            onClick={() => setLeftSidebarCollapsed(prev => !prev)}
            className="absolute -right-2 top-3 z-10 w-5 h-5 rounded-full bg-slate-700 border border-slate-600 flex items-center justify-center hover:bg-slate-600 transition-colors"
            title={leftSidebarCollapsed ? 'Expand (⌘B)' : 'Collapse (⌘B)'}
          >
            {leftSidebarCollapsed ? (
              <ChevronRight className="w-2.5 h-2.5 text-slate-400" />
            ) : (
              <ChevronLeft className="w-2.5 h-2.5 text-slate-400" />
            )}
          </button>

          {leftSidebarCollapsed ? (
            /* Collapsed state - icon stack, click to expand */
            <div className="flex flex-col items-center gap-1 pt-1">
              {databases.map(db => (
                <button
                  key={db.id}
                  onClick={() => {
                    setLeftSidebarCollapsed(false);
                    selectDatabase(db);
                  }}
                  className={clsx(
                    'w-6 h-6 flex items-center justify-center transition-colors',
                    selectedDatabase?.id === db.id
                      ? 'bg-cyan-500/20 ring-1 ring-cyan-500/40'
                      : 'hover:bg-slate-700'
                  )}
                  title={`${db.name} (${db.size_display}) - Click to expand`}
                >
                  {db.db_type === 'sqlite' ? (
                    <Database className="w-3.5 h-3.5 text-blue-400" />
                  ) : (
                    <Search className="w-3.5 h-3.5 text-orange-400" />
                  )}
                </button>
              ))}
            </div>
          ) : (
            <>
              {/* Header */}
              <div className="flex items-center justify-between mb-2">
                <h3 className="text-xs font-semibold text-white flex items-center gap-1.5">
                  <Database className="w-3.5 h-3.5 text-cyan-400" />
                  Databases
                </h3>
                <button
                  onClick={handleRefreshDatabases}
                  className="p-1 hover:bg-slate-700 transition-colors"
                  disabled={isScanning}
                  title="Refresh"
                >
                  <RefreshCw className={clsx(
                    'w-3 h-3 text-slate-400',
                    isScanning && 'animate-spin'
                  )} />
                </button>
              </div>

              {/* Search */}
              <div className="mb-2">
                <input
                  ref={searchInputRef}
                  type="text"
                  placeholder="Search... (⌘K)"
                  className="w-full px-2 py-1 bg-slate-800/50 border border-slate-700 text-xs text-white placeholder-slate-500 focus:border-cyan-500 focus:outline-none"
                />
              </div>

              {/* Database Tree */}
              <div className="space-y-0.5 max-h-[calc(100vh-320px)] overflow-y-auto">
                {databases.map(db => (
                  <div key={db.id}>
                    {/* Database Node */}
                    <div
                      className={clsx(
                        'group flex items-center gap-1.5 px-1.5 py-1 cursor-pointer transition-colors',
                        selectedDatabase?.id === db.id
                          ? 'bg-cyan-500/10 text-cyan-400'
                          : 'hover:bg-slate-800/50 text-slate-300'
                      )}
                      onClick={() => selectDatabase(db)}
                    >
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          toggleDatabaseExpand(db.id);
                        }}
                        className="p-0"
                      >
                        {db.isExpanded ? (
                          <ChevronDown className="w-3 h-3" />
                        ) : (
                          <ChevronRight className="w-3 h-3" />
                        )}
                      </button>
                      {db.db_type === 'sqlite' ? (
                        <Database className="w-3.5 h-3.5 text-blue-400 flex-shrink-0" />
                      ) : (
                        <Search className="w-3.5 h-3.5 text-orange-400 flex-shrink-0" />
                      )}
                      <div className="flex-1 min-w-0">
                        <p className="text-[11px] font-medium truncate leading-tight">{db.name}</p>
                        <p className="text-[10px] text-slate-500 leading-tight">{db.size_display}</p>
                      </div>
                      {db.status === 'locked' && (
                        <AlertCircle className="w-3 h-3 text-amber-400 flex-shrink-0" />
                      )}
                    </div>

                    {/* Tables */}
                    {db.isExpanded && db.tableSchemas && (
                      <div className="ml-4 mt-0.5 space-y-0">
                        {db.tableSchemas.map(table => (
                          <div
                            key={table.name}
                            className={clsx(
                              'group flex items-center gap-1.5 px-1.5 py-0.5 cursor-pointer transition-colors text-[11px]',
                              selectedTable === table.name && selectedDatabase?.id === db.id
                                ? 'bg-slate-700/50 text-cyan-400'
                                : 'hover:bg-slate-800/30 text-slate-400'
                            )}
                            onClick={() => selectTable(db, table.name)}
                          >
                            <Table2 className="w-3 h-3 flex-shrink-0" />
                            <span className="flex-1 truncate">{table.name}</span>
                            {table.row_count !== null && (
                              <span className="text-[10px] text-slate-500 tabular-nums">
                                {table.row_count.toLocaleString()}
                              </span>
                            )}
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                ))}
              </div>

              {/* Discovery Status */}
              {isScanning && (
                <div className="mt-2 px-2 py-1 bg-cyan-500/10 border border-cyan-500/20">
                  <p className="text-[10px] text-cyan-400 flex items-center gap-1.5">
                    <Loader2 className="w-3 h-3 animate-spin" />
                    Scanning...
                  </p>
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {/* Center Area - Query and Results */}
      <div className={clsx('flex-1 flex flex-col min-w-0 h-full', d.gap)}>
        {/* Query Builder (Collapsible) */}
        {!queryBuilderCollapsed && (
          <div className={clsx('admin-card admin-card-glow', d.card)}>
            <div className="flex items-center justify-between mb-2">
              <h3 className="text-xs font-semibold text-white">Visual Query Builder</h3>
              <button
                onClick={() => setQueryBuilderCollapsed(true)}
                className="p-0.5 hover:bg-slate-700 transition-colors"
              >
                <ChevronDown className="w-3.5 h-3.5 text-slate-400" />
              </button>
            </div>
            <div className="text-center py-4 text-slate-500 text-xs">
              Visual query builder coming soon...
            </div>
          </div>
        )}

        {/* Query Editor (Collapsible) */}
        <div className={clsx('admin-card admin-card-glow', d.card)}>
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-3">
              <h3 className="text-xs font-semibold text-white">
                {selectedDatabase?.db_type === 'sqlite' ? 'SQL' : 'Search'}
              </h3>
              {queryBuilderCollapsed && (
                <button
                  onClick={() => setQueryBuilderCollapsed(false)}
                  className="text-[10px] text-cyan-400 hover:text-cyan-300"
                >
                  Visual Builder
                </button>
              )}
            </div>
            <div className="flex items-center gap-1.5">
              {/* Density Toggle */}
              <button
                onClick={() => setDensity(prev => prev === 'compact' ? 'normal' : 'compact')}
                className="flex items-center gap-1 px-2 py-0.5 text-[10px] font-medium transition-colors bg-slate-700 text-slate-400 border border-slate-600 hover:bg-slate-600"
                title={`Density: ${density} - Click to toggle`}
              >
                {density === 'compact' ? (
                  <Rows4 className="w-2.5 h-2.5" />
                ) : (
                  <Rows3 className="w-2.5 h-2.5" />
                )}
              </button>
              {/* Safe Mode Toggle */}
              <button
                onClick={() => setSafeMode(!safeMode)}
                className={clsx(
                  'flex items-center gap-1 px-2 py-0.5 text-[10px] font-medium transition-colors',
                  safeMode
                    ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                    : 'bg-slate-700 text-slate-400 border border-slate-600'
                )}
              >
                <Shield className="w-2.5 h-2.5" />
                Safe
              </button>
              <button
                onClick={() => setQueryEditorCollapsed(!queryEditorCollapsed)}
                className="p-0.5 hover:bg-slate-700 transition-colors"
              >
                {queryEditorCollapsed ? (
                  <ChevronRight className="w-3.5 h-3.5 text-slate-400" />
                ) : (
                  <ChevronDown className="w-3.5 h-3.5 text-slate-400" />
                )}
              </button>
            </div>
          </div>

          {!queryEditorCollapsed && (
            <>
              <div className="relative">
                <textarea
                  ref={queryEditorRef}
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder={
                    selectedDatabase?.db_type === 'sqlite'
                      ? 'SELECT * FROM table_name LIMIT 50'
                      : 'Enter search query...'
                  }
                  className="w-full min-h-[60px] max-h-[150px] px-2.5 py-2 bg-slate-900/50 border border-slate-700 text-xs text-white font-mono placeholder-slate-500 focus:border-cyan-500 focus:outline-none resize-y"
                  disabled={!selectedDatabase}
                />
                {selectedDatabase && (
                  <div className="absolute top-1.5 right-1.5">
                    <span className={clsx(
                      'px-1.5 py-0.5 text-[10px] font-medium',
                      selectedDatabase.db_type === 'sqlite'
                        ? 'bg-blue-500/20 text-blue-400'
                        : 'bg-orange-500/20 text-orange-400'
                    )}>
                      {selectedDatabase.db_type.toUpperCase()}
                    </span>
                  </div>
                )}
              </div>

              <div className="flex items-center justify-between mt-2">
                <div className="flex items-center gap-1">
                  <button className="flex items-center gap-1 px-2 py-1 text-[10px] text-slate-400 hover:text-white hover:bg-slate-700 transition-colors">
                    <History className="w-3 h-3" />
                    History
                  </button>
                  <button className="flex items-center gap-1 px-2 py-1 text-[10px] text-slate-400 hover:text-white hover:bg-slate-700 transition-colors">
                    <BookTemplate className="w-3 h-3" />
                    Templates
                  </button>
                </div>
                <button
                  onClick={runQuery}
                  disabled={!query.trim() || isExecuting || !selectedDatabase}
                  className="flex items-center gap-1.5 px-3 py-1 bg-cyan-500/20 hover:bg-cyan-500/30 text-cyan-400 text-xs font-medium border border-cyan-500/30 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {isExecuting ? (
                    <>
                      <Loader2 className="w-3 h-3 animate-spin" />
                      Running...
                    </>
                  ) : (
                    <>
                      <Play className="w-3 h-3" />
                      Run
                      <span className="text-[10px] text-cyan-400/60 ml-1">⌘↵</span>
                    </>
                  )}
                </button>
              </div>
            </>
          )}
        </div>

        {/* Results */}
        <div className={clsx('admin-card admin-card-glow flex-1 min-h-0 flex flex-col', d.card)}>
          <div className="flex items-center justify-between mb-2">
            <h3 className="text-xs font-semibold text-white">Results</h3>
            {queryResult && queryResult.row_count > 0 && (
              <div className="flex items-center gap-2">
                <span className="text-[10px] text-slate-400 tabular-nums">
                  {queryResult.row_count} rows
                  {queryResult.execution_time_ms > 0 && (
                    <span className="text-slate-500 ml-1">
                      ({queryResult.execution_time_ms}ms)
                    </span>
                  )}
                </span>
                <button className="px-2 py-0.5 text-[10px] text-slate-400 hover:text-white hover:bg-slate-700 transition-colors">
                  Export
                </button>
              </div>
            )}
          </div>

          {error && (
            <div className="px-2 py-1.5 bg-red-500/10 border border-red-500/20 mb-2">
              <p className="text-xs text-red-400">{error}</p>
            </div>
          )}

          {queryResult && queryResult.rows.length > 0 ? (
            <div className="overflow-auto flex-1">
              <table className="w-full text-xs">
                <thead className="sticky top-0 z-10">
                  <tr className="bg-slate-900/80 backdrop-blur-sm">
                    {queryResult.columns.map((col, colIdx) => (
                      <th
                        key={colIdx}
                        className={clsx(
                          'text-left text-slate-400 font-medium text-[10px] uppercase tracking-wider border-b border-slate-700',
                          d.tableCell
                        )}
                      >
                        {col}
                      </th>
                    ))}
                    <th className="w-20 px-2 py-1.5 border-b border-slate-700" />
                  </tr>
                </thead>
                <tbody>
                  {queryResult.rows.map((row, i) => (
                    <tr
                      key={i}
                      className="group border-b border-slate-800/50 hover:bg-slate-800/30"
                      onMouseEnter={() => setHoveredRow(i)}
                      onMouseLeave={() => setHoveredRow(null)}
                    >
                      {row.map((value: any, j) => (
                        <td
                          key={j}
                          className={clsx('text-slate-300 truncate max-w-[200px]', d.tableCell)}
                          title={value === null ? 'NULL' : typeof value === 'object' ? JSON.stringify(value) : String(value)}
                        >
                          {value === null ? (
                            <span className="text-slate-500 italic">NULL</span>
                          ) : typeof value === 'object' ? (
                            JSON.stringify(value)
                          ) : (
                            String(value)
                          )}
                        </td>
                      ))}
                      <td className="px-2 py-1">
                        <div className={clsx(
                          'flex items-center gap-0.5 transition-opacity',
                          hoveredRow === i ? 'opacity-100' : 'opacity-0'
                        )}>
                          <button
                            onClick={() => copyToClipboard(JSON.stringify(Object.fromEntries(
                              queryResult.columns.map((col, idx) => [col, row[idx]])
                            )))}
                            className="p-1 hover:bg-slate-700 transition-colors"
                            title="Copy row"
                          >
                            <Copy className="w-3 h-3 text-slate-400" />
                          </button>
                          <button
                            className="p-1 hover:bg-slate-700 transition-colors"
                            title="Expand"
                          >
                            <Maximize2 className="w-3 h-3 text-slate-400" />
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="flex-1 flex items-center justify-center">
              <div className="text-center">
                <Database className="w-8 h-8 text-slate-600 mx-auto mb-2" />
                <p className="text-xs text-slate-500">
                  {selectedDatabase
                    ? 'Run a query to see results'
                    : 'Select a database to start'}
                </p>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Right Sidebar - Details and History */}
      <div className={clsx(
        'transition-all duration-200 flex-shrink-0 h-full',
        rightSidebarCollapsed ? d.rightSidebar.collapsed : d.rightSidebar.expanded
      )}>
        <div className={clsx('admin-card admin-card-glow h-full relative', d.card)}>
          {/* Collapse Toggle */}
          <button
            onClick={() => setRightSidebarCollapsed(prev => !prev)}
            className="absolute -left-2 top-3 z-10 w-5 h-5 rounded-full bg-slate-700 border border-slate-600 flex items-center justify-center hover:bg-slate-600 transition-colors"
            title={rightSidebarCollapsed ? 'Expand (⌘⇧B)' : 'Collapse (⌘⇧B)'}
          >
            {rightSidebarCollapsed ? (
              <ChevronLeft className="w-2.5 h-2.5 text-slate-400" />
            ) : (
              <ChevronRight className="w-2.5 h-2.5 text-slate-400" />
            )}
          </button>

          {rightSidebarCollapsed ? (
            /* Collapsed state - icon stack, click to expand */
            <div className="flex flex-col items-center gap-1 pt-1">
              <button
                onClick={() => setRightSidebarCollapsed(false)}
                className="w-6 h-6 flex items-center justify-center hover:bg-slate-700"
                title="Schema - Click to expand"
              >
                <Table2 className="w-3.5 h-3.5 text-slate-400" />
              </button>
              <button
                onClick={() => setRightSidebarCollapsed(false)}
                className="w-6 h-6 flex items-center justify-center hover:bg-slate-700"
                title="History - Click to expand"
              >
                <History className="w-3.5 h-3.5 text-slate-400" />
              </button>
              <button
                onClick={() => setRightSidebarCollapsed(false)}
                className="w-6 h-6 flex items-center justify-center hover:bg-slate-700"
                title="Templates - Click to expand"
              >
                <BookTemplate className="w-3.5 h-3.5 text-slate-400" />
              </button>
            </div>
          ) : (
            <div className="space-y-3">
              {/* Schema Details */}
              {selectedTable && selectedDatabase && (
                <div>
                  <h3 className="text-[10px] font-semibold text-slate-400 uppercase tracking-wider mb-1.5">
                    Schema
                  </h3>
                  <div className="space-y-1">
                    <div className="px-2 py-1 bg-slate-800/50">
                      <p className="text-[10px] text-slate-500">Table</p>
                      <p className="text-xs text-white font-medium truncate">{selectedTable}</p>
                    </div>
                    <div className="px-2 py-1 bg-slate-800/50">
                      <p className="text-[10px] text-slate-500">Database</p>
                      <p className="text-xs text-white font-medium truncate">{selectedDatabase.name}</p>
                    </div>
                    <div className="px-2 py-1 bg-slate-800/50">
                      <p className="text-[10px] text-slate-500">Type</p>
                      <p className="text-xs text-white font-medium uppercase">
                        {selectedDatabase.db_type}
                      </p>
                    </div>
                  </div>
                </div>
              )}

              {/* Query History */}
              <div>
                <h3 className="text-[10px] font-semibold text-slate-400 uppercase tracking-wider mb-1.5">
                  History
                </h3>
                <div className="space-y-1 max-h-32 overflow-y-auto">
                  {queryHistory.length > 0 ? (
                    queryHistory.map((entry, i) => {
                      const ago = Math.round((Date.now() - entry.time.getTime()) / 60000);
                      return (
                        <div
                          key={i}
                          className="px-2 py-1 bg-slate-800/50 hover:bg-slate-700/50 cursor-pointer"
                          onClick={() => setQuery(entry.query)}
                        >
                          <p className="text-[10px] text-slate-500">
                            {ago < 1 ? 'Just now' : `${ago}m ago`}
                          </p>
                          <p className="text-[11px] text-white font-mono truncate">
                            {entry.query.slice(0, 50)}
                          </p>
                        </div>
                      );
                    })
                  ) : (
                    <p className="text-[10px] text-slate-500 px-2">No history yet</p>
                  )}
                </div>
              </div>

              {/* Templates */}
              <div>
                <h3 className="text-[10px] font-semibold text-slate-400 uppercase tracking-wider mb-1.5">
                  Templates
                </h3>
                <div className="space-y-1 max-h-40 overflow-y-auto">
                  {templates
                    .filter(t => !selectedDatabase || t.database_type === selectedDatabase.db_type)
                    .slice(0, 5)
                    .map((template) => (
                      <button
                        key={template.id}
                        className="w-full px-2 py-1 bg-slate-800/50 hover:bg-slate-700/50 text-left"
                        onClick={() => applyTemplate(template)}
                      >
                        <p className="text-[11px] text-white">{template.name}</p>
                        <p className="text-[10px] text-slate-500 truncate">{template.description}</p>
                      </button>
                    ))}
                  {templates.length === 0 && (
                    <p className="text-[10px] text-slate-500 px-2">Loading templates...</p>
                  )}
                </div>
              </div>

              {/* Keyboard Shortcuts */}
              <div className="pt-2 border-t border-slate-700/50">
                <h3 className="text-[10px] font-semibold text-slate-400 uppercase tracking-wider mb-1.5">
                  Shortcuts
                </h3>
                <div className="space-y-0.5 text-[10px]">
                  <div className="flex justify-between text-slate-500">
                    <span>Run query</span>
                    <span className="text-slate-400">⌘↵</span>
                  </div>
                  <div className="flex justify-between text-slate-500">
                    <span>Search</span>
                    <span className="text-slate-400">⌘K</span>
                  </div>
                  <div className="flex justify-between text-slate-500">
                    <span>Toggle sidebar</span>
                    <span className="text-slate-400">⌘B</span>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
