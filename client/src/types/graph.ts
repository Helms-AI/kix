// Extended graph types for Knowledge Graph visualization

import type { GraphNode, GraphEdge } from './index';

// View mode for 2D/3D toggle
export type GraphViewMode = '2d' | '3d';

// Layout algorithm types
export type GraphLayoutType = 'force' | 'hierarchical' | 'radial' | 'grid';

// Clustering options
export type ClusterByType = 'domain' | 'category' | 'type' | 'none';

// Extended node with visualization properties
export interface GraphNodeExtended extends GraphNode {
  // Clustering metadata
  clusterId: string;
  clusterLabel: string;
  isClusterNode?: boolean;
  childCount?: number;

  // Source metadata (from API)
  source_domain?: string;
  description?: string;
  tags?: string[];
  created_at?: string;

  // Visual properties (computed)
  color?: string;
  size?: number;
  opacity?: number;

  // Force simulation properties
  x?: number;
  y?: number;
  z?: number;
  vx?: number;
  vy?: number;
  vz?: number;
  fx?: number | null;
  fy?: number | null;
  fz?: number | null;
}

// Extended edge with visualization properties
export interface GraphEdgeExtended extends GraphEdge {
  // Visual properties
  color?: string;
  opacity?: number;
  curvature?: number;

  // Computed properties
  isHighlighted?: boolean;
}

// Cluster representation for grouping
export interface GraphCluster {
  id: string;
  label: string;
  nodeCount: number;
  color: string;
  isExpanded: boolean;
  nodeIds: string[];
  centroid?: { x: number; y: number; z?: number };
}

// Layout configuration
export interface LayoutConfig {
  type: GraphLayoutType;
  params: {
    strength?: number;
    distance?: number;
    iterations?: number;
    radius?: number;
  };
}

// Graph settings (URL-synced)
export interface GraphSettings {
  view: GraphViewMode;
  layout: GraphLayoutType;
  clusterBy: ClusterByType;
  showLabels: boolean;
  showLegend: boolean;
  highlightDepth: number;
  selectedNodeId: string | null;
  searchQuery: string;
  expandedClusters: string[];
}

// Default graph settings
export const DEFAULT_GRAPH_SETTINGS: GraphSettings = {
  view: '2d',
  layout: 'force',
  clusterBy: 'none',
  showLabels: true,
  showLegend: true,
  highlightDepth: 2,
  selectedNodeId: null,
  searchQuery: '',
  expandedClusters: [],
};

// Graph color palette (matches design system)
export const GRAPH_COLORS = {
  // Backgrounds
  canvasBg: '#0f172a',      // slate-950
  canvasBg3d: '#020617',    // slate-950 darker for 3D
  panelBg: 'rgba(15, 23, 42, 0.95)', // slate-900/95

  // Primary accent
  primary: '#22d3ee',       // cyan-400
  primaryDim: '#0891b2',    // cyan-600
  primaryGlow: 'rgba(34, 211, 238, 0.3)',

  // Category colors (extended palette)
  categories: [
    '#06b6d4', // cyan-500
    '#10b981', // emerald-500
    '#8b5cf6', // violet-500
    '#f59e0b', // amber-500
    '#ef4444', // red-500
    '#ec4899', // pink-500
    '#6366f1', // indigo-500
    '#14b8a6', // teal-500
    '#84cc16', // lime-500
    '#f97316', // orange-500
    '#3b82f6', // blue-500
    '#a855f7', // purple-500
  ],

  // Entry type colors
  entryTypes: {
    document: '#3b82f6',    // blue-500
    article: '#10b981',     // emerald-500
    pdf: '#f43f5e',         // rose-500
    code: '#f59e0b',        // amber-500
  } as Record<string, string>,

  // Edge colors
  edgeDefault: 'rgba(34, 211, 238, 0.12)',
  edgeHighlight: 'rgba(34, 211, 238, 0.6)',
  edgeSelected: 'rgba(34, 211, 238, 0.9)',

  // Text colors
  labelDefault: '#94a3b8',  // slate-400
  labelHighlight: '#ffffff',
  labelDim: '#64748b',      // slate-500

  // Cluster colors
  clusterHull: 'rgba(34, 211, 238, 0.08)',
  clusterBorder: 'rgba(34, 211, 238, 0.25)',
} as const;

// Graph fonts
export const GRAPH_FONTS = {
  labels: 'JetBrains Mono, monospace',
  ui: 'IBM Plex Sans, system-ui, sans-serif',
} as const;

// Graph size constants
export const GRAPH_SIZES = {
  nodeMinRadius: 6,
  nodeMaxRadius: 16,
  nodeDefaultRadius: 8,
  nodeSelectedRadius: 12,
  labelFontSize: 11,
  labelFontSizeSelected: 13,
  edgeMinWidth: 0.5,
  edgeMaxWidth: 3,
  clusterPadding: 20,
} as const;

// Helper function to get category color
export function getCategoryColor(category: string, allCategories: string[]): string {
  const index = allCategories.indexOf(category);
  if (index === -1) return GRAPH_COLORS.categories[0];
  return GRAPH_COLORS.categories[index % GRAPH_COLORS.categories.length];
}

// Helper function to get entry type color
export function getEntryTypeColor(entryType: string): string {
  return GRAPH_COLORS.entryTypes[entryType] || GRAPH_COLORS.primary;
}

// Helper function to calculate node size based on connections
export function getNodeSize(connectionCount: number, maxConnections: number): number {
  const normalized = maxConnections > 0 ? connectionCount / maxConnections : 0;
  const { nodeMinRadius, nodeMaxRadius } = GRAPH_SIZES;
  return nodeMinRadius + normalized * (nodeMaxRadius - nodeMinRadius);
}
