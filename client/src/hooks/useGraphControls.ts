import { useCallback, useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';
import type { GraphSettings, GraphViewMode, GraphLayoutType, ClusterByType } from '../types/graph';
import { DEFAULT_GRAPH_SETTINGS } from '../types/graph';

type SettingKey = keyof GraphSettings;

export function useGraphControls() {
  const [searchParams, setSearchParams] = useSearchParams();

  // Parse settings from URL
  const settings: GraphSettings = useMemo(() => {
    const view = (searchParams.get('view') as GraphViewMode) || DEFAULT_GRAPH_SETTINGS.view;
    const layout = (searchParams.get('layout') as GraphLayoutType) || DEFAULT_GRAPH_SETTINGS.layout;
    const clusterBy = (searchParams.get('cluster') as ClusterByType) || DEFAULT_GRAPH_SETTINGS.clusterBy;
    const showLabels = searchParams.get('labels') !== 'false';
    const showLegend = searchParams.get('legend') !== 'false';
    const highlightDepth = parseInt(searchParams.get('depth') || '2', 10);
    const selectedNodeId = searchParams.get('node') || null;
    const searchQuery = searchParams.get('q') || '';
    const expandedClusters = searchParams.get('expanded')?.split(',').filter(Boolean) || [];

    return {
      view,
      layout,
      clusterBy,
      showLabels,
      showLegend,
      highlightDepth,
      selectedNodeId,
      searchQuery,
      expandedClusters,
    };
  }, [searchParams]);

  // Update a single setting
  const setSetting = useCallback(<K extends SettingKey>(
    key: K,
    value: GraphSettings[K]
  ) => {
    setSearchParams(prev => {
      const next = new URLSearchParams(prev);

      // Map setting keys to URL params
      const paramMap: Record<SettingKey, string> = {
        view: 'view',
        layout: 'layout',
        clusterBy: 'cluster',
        showLabels: 'labels',
        showLegend: 'legend',
        highlightDepth: 'depth',
        selectedNodeId: 'node',
        searchQuery: 'q',
        expandedClusters: 'expanded',
      };

      const param = paramMap[key];
      const defaultValue = DEFAULT_GRAPH_SETTINGS[key];

      // Handle different value types
      if (value === defaultValue || value === null || value === '') {
        next.delete(param);
      } else if (Array.isArray(value)) {
        if (value.length === 0) {
          next.delete(param);
        } else {
          next.set(param, value.join(','));
        }
      } else if (typeof value === 'boolean') {
        if (value === true) {
          next.delete(param); // true is default, don't store
        } else {
          next.set(param, 'false');
        }
      } else {
        next.set(param, String(value));
      }

      return next;
    }, { replace: true });
  }, [setSearchParams]);

  // Update multiple settings at once
  const setMultipleSettings = useCallback((updates: Partial<GraphSettings>) => {
    setSearchParams(prev => {
      const next = new URLSearchParams(prev);

      const paramMap: Record<SettingKey, string> = {
        view: 'view',
        layout: 'layout',
        clusterBy: 'cluster',
        showLabels: 'labels',
        showLegend: 'legend',
        highlightDepth: 'depth',
        selectedNodeId: 'node',
        searchQuery: 'q',
        expandedClusters: 'expanded',
      };

      Object.entries(updates).forEach(([key, value]) => {
        const param = paramMap[key as SettingKey];
        const defaultValue = DEFAULT_GRAPH_SETTINGS[key as SettingKey];

        if (value === defaultValue || value === null || value === '') {
          next.delete(param);
        } else if (Array.isArray(value)) {
          if (value.length === 0) {
            next.delete(param);
          } else {
            next.set(param, value.join(','));
          }
        } else if (typeof value === 'boolean') {
          if (value === true) {
            next.delete(param);
          } else {
            next.set(param, 'false');
          }
        } else {
          next.set(param, String(value));
        }
      });

      return next;
    }, { replace: true });
  }, [setSearchParams]);

  // Reset all settings to defaults
  const resetSettings = useCallback(() => {
    setSearchParams({}, { replace: true });
  }, [setSearchParams]);

  // Convenience setters
  const setView = useCallback((view: GraphViewMode) => setSetting('view', view), [setSetting]);
  const setLayout = useCallback((layout: GraphLayoutType) => setSetting('layout', layout), [setSetting]);
  const setClusterBy = useCallback((clusterBy: ClusterByType) => setSetting('clusterBy', clusterBy), [setSetting]);
  const setShowLabels = useCallback((show: boolean) => setSetting('showLabels', show), [setSetting]);
  const setShowLegend = useCallback((show: boolean) => setSetting('showLegend', show), [setSetting]);
  const setHighlightDepth = useCallback((depth: number) => setSetting('highlightDepth', depth), [setSetting]);
  const selectNode = useCallback((nodeId: string | null) => setSetting('selectedNodeId', nodeId), [setSetting]);
  const setSearchQuery = useCallback((query: string) => setSetting('searchQuery', query), [setSetting]);

  // Toggle cluster expansion
  const toggleCluster = useCallback((clusterId: string) => {
    setSearchParams(prev => {
      const next = new URLSearchParams(prev);
      const expanded = prev.get('expanded')?.split(',').filter(Boolean) || [];

      if (expanded.includes(clusterId)) {
        const newExpanded = expanded.filter(id => id !== clusterId);
        if (newExpanded.length === 0) {
          next.delete('expanded');
        } else {
          next.set('expanded', newExpanded.join(','));
        }
      } else {
        next.set('expanded', [...expanded, clusterId].join(','));
      }

      return next;
    }, { replace: true });
  }, [setSearchParams]);

  return {
    settings,
    setSetting,
    setMultipleSettings,
    resetSettings,
    // Convenience methods
    setView,
    setLayout,
    setClusterBy,
    setShowLabels,
    setShowLegend,
    setHighlightDepth,
    selectNode,
    setSearchQuery,
    toggleCluster,
  };
}
