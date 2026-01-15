import { RefObject, useState } from 'react';
import clsx from 'clsx';
import {
  ChevronRight,
  ChevronDown,
  Play,
  History,
  BookTemplate,
  Loader2,
  Shield,
  Copy,
  Maximize2,
  Rows3,
  Rows4,
  Database,
} from 'lucide-react';
import type { DatabaseInfo, QueryResult, DensityConfig, DensityMode } from './types';

// ============================================================================
// ATOMIC COMPONENTS
// ============================================================================

// Icon Button - reusable icon-only button
interface IconButtonProps {
  icon: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
  title?: string;
  className?: string;
}

export function IconButton({ icon, onClick, disabled, title, className }: IconButtonProps) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={clsx(
        'p-1 hover:bg-slate-700 transition-colors disabled:opacity-50',
        className
      )}
    >
      {icon}
    </button>
  );
}

// Toggle Button - for toggleable states like Safe Mode
interface ToggleButtonProps {
  icon: React.ReactNode;
  label: string;
  isActive: boolean;
  onClick: () => void;
  activeClassName?: string;
  inactiveClassName?: string;
}

export function ToggleButton({
  icon,
  label,
  isActive,
  onClick,
  activeClassName = 'bg-amber-500/20 text-amber-400 border-amber-500/30',
  inactiveClassName = 'bg-slate-700 text-slate-400 border-slate-600',
}: ToggleButtonProps) {
  return (
    <button
      onClick={onClick}
      className={clsx(
        'flex items-center gap-1 px-2 py-0.5 text-[10px] font-medium transition-colors border',
        isActive ? activeClassName : inactiveClassName
      )}
    >
      {icon}
      {label}
    </button>
  );
}

// Database Type Badge - shows database type indicator
interface DatabaseTypeBadgeProps {
  dbType: 'sqlite' | 'tantivy';
}

export function DatabaseTypeBadge({ dbType }: DatabaseTypeBadgeProps) {
  return (
    <span
      className={clsx(
        'px-1.5 py-0.5 text-[10px] font-medium',
        dbType === 'sqlite'
          ? 'bg-blue-500/20 text-blue-400'
          : 'bg-orange-500/20 text-orange-400'
      )}
    >
      {dbType.toUpperCase()}
    </span>
  );
}

// Action Button - primary action button with loading state
interface ActionButtonProps {
  onClick: () => void;
  disabled?: boolean;
  isLoading?: boolean;
  loadingText?: string;
  children: React.ReactNode;
}

export function ActionButton({
  onClick,
  disabled,
  isLoading,
  loadingText = 'Running...',
  children,
}: ActionButtonProps) {
  return (
    <button
      onClick={onClick}
      disabled={disabled || isLoading}
      className="flex items-center gap-1.5 px-3 py-1 bg-cyan-500/20 hover:bg-cyan-500/30 text-cyan-400 text-xs font-medium border border-cyan-500/30 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    >
      {isLoading ? (
        <>
          <Loader2 className="w-3 h-3 animate-spin" />
          {loadingText}
        </>
      ) : (
        children
      )}
    </button>
  );
}

// Collapsible Section Header
interface CollapsibleHeaderProps {
  title: string;
  isCollapsed: boolean;
  onToggle: () => void;
  actions?: React.ReactNode;
}

export function CollapsibleHeader({
  title,
  isCollapsed,
  onToggle,
  actions,
}: CollapsibleHeaderProps) {
  return (
    <div className="flex items-center justify-between mb-2">
      <h3 className="text-xs font-semibold text-white">{title}</h3>
      <div className="flex items-center gap-1.5">
        {actions}
        <IconButton
          icon={
            isCollapsed ? (
              <ChevronRight className="w-3.5 h-3.5 text-slate-400" />
            ) : (
              <ChevronDown className="w-3.5 h-3.5 text-slate-400" />
            )
          }
          onClick={onToggle}
        />
      </div>
    </div>
  );
}

// Error Alert
interface ErrorAlertProps {
  message: string;
}

export function ErrorAlert({ message }: ErrorAlertProps) {
  return (
    <div className="px-2 py-1.5 bg-red-500/10 border border-red-500/20 mb-2">
      <p className="text-xs text-red-400">{message}</p>
    </div>
  );
}

// Empty State
interface EmptyStateProps {
  icon: React.ReactNode;
  title: string;
  description?: string;
}

