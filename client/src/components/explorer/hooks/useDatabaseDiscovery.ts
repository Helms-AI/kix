import { useState, useCallback, useEffect } from 'react';
import {
  listDatabases,
  refreshDatabases,
  getDatabaseSchema,
} from '../../../api/explorerClient';
import type { DatabaseInfo } from '../types';

interface UseDatabaseDiscoveryReturn {
  databases: DatabaseInfo[];
  isScanning: boolean;
  error: string | null;
  discoverDatabases: () => Promise<void>;
  refreshDatabaseList: () => Promise<void>;
  loadDatabaseSchema: (db: DatabaseInfo) => Promise<void>;
  toggleDatabaseExpand: (dbId: string) => Promise<void>;
  updateDatabase: (dbId: string, updates: Partial<DatabaseInfo>) => void;
}

/**
 * Hook for managing database discovery and schema loading
 */
export function useDatabaseDiscovery(): UseDatabaseDiscoveryReturn {
  const [databases, setDatabases] = useState<DatabaseInfo[]>([]);
  const [isScanning, setIsScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const discoverDatabases = useCallback(async () => {
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
  }, []);

  const refreshDatabaseList = useCallback(async () => {
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
  }, []);

  const loadDatabaseSchema = useCallback(async (db: DatabaseInfo) => {
    if (db.db_type !== 'sqlite' || db.tableSchemas) return;

    try {
      const schema = await getDatabaseSchema(db.id);
      setDatabases(prev =>
        prev.map(d =>
          d.id === db.id ? { ...d, tableSchemas: schema.tables } : d
        )
      );
    } catch (e) {
      console.error('Failed to load schema:', e);
    }
  }, []);

  const toggleDatabaseExpand = useCallback(
    async (dbId: string) => {
      const db = databases.find(d => d.id === dbId);
      if (db && !db.isExpanded && !db.tableSchemas) {
        await loadDatabaseSchema(db);
      }
      setDatabases(prev =>
        prev.map(d =>
          d.id === dbId ? { ...d, isExpanded: !d.isExpanded } : d
        )
      );
    },
    [databases, loadDatabaseSchema]
  );

  const updateDatabase = useCallback(
    (dbId: string, updates: Partial<DatabaseInfo>) => {
      setDatabases(prev =>
        prev.map(d => (d.id === dbId ? { ...d, ...updates } : d))
      );
    },
    []
  );

  // Discover databases on mount
  useEffect(() => {
    discoverDatabases();
  }, [discoverDatabases]);

  return {
    databases,
    isScanning,
    error,
    discoverDatabases,
    refreshDatabaseList,
    loadDatabaseSchema,
    toggleDatabaseExpand,
    updateDatabase,
  };
}
