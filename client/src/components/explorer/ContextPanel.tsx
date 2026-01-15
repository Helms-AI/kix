import clsx from 'clsx';
import {
  ChevronRight,
  ChevronLeft,
  Table2,
  History,
  BookTemplate,
} from 'lucide-react';
import type { DatabaseInfo, QueryTemplate, QueryHistoryEntry, DensityConfig } from './types';

// ============================================================================
// ATOMIC COMPONENTS
// ============================================================================

// Section Header
interface SectionHeaderProps {
  title: string;
}

export function SectionHeader({ title }: SectionHeaderProps) {
  return (
    <h3 className="text-[10px] font-semibold text-slate-400 uppercase tracking-wider mb-1.5">
      {title}
    </h3>
  );
}

// Info Item - displays a label/value pair
interface InfoItemProps {
  label: string;
  value: string;
  truncate?: boolean;
  uppercase?: boolean;
}

export function InfoItem({ label, value, truncate = true, uppercase = false }: InfoItemProps) {
  return (
    <div className="px-2 py-1 bg-slate-800/50">
      <p className="text-[10px] text-slate-500">{label}</p>
      <p
        className={clsx(
          'text-xs text-white font-medium',
          truncate && 'truncate',
          uppercase && 'uppercase'
        )}
      >
        {value}
      </p>
    </div>
  );
}

// Shortcut Item - displays a keyboard shortcut
interface ShortcutItemProps {
  label: string;
  shortcut: string;
}

export function ShortcutItem({ label, shortcut }: ShortcutItemProps) {
  return (
    <div className="flex justify-between text-slate-500">
      <span>{label}</span>
      <span className="text-slate-400">{shortcut}</span>
    </div>
  );
}

// Collapsed Icon Button
interface CollapsedIconButtonProps {
  icon: React.ReactNode;
  title: string;
  onClick: () => void;
}

export function CollapsedIconButton({ icon, title, onClick }: CollapsedIconButtonProps) {
  return (
    <button
      onClick={onClick}
      className="w-6 h-6 flex items-center justify-center hover:bg-slate-700"
      title={`${title} - Click to expand`}
    >
      {icon}
    </button>
  );
}

// ============================================================================
// MOLECULE COMPONENTS
// ============================================================================

// Schema Details Section
interface SchemaDetailsProps {
  selectedDatabase: DatabaseInfo | null;
  selectedTable: string | null;
}

export function SchemaDetails({ selectedDatabase, selectedTable }: SchemaDetailsProps) {
  if (!selectedTable || !selectedDatabase) return null;

  return (
    <div>
      <SectionHeader title="Schema" />
      <div className="space-y-1">
        <InfoItem label="Table" value={selectedTable} />
        <InfoItem label="Database" value={selectedDatabase.name} />
        <InfoItem label="Type" value={selectedDatabase.db_type} uppercase />
      </div>
    </div>
  );
}

// Query History Section
interface QueryHistorySectionProps {
  history: QueryHistoryEntry[];
  onSelectQuery: (query: string) => void;
}

export function QueryHistorySection({ history, onSelectQuery }: QueryHistorySectionProps) {
  return (
    <div>
      <SectionHeader title="History" />
      <div className="space-y-1 max-h-32 overflow-y-auto">
        {history.length > 0 ? (
          history.map((entry, i) => (
            <HistoryItem
              key={i}
              entry={entry}
              onClick={() => onSelectQuery(entry.query)}
            />
          ))
        ) : (
          <p className="text-[10px] text-slate-500 px-2">No history yet</p>
        )}
      </div>
    </div>
  );
}

// Individual History Item
interface HistoryItemProps {
  entry: QueryHistoryEntry;
  onClick: () => void;
}

function HistoryItem({ entry, onClick }: HistoryItemProps) {
  const ago = Math.round((Date.now() - entry.time.getTime()) / 60000);

  return (
    <div
      className="px-2 py-1 bg-slate-800/50 hover:bg-slate-700/50 cursor-pointer"
      onClick={onClick}
    >
      <p className="text-[10px] text-slate-500">
        {ago < 1 ? 'Just now' : `${ago}m ago`}
      </p>
      <p className="text-[11px] text-white font-mono truncate">
        {entry.query.slice(0, 50)}
      </p>
    </div>
  );
}

// Templates Section
interface TemplatesSectionProps {
  templates: QueryTemplate[];
  selectedDatabase: DatabaseInfo | null;
  onApplyTemplate: (template: QueryTemplate) => void;
}

export function TemplatesSection({
  templates,
  selectedDatabase,
  onApplyTemplate,
}: TemplatesSectionProps) {
  const filteredTemplates = templates
    .filter(t => !selectedDatabase || t.database_type === selectedDatabase.db_type)
    .slice(0, 5);

  return (
    <div>
      <SectionHeader title="Templates" />
      <div className="space-y-1 max-h-40 overflow-y-auto">
        {filteredTemplates.length > 0 ? (
          filteredTemplates.map(template => (
            <TemplateItem
              key={template.id}
              template={template}
              onClick={() => onApplyTemplate(template)}
            />
          ))
        ) : (
          <p className="text-[10px] text-slate-500 px-2">
            {templates.length === 0 ? 'Loading templates...' : 'No matching templates'}
          </p>
        )}
      </div>
    </div>
  );
}