export function EmptyState({ icon, title, description }: EmptyStateProps) {
  return (
    <div className="flex-1 flex items-center justify-center">
      <div className="text-center">
        <div className="mx-auto mb-2">{icon}</div>
        <p className="text-xs text-slate-500">{title}</p>
        {description && (
          <p className="text-[10px] text-slate-600 mt-1">{description}</p>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// MOLECULE COMPONENTS
// ============================================================================

// Query Toolbar - contains all toolbar actions
interface QueryToolbarProps {
  selectedDatabase: DatabaseInfo | null;
  density: DensityMode;
  safeMode: boolean;
  queryBuilderCollapsed: boolean;
  queryEditorCollapsed: boolean;
  onToggleDensity: () => void;
  onToggleSafeMode: () => void;
  onToggleQueryBuilder: () => void;
  onToggleQueryEditor: () => void;
}

export function QueryToolbar({
  selectedDatabase,
  density,
  safeMode,
  queryBuilderCollapsed,
  queryEditorCollapsed,
  onToggleDensity,
  onToggleSafeMode,
  onToggleQueryBuilder,
  onToggleQueryEditor,
}: QueryToolbarProps) {
  return (
    <div className="flex items-center justify-between mb-2">
      <div className="flex items-center gap-3">
        <h3 className="text-xs font-semibold text-white">
          {selectedDatabase?.db_type === 'sqlite' ? 'SQL' : 'Search'}
        </h3>
        {queryBuilderCollapsed && (
          <button
            onClick={onToggleQueryBuilder}
            className="text-[10px] text-cyan-400 hover:text-cyan-300"
          >
            Visual Builder
          </button>
        )}
      </div>
      <div className="flex items-center gap-1.5">
        <ToggleButton
          icon={density === 'compact' ? <Rows4 className="w-2.5 h-2.5" /> : <Rows3 className="w-2.5 h-2.5" />}
          label=""
          isActive={false}
          onClick={onToggleDensity}
          activeClassName="bg-slate-700 text-slate-400 border-slate-600"
          inactiveClassName="bg-slate-700 text-slate-400 border-slate-600 hover:bg-slate-600"
        />
        <ToggleButton
          icon={<Shield className="w-2.5 h-2.5" />}
          label="Safe"
          isActive={safeMode}
          onClick={onToggleSafeMode}
        />
        <IconButton
          icon={
            queryEditorCollapsed ? (
              <ChevronRight className="w-3.5 h-3.5 text-slate-400" />
            ) : (
              <ChevronDown className="w-3.5 h-3.5 text-slate-400" />
            )
          }
          onClick={onToggleQueryEditor}
        />
      </div>
    </div>
  );
}

// Query Input Area
interface QueryInputProps {
  query: string;
  selectedDatabase: DatabaseInfo | null;
  editorRef: RefObject<HTMLTextAreaElement>;
  onChange: (value: string) => void;
}

export function QueryInput({
  query,
  selectedDatabase,
  editorRef,
  onChange,
}: QueryInputProps) {
  return (
    <div className="relative">
      <textarea
        ref={editorRef}
        value={query}
        onChange={(e) => onChange(e.target.value)}
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
          <DatabaseTypeBadge dbType={selectedDatabase.db_type} />
        </div>
      )}
    </div>
  );
}

// Query Actions Bar
interface QueryActionsProps {
  query: string;
  selectedDatabase: DatabaseInfo | null;
  isExecuting: boolean;
  onRun: () => void;
}

export function QueryActions({
  query,
  selectedDatabase,
  isExecuting,
  onRun,
}: QueryActionsProps) {
  return (
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
      <ActionButton
        onClick={onRun}
        disabled={!query.trim() || !selectedDatabase}
        isLoading={isExecuting}
      >
        <Play className="w-3 h-3" />
        Run
        <span className="text-[10px] text-cyan-400/60 ml-1">⌘↵</span>
      </ActionButton>
    </div>
  );
}

// Results Header
interface ResultsHeaderProps {
  result: QueryResult | null;
}

export function ResultsHeader({ result }: ResultsHeaderProps) {
  return (
    <div className="flex items-center justify-between mb-2">
      <h3 className="text-xs font-semibold text-white">Results</h3>
      {result && result.row_count > 0 && (
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-slate-400 tabular-nums">
            {result.row_count} rows
            {result.execution_time_ms > 0 && (
              <span className="text-slate-500 ml-1">
                ({result.execution_time_ms}ms)
              </span>
            )}
          </span>
          <button className="px-2 py-0.5 text-[10px] text-slate-400 hover:text-white hover:bg-slate-700 transition-colors">
            Export
          </button>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// ORGANISM COMPONENTS
// ============================================================================

// Results Table
interface ResultsTableProps {
  result: QueryResult;
  density: DensityConfig;
  onCopyRow: (rowData: Record<string, any>) => void;
}

export function ResultsTable({ result, density: d, onCopyRow }: ResultsTableProps) {
  return (
    <div className="overflow-auto flex-1">
      <table className="w-full text-xs">
        <thead className="sticky top-0 z-10">
          <tr className="bg-slate-900/80 backdrop-blur-sm">
            {result.columns.map((col, colIdx) => (
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
          {result.rows.map((row, i) => (
            <ResultsRow
              key={i}
              row={row}
              density={d}
              onCopy={() =>
                onCopyRow(
                  Object.fromEntries(result.columns.map((col, idx) => [col, row[idx]]))
                )
              }
            />
          ))}
        </tbody>
      </table>
    </div>
  );
}

// Individual Results Row
interface ResultsRowProps {
  row: any[];
  density: DensityConfig;
  onCopy: () => void;
}

function ResultsRow({ row, density: d, onCopy }: ResultsRowProps) {
  return (
    <tr className="group border-b border-slate-800/50 hover:bg-slate-800/30">
      {row.map((value: any, j) => (
        <ResultsCell key={j} value={value} density={d} />
      ))}
      <td className="px-2 py-1">
        <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
          <IconButton
            icon={<Copy className="w-3 h-3 text-slate-400" />}
            onClick={onCopy}
            title="Copy row"
          />
          <IconButton
            icon={<Maximize2 className="w-3 h-3 text-slate-400" />}
            onClick={() => {}}
            title="Expand"
          />
        </div>
      </td>
    </tr>
  );
}

// Individual Results Cell
interface ResultsCellProps {
  value: any;
  density: DensityConfig;
}

function ResultsCell({ value, density: d }: ResultsCellProps) {
  const displayValue =
    value === null ? (
      <span className="text-slate-500 italic">NULL</span>
    ) : typeof value === 'object' ? (
      JSON.stringify(value)
    ) : (
      String(value)
    );

  return (
    <td
      className={clsx('text-slate-300 truncate max-w-[200px]', d.tableCell)}
      title={
        value === null
          ? 'NULL'
          : typeof value === 'object'
          ? JSON.stringify(value)
          : String(value)
      }
    >
      {displayValue}
    </td>
  );
}

// ============================================================================
// MAIN COMPONENT
// ============================================================================

interface QueryWorkspaceProps {
  selectedDatabase: DatabaseInfo | null;
  query: string;
  queryResult: QueryResult | null;
  isExecuting: boolean;
  error: string | null;
  safeMode: boolean;
  density: DensityMode;
  densityConfig: DensityConfig;
  queryEditorRef: RefObject<HTMLTextAreaElement>;
  onQueryChange: (query: string) => void;
  onExecuteQuery: () => void;
  onToggleSafeMode: () => void;
  onToggleDensity: () => void;
}

export function QueryWorkspace({
  selectedDatabase,
  query,
  queryResult,
  isExecuting,
  error,
  safeMode,
  density,
  densityConfig: d,
  queryEditorRef,
  onQueryChange,
  onExecuteQuery,
  onToggleSafeMode,
  onToggleDensity,
}: QueryWorkspaceProps) {
  const [queryBuilderCollapsed, setQueryBuilderCollapsed] = useState(true);
  const [queryEditorCollapsed, setQueryEditorCollapsed] = useState(false);

  const copyToClipboard = (data: Record<string, any>) => {
    navigator.clipboard.writeText(JSON.stringify(data));
  };

  return (
    <div className={clsx('flex-1 flex flex-col min-w-0 h-full', d.gap)}>
      {/* Query Builder (Collapsible) - Placeholder */}
      {!queryBuilderCollapsed && (
        <div className={clsx('admin-card admin-card-glow', d.card)}>
          <CollapsibleHeader
            title="Visual Query Builder"
            isCollapsed={false}
            onToggle={() => setQueryBuilderCollapsed(true)}
          />
          <div className="text-center py-4 text-slate-500 text-xs">
            Visual query builder coming soon...
          </div>
        </div>
      )}

      {/* Query Editor */}
      <div className={clsx('admin-card admin-card-glow', d.card)}>
        <QueryToolbar
          selectedDatabase={selectedDatabase}
          density={density}
          safeMode={safeMode}
          queryBuilderCollapsed={queryBuilderCollapsed}
          queryEditorCollapsed={queryEditorCollapsed}
          onToggleDensity={onToggleDensity}
          onToggleSafeMode={onToggleSafeMode}
          onToggleQueryBuilder={() => setQueryBuilderCollapsed(false)}
          onToggleQueryEditor={() => setQueryEditorCollapsed(!queryEditorCollapsed)}
        />

        {!queryEditorCollapsed && (
          <>
            <QueryInput
              query={query}
              selectedDatabase={selectedDatabase}
              editorRef={queryEditorRef}
              onChange={onQueryChange}
            />
            <QueryActions
              query={query}
              selectedDatabase={selectedDatabase}
              isExecuting={isExecuting}
              onRun={onExecuteQuery}
            />
          </>
        )}
      </div>

      {/* Results Panel */}
      <div className={clsx('admin-card admin-card-glow flex-1 min-h-0 flex flex-col', d.card)}>
        <ResultsHeader result={queryResult} />

        {error && <ErrorAlert message={error} />}

        {queryResult && queryResult.rows.length > 0 ? (
          <ResultsTable
            result={queryResult}
            density={d}
            onCopyRow={copyToClipboard}
          />
        ) : (
          <EmptyState
            icon={<Database className="w-8 h-8 text-slate-600" />}
            title={
              selectedDatabase
                ? 'Run a query to see results'
                : 'Select a database to start'
            }
          />
        )}
      </div>
    </div>
  );
}

