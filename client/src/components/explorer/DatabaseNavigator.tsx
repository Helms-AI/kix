import { RefObject } from 'react';
import clsx from 'clsx';
import {
  Database,
  Search,
  ChevronRight,
  ChevronDown,
  ChevronLeft,
  Table2,
  RefreshCw,
  AlertCircle,
  Loader2,
} from 'lucide-react';
import type { DatabaseInfo, DensityConfig } from './types';

interface DatabaseNavigatorProps {
  databases: DatabaseInfo[];
  selectedDatabase: DatabaseInfo | null;
  selectedTable: string | null;
  isCollapsed: boolean;
  isScanning: boolean;
  density: DensityConfig;
  searchInputRef: RefObject<HTMLInputElement>;
  onSelectDatabase: (db: DatabaseInfo) => void;
  onSelectTable: (db: DatabaseInfo, tableName: string) => void;
  onToggleExpand: (dbId: string) => void;
  onRefresh: () => void;
  onToggleCollapse: () => void;
}

export function DatabaseNavigator({
  databases,
  selectedDatabase,
  selectedTable,
  isCollapsed,
  isScanning,
  density: d,
  searchInputRef,
  onSelectDatabase,
  onSelectTable,
  onToggleExpand,
  onRefresh,
  onToggleCollapse,
}: DatabaseNavigatorProps) {
  return (
    <div
      className={clsx(
        'transition-all duration-200 flex-shrink-0 h-full',
        isCollapsed ? d.sidebar.collapsed : d.sidebar.expanded
      )}
    >
      <div className={clsx('admin-card admin-card-glow h-full relative', d.card)}>
        {/* Collapse Toggle */}
        <button
          onClick={onToggleCollapse}
          className="absolute -right-2 top-3 z-10 w-5 h-5 rounded-full bg-slate-700 border border-slate-600 flex items-center justify-center hover:bg-slate-600 transition-colors"
          title={isCollapsed ? 'Expand (⌘B)' : 'Collapse (⌘B)'}
        >
          {isCollapsed ? (
            <ChevronRight className="w-2.5 h-2.5 text-slate-400" />
          ) : (
            <ChevronLeft className="w-2.5 h-2.5 text-slate-400" />
          )}
        </button>

        {isCollapsed ? (
          <CollapsedView
            databases={databases}
            selectedDatabase={selectedDatabase}
            onSelectDatabase={onSelectDatabase}
            onExpand={onToggleCollapse}
          />
        ) : (
          <ExpandedView
            databases={databases}
            selectedDatabase={selectedDatabase}
            selectedTable={selectedTable}
            isScanning={isScanning}
            searchInputRef={searchInputRef}
            onSelectDatabase={onSelectDatabase}
            onSelectTable={onSelectTable}
            onToggleExpand={onToggleExpand}
            onRefresh={onRefresh}
          />
        )}
      </div>
    </div>
  );
}

// Collapsed sidebar view - icon stack
interface CollapsedViewProps {
  databases: DatabaseInfo[];
  selectedDatabase: DatabaseInfo | null;
  onSelectDatabase: (db: DatabaseInfo) => void;
  onExpand: () => void;
}

function CollapsedView({
  databases,
  selectedDatabase,
  onSelectDatabase,
  onExpand,
}: CollapsedViewProps) {
  return (
    <div className="flex flex-col items-center gap-1 pt-1">
      {databases.map(db => (
        <button
          key={db.id}
          onClick={() => {
            onExpand();
            onSelectDatabase(db);
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
  );
}

// Expanded sidebar view - full navigation
interface ExpandedViewProps {
  databases: DatabaseInfo[];
  selectedDatabase: DatabaseInfo | null;
  selectedTable: string | null;
  isScanning: boolean;
  searchInputRef: RefObject<HTMLInputElement>;
  onSelectDatabase: (db: DatabaseInfo) => void;
  onSelectTable: (db: DatabaseInfo, tableName: string) => void;
  onToggleExpand: (dbId: string) => void;
  onRefresh: () => void;
}

function ExpandedView({
  databases,
  selectedDatabase,
  selectedTable,
  isScanning,
  searchInputRef,
  onSelectDatabase,
  onSelectTable,
  onToggleExpand,
  onRefresh,
}: ExpandedViewProps) {
  return (
    <>
      {/* Header */}
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-xs font-semibold text-white flex items-center gap-1.5">
          <Database className="w-3.5 h-3.5 text-cyan-400" />
          Databases
        </h3>
        <button
          onClick={onRefresh}
          className="p-1 hover:bg-slate-700 transition-colors"
          disabled={isScanning}
          title="Refresh"
        >
          <RefreshCw
            className={clsx(
              'w-3 h-3 text-slate-400',
              isScanning && 'animate-spin'
            )}
          />
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
          <DatabaseNode
            key={db.id}
            database={db}
            isSelected={selectedDatabase?.id === db.id}
            selectedTable={selectedTable}
            onSelect={() => onSelectDatabase(db)}
            onToggleExpand={() => onToggleExpand(db.id)}
            onSelectTable={(tableName) => onSelectTable(db, tableName)}
          />
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
  );
}

// Individual database node in the tree
interface DatabaseNodeProps {
  database: DatabaseInfo;
  isSelected: boolean;
  selectedTable: string | null;
  onSelect: () => void;
  onToggleExpand: () => void;
  onSelectTable: (tableName: string) => void;
}

function DatabaseNode({
  database: db,
  isSelected,
  selectedTable,
  onSelect,
  onToggleExpand,
  onSelectTable,
}: DatabaseNodeProps) {
  return (
    <div>
      {/* Database Row */}
      <div
        className={clsx(
          'group flex items-center gap-1.5 px-1.5 py-1 cursor-pointer transition-colors',
          isSelected
            ? 'bg-cyan-500/10 text-cyan-400'
            : 'hover:bg-slate-800/50 text-slate-300'
        )}
        onClick={onSelect}
      >
        <button
          onClick={(e) => {
            e.stopPropagation();
            onToggleExpand();
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
          <p className="text-[11px] font-medium truncate leading-tight">
            {db.name}
          </p>
          <p className="text-[10px] text-slate-500 leading-tight">
            {db.size_display}
          </p>
        </div>
        {db.status === 'locked' && (
          <AlertCircle className="w-3 h-3 text-amber-400 flex-shrink-0" />
        )}
      </div>

      {/* Tables */}
      {db.isExpanded && db.tableSchemas && (
        <div className="ml-4 mt-0.5 space-y-0">
          {db.tableSchemas.map(table => (
            <TableNode
              key={table.name}
              tableName={table.name}
              rowCount={table.row_count}
              isSelected={selectedTable === table.name && isSelected}
              onSelect={() => onSelectTable(table.name)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// Individual table node
interface TableNodeProps {
  tableName: string;
  rowCount: number | null;
  isSelected: boolean;
  onSelect: () => void;
}

function TableNode({ tableName, rowCount, isSelected, onSelect }: TableNodeProps) {
  return (
    <div
      className={clsx(
        'group flex items-center gap-1.5 px-1.5 py-0.5 cursor-pointer transition-colors text-[11px]',
        isSelected
          ? 'bg-slate-700/50 text-cyan-400'
          : 'hover:bg-slate-800/30 text-slate-400'
      )}
      onClick={onSelect}
    >
      <Table2 className="w-3 h-3 flex-shrink-0" />
      <span className="flex-1 truncate">{tableName}</span>
      {rowCount !== null && (
        <span className="text-[10px] text-slate-500 tabular-nums">
          {rowCount.toLocaleString()}
        </span>
      )}
    </div>
  );
}