// Individual Template Item
interface TemplateItemProps {
  template: QueryTemplate;
  onClick: () => void;
}

function TemplateItem({ template, onClick }: TemplateItemProps) {
  return (
    <button
      className="w-full px-2 py-1 bg-slate-800/50 hover:bg-slate-700/50 text-left"
      onClick={onClick}
    >
      <p className="text-[11px] text-white">{template.name}</p>
      <p className="text-[10px] text-slate-500 truncate">{template.description}</p>
    </button>
  );
}

// Keyboard Shortcuts Section
export function KeyboardShortcuts() {
  return (
    <div className="pt-2 border-t border-slate-700/50">
      <SectionHeader title="Shortcuts" />
      <div className="space-y-0.5 text-[10px]">
        <ShortcutItem label="Run query" shortcut="⌘↵" />
        <ShortcutItem label="Search" shortcut="⌘K" />
        <ShortcutItem label="Toggle sidebar" shortcut="⌘B" />
      </div>
    </div>
  );
}

// ============================================================================
// ORGANISM COMPONENTS
// ============================================================================

// Collapsed Panel View
interface CollapsedPanelViewProps {
  onExpand: () => void;
}

function CollapsedPanelView({ onExpand }: CollapsedPanelViewProps) {
  return (
    <div className="flex flex-col items-center gap-1 pt-1">
      <CollapsedIconButton
        icon={<Table2 className="w-3.5 h-3.5 text-slate-400" />}
        title="Schema"
        onClick={onExpand}
      />
      <CollapsedIconButton
        icon={<History className="w-3.5 h-3.5 text-slate-400" />}
        title="History"
        onClick={onExpand}
      />
      <CollapsedIconButton
        icon={<BookTemplate className="w-3.5 h-3.5 text-slate-400" />}
        title="Templates"
        onClick={onExpand}
      />
    </div>
  );
}

// Expanded Panel View
interface ExpandedPanelViewProps {
  selectedDatabase: DatabaseInfo | null;
  selectedTable: string | null;
  queryHistory: QueryHistoryEntry[];
  templates: QueryTemplate[];
  onSelectQuery: (query: string) => void;
  onApplyTemplate: (template: QueryTemplate) => void;
}

function ExpandedPanelView({
  selectedDatabase,
  selectedTable,
  queryHistory,
  templates,
  onSelectQuery,
  onApplyTemplate,
}: ExpandedPanelViewProps) {
  return (
    <div className="space-y-3">
      <SchemaDetails
        selectedDatabase={selectedDatabase}
        selectedTable={selectedTable}
      />
      <QueryHistorySection
        history={queryHistory}
        onSelectQuery={onSelectQuery}
      />
      <TemplatesSection
        templates={templates}
        selectedDatabase={selectedDatabase}
        onApplyTemplate={onApplyTemplate}
      />
      <KeyboardShortcuts />
    </div>
  );
}

// ============================================================================
// MAIN COMPONENT
// ============================================================================

interface ContextPanelProps {
  selectedDatabase: DatabaseInfo | null;
  selectedTable: string | null;
  queryHistory: QueryHistoryEntry[];
  templates: QueryTemplate[];
  isCollapsed: boolean;
  density: DensityConfig;
  onSelectQuery: (query: string) => void;
  onApplyTemplate: (template: QueryTemplate) => void;
  onToggleCollapse: () => void;
}

export function ContextPanel({
  selectedDatabase,
  selectedTable,
  queryHistory,
  templates,
  isCollapsed,
  density: d,
  onSelectQuery,
  onApplyTemplate,
  onToggleCollapse,
}: ContextPanelProps) {
  return (
    <div
      className={clsx(
        'transition-all duration-200 flex-shrink-0 h-full',
        isCollapsed ? d.rightSidebar.collapsed : d.rightSidebar.expanded
      )}
    >
      <div className={clsx('admin-card admin-card-glow h-full relative', d.card)}>
        {/* Collapse Toggle */}
        <button
          onClick={onToggleCollapse}
          className="absolute -left-2 top-3 z-10 w-5 h-5 rounded-full bg-slate-700 border border-slate-600 flex items-center justify-center hover:bg-slate-600 transition-colors"
          title={isCollapsed ? 'Expand (⌘⇧B)' : 'Collapse (⌘⇧B)'}
        >
          {isCollapsed ? (
            <ChevronLeft className="w-2.5 h-2.5 text-slate-400" />
          ) : (
            <ChevronRight className="w-2.5 h-2.5 text-slate-400" />
          )}
        </button>

        {isCollapsed ? (
          <CollapsedPanelView onExpand={onToggleCollapse} />
        ) : (
          <ExpandedPanelView
            selectedDatabase={selectedDatabase}
            selectedTable={selectedTable}
            queryHistory={queryHistory}
            templates={templates}
            onSelectQuery={onSelectQuery}
            onApplyTemplate={onApplyTemplate}
          />
        )}
      </div>
    </div>
  );
}
