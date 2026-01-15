// Data Explorer Types and Interfaces

import type {
  DatabaseInfo as ApiDatabaseInfo,
  TableSchema,
  QueryResult,
  QueryTemplate,
} from '../../api/explorerClient';

// Extended database info with UI state
export interface DatabaseInfo extends ApiDatabaseInfo {
  isExpanded?: boolean;
  tableSchemas?: TableSchema[];
}

// Density mode for compact/normal display
export type DensityMode = 'compact' | 'normal';

// Query history entry
export interface QueryHistoryEntry {
  query: string;
  time: Date;
  databaseId?: string;
}

// Density configuration
export const densityConfig = {
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
} as const;

export type DensityConfig = typeof densityConfig[DensityMode];

// Re-export API types
export type { TableSchema, QueryResult, QueryTemplate };
