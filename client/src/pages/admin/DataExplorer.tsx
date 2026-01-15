import { useState, useRef, useCallback, useEffect } from 'react';
import clsx from 'clsx';
import {
  DatabaseNavigator,
  QueryWorkspace,
  ContextPanel,
  useDatabaseDiscovery,
  useQueryHistory,
  useExplorerKeyboard,
  densityConfig,
  type DatabaseInfo,
  type DensityMode,
  type QueryTemplate,
} from '../../components/explorer';
import { getQueryTemplates } from '../../api/explorerClient';

/**
 * DataExplorer - Main orchestrator component
 *
 * Composes the explorer UI from three main sections:
 * - DatabaseNavigator (left sidebar) - database tree navigation
 * - QueryWorkspace (center) - query editor and results
 * - ContextPanel (right sidebar) - schema details, history, templates
 */
export default function DataExplorer() {
  // ============================================================================
  // STATE
  // ============================================================================

  // Database discovery (via custom hook)
  const {
    databases,
    isScanning,
    refreshDatabaseList,
    loadDatabaseSchema,
    toggleDatabaseExpand,
  } = useDatabaseDiscovery();

  // Selection state
  const [selectedDatabase, setSelectedDatabase] = useState<DatabaseInfo | null>(null);
  const [selectedTable, setSelectedTable] = useState<string | null>(null);

  // Query state
  const [query, setQuery] = useState('');
  const [queryResult, setQueryResult] = useState<any>(null);
  const [isExecuting, setIsExecuting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Query history (via custom hook)
  const { history: queryHistory, addToHistory } = useQueryHistory();

  // UI state
  const [leftSidebarCollapsed, setLeftSidebarCollapsed] = useState(false);
  const [rightSidebarCollapsed, setRightSidebarCollapsed] = useState(false);
  const [safeMode, setSafeMode] = useState(true);
  const [density, setDensity] = useState<DensityMode>('normal');
  const [templates, setTemplates] = useState<QueryTemplate[]>([]);

  // Refs
  const containerRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const queryEditorRef = useRef<HTMLTextAreaElement>(null);

  // Density config
  const d = densityConfig[density];

  // ============================================================================
  // EFFECTS
  // ============================================================================

  // Load templates on mount
  useEffect(() => {
    const loadTemplates = async () => {
      try {
        const t = await getQueryTemplates();
        setTemplates(t);
      } catch (e) {
        console.error('Failed to load templates:', e);
      }
    };
    loadTemplates();
  }, []);

  // ============================================================================
  // HANDLERS
  // ============================================================================

  // Execute query
  const runQuery = useCallback(async () => {
    if (!query.trim() || !selectedDatabase) return;

    setIsExecuting(true);
    setError(null);
    setQueryResult(null);

    try {
      const { executeQuery } = await import('../../api/explorerClient');
      const response = await executeQuery(selectedDatabase.id, query, {
        safe_mode: safeMode,
        limit: 1000,
      });

      if (response.success && response.result) {
        setQueryResult(response.result);
        addToHistory({ query, time: new Date(), databaseId: selectedDatabase.id });
      } else {
        setError(response.error || 'Query failed');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Query execution failed');
    } finally {
      setIsExecuting(false);
    }
  }, [query, selectedDatabase, safeMode, addToHistory]);

  // Select database
  const handleSelectDatabase = useCallback(
    async (db: DatabaseInfo) => {
      setSelectedDatabase(db);
      setSelectedTable(null);
      setQuery('');
      setQueryResult(null);
      setError(null);

      if (db.db_type === 'sqlite' && !db.tableSchemas) {
        await loadDatabaseSchema(db);
      }
    },
    [loadDatabaseSchema]
  );

  // Select table
  const handleSelectTable = useCallback((db: DatabaseInfo, tableName: string) => {
    setSelectedDatabase(db);
    setSelectedTable(tableName);
    setQuery(
      db.db_type === 'sqlite'
        ? `SELECT * FROM "${tableName}" LIMIT 50`
        : '*'
    );
    setQueryResult(null);
    setError(null);
  }, []);

  // Apply template
  const handleApplyTemplate = useCallback((template: QueryTemplate) => {
    setQuery(template.query);
  }, []);

  // Select query from history
  const handleSelectQuery = useCallback((q: string) => {
    setQuery(q);
  }, []);

  // ============================================================================
  // KEYBOARD SHORTCUTS
  // ============================================================================

  useExplorerKeyboard(containerRef, {
    onExecuteQuery: runQuery,
    onFocusSearch: () => searchInputRef.current?.focus(),
    onFocusEditor: () => queryEditorRef.current?.focus(),
    onToggleLeftSidebar: () => setLeftSidebarCollapsed(prev => !prev),
    onToggleRightSidebar: () => setRightSidebarCollapsed(prev => !prev),
  });

  // ============================================================================
  // RENDER
  // ============================================================================

  return (
    <div ref={containerRef} className={clsx('explorer-container flex h-full', d.gap)}>
      {/* Left Sidebar - Database Navigation */}
      <DatabaseNavigator
        databases={databases}
        selectedDatabase={selectedDatabase}
        selectedTable={selectedTable}
        isCollapsed={leftSidebarCollapsed}
        isScanning={isScanning}
        density={d}
        searchInputRef={searchInputRef}
        onSelectDatabase={handleSelectDatabase}
        onSelectTable={handleSelectTable}
        onToggleExpand={toggleDatabaseExpand}
        onRefresh={refreshDatabaseList}
        onToggleCollapse={() => setLeftSidebarCollapsed(prev => !prev)}
      />

      {/* Center Area - Query and Results */}
      <QueryWorkspace
        selectedDatabase={selectedDatabase}
        query={query}
        queryResult={queryResult}
        isExecuting={isExecuting}
        error={error}
        safeMode={safeMode}
        density={density}
        densityConfig={d}
        queryEditorRef={queryEditorRef}
        onQueryChange={setQuery}
        onExecuteQuery={runQuery}
        onToggleSafeMode={() => setSafeMode(prev => !prev)}
        onToggleDensity={() => setDensity(prev => (prev === 'compact' ? 'normal' : 'compact'))}
      />

      {/* Right Sidebar - Context Panel */}
      <ContextPanel
        selectedDatabase={selectedDatabase}
        selectedTable={selectedTable}
        queryHistory={queryHistory}
        templates={templates}
        isCollapsed={rightSidebarCollapsed}
        density={d}
        onSelectQuery={handleSelectQuery}
        onApplyTemplate={handleApplyTemplate}
        onToggleCollapse={() => setRightSidebarCollapsed(prev => !prev)}
      />
    </div>
  );
}
