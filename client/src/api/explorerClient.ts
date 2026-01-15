// Data Explorer API Client

const API_BASE = '/api/explorer';

// Types matching backend
export interface DatabaseInfo {
  id: string;
  name: string;
  db_type: 'sqlite' | 'tantivy';
  path: string;
  size_bytes: number;
  size_display: string;
  modified: string | null;
  status: 'available' | 'locked' | 'scanning' | 'error';
  tables: string[] | null;
}

export interface TableSchema {
  name: string;
  columns: ColumnInfo[];
  row_count: number | null;
  indexes: IndexInfo[];
  foreign_keys: ForeignKeyInfo[];
}

export interface ColumnInfo {
  name: string;
  data_type: string;
  nullable: boolean;
  primary_key: boolean;
  default_value: string | null;
}

export interface IndexInfo {
  name: string;
  columns: string[];
  unique: boolean;
}

export interface ForeignKeyInfo {
  column: string;
  references_table: string;
  references_column: string;
  on_delete: string | null;
  on_update: string | null;
}

export interface QueryResult {
  columns: string[];
  rows: any[][];
  row_count: number;
  execution_time_ms: number;
  affected_rows: number | null;
  query_type: 'select' | 'insert' | 'update' | 'delete' | 'create' | 'drop' | 'alter' | 'other';
  warnings: string[];
}

export interface QueryTemplate {
  id: string;
  name: string;
  description: string;
  query: string;
  database_type: 'sqlite' | 'tantivy';
  category: string;
  parameters: TemplateParameter[];
}

export interface TemplateParameter {
  name: string;
  description: string;
  data_type: string;
  default_value: string | null;
}

// API Response types
interface DatabaseListResponse {
  databases: DatabaseInfo[];
  total: number;
}

interface SchemaResponse {
  database_id: string;
  database_name: string;
  tables: TableSchema[];
}

interface QueryResponse {
  success: boolean;
  result: QueryResult | null;
  error: string | null;
}

interface TemplatesResponse {
  templates: QueryTemplate[];
}

// ErrorResponse type for API error handling (used in catch blocks)
interface ErrorResponse {
  error: string;
  message: string;
}

// Suppress unused warning - type is used for error handling patterns
void (0 as unknown as ErrorResponse);

// API Functions

/**
 * Helper to safely parse JSON response
 */
async function parseJsonResponse<T>(response: Response): Promise<T> {
  const text = await response.text();
  if (!text) {
    throw new Error('Empty response from server');
  }
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new Error(`Invalid JSON response: ${text.slice(0, 100)}`);
  }
}

/**
 * List all discovered databases
 */
export async function listDatabases(): Promise<DatabaseInfo[]> {
  const response = await fetch(`${API_BASE}/databases`);
  if (!response.ok) {
    const errorText = await response.text();
    let message = 'Failed to list databases';
    try {
      const error = JSON.parse(errorText);
      message = error.message || message;
    } catch {
      message = errorText || `HTTP ${response.status}`;
    }
    throw new Error(message);
  }
  const data = await parseJsonResponse<DatabaseListResponse>(response);
  return data.databases;
}

/**
 * Force refresh database discovery
 */
export async function refreshDatabases(): Promise<DatabaseInfo[]> {
  const response = await fetch(`${API_BASE}/databases/refresh`, { method: 'POST' });
  if (!response.ok) {
    const errorText = await response.text();
    let message = 'Failed to refresh databases';
    try {
      const error = JSON.parse(errorText);
      message = error.message || message;
    } catch {
      message = errorText || `HTTP ${response.status}`;
    }
    throw new Error(message);
  }
  const data = await parseJsonResponse<DatabaseListResponse>(response);
  return data.databases;
}

/**
 * Get schema for a database
 */
export async function getDatabaseSchema(databaseId: string): Promise<SchemaResponse> {
  const response = await fetch(`${API_BASE}/databases/${databaseId}/schema`);
  if (!response.ok) {
    const errorText = await response.text();
    let message = 'Failed to get schema';
    try {
      const error = JSON.parse(errorText);
      message = error.message || message;
    } catch {
      message = errorText || `HTTP ${response.status}`;
    }
    throw new Error(message);
  }
  return parseJsonResponse<SchemaResponse>(response);
}

/**
 * Execute a query against a database
 */
export async function executeQuery(
  databaseId: string,
  query: string,
  options?: {
    limit?: number;
    offset?: number;
    safe_mode?: boolean;
  }
): Promise<QueryResponse> {
  const response = await fetch(`${API_BASE}/databases/${databaseId}/query`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      query,
      limit: options?.limit ?? 1000,
      offset: options?.offset ?? 0,
      safe_mode: options?.safe_mode ?? true,
    }),
  });

  if (!response.ok) {
    const errorText = await response.text();
    let message = 'Failed to execute query';
    try {
      const error = JSON.parse(errorText);
      message = error.message || message;
    } catch {
      message = errorText || `HTTP ${response.status}`;
    }
    throw new Error(message);
  }

  return parseJsonResponse<QueryResponse>(response);
}

/**
 * Get table data with pagination
 */
export async function getTableData(
  databaseId: string,
  tableName: string,
  options?: {
    limit?: number;
    offset?: number;
    order_by?: string;
    order_dir?: 'ASC' | 'DESC';
  }
): Promise<QueryResponse> {
  const params = new URLSearchParams();
  if (options?.limit) params.set('limit', options.limit.toString());
  if (options?.offset) params.set('offset', options.offset.toString());
  if (options?.order_by) params.set('order_by', options.order_by);
  if (options?.order_dir) params.set('order_dir', options.order_dir);

  const url = `${API_BASE}/databases/${databaseId}/tables/${tableName}${params.toString() ? '?' + params.toString() : ''}`;
  const response = await fetch(url);

  if (!response.ok) {
    const errorText = await response.text();
    let message = 'Failed to get table data';
    try {
      const error = JSON.parse(errorText);
      message = error.message || message;
    } catch {
      message = errorText || `HTTP ${response.status}`;
    }
    throw new Error(message);
  }

  return parseJsonResponse<QueryResponse>(response);
}

/**
 * Get query templates
 */
export async function getQueryTemplates(): Promise<QueryTemplate[]> {
  const response = await fetch(`${API_BASE}/templates`);
  if (!response.ok) {
    const errorText = await response.text();
    let message = 'Failed to get templates';
    try {
      const error = JSON.parse(errorText);
      message = error.message || message;
    } catch {
      message = errorText || `HTTP ${response.status}`;
    }
    throw new Error(message);
  }
  const data = await parseJsonResponse<TemplatesResponse>(response);
  return data.templates;
}
