import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { api } from '../api/client';
import type { GraphNodeExtended, GraphEdgeExtended, GraphCluster, ClusterByType } from '../types/graph';
import { getCategoryColor, getEntryTypeColor, getNodeSize, GRAPH_COLORS } from '../types/graph';

interface TransformedGraphData {
  nodes: GraphNodeExtended[];
  edges: GraphEdgeExtended[];
  clusters: GraphCluster[];
  categories: string[];
  domains: string[];
  maxConnections: number;
}

// Group nodes into clusters based on clustering type
function createClusters(
  nodes: GraphNodeExtended[],
  clusterBy: ClusterByType
): GraphCluster[] {
  if (clusterBy === 'none') return [];

  const clusterMap = new Map<string, GraphNodeExtended[]>();

  nodes.forEach(node => {
    let key: string;
    switch (clusterBy) {
      case 'domain':
        key = node.source_domain || 'Unknown';
        break;
      case 'category':
        key = node.category || 'Uncategorized';
        break;
      case 'type':
        key = node.entry_type || 'Unknown';
        break;
      default:
        key = 'all';
    }

    if (!clusterMap.has(key)) {
      clusterMap.set(key, []);
    }
    clusterMap.get(key)!.push(node);
  });

  return Array.from(clusterMap.entries()).map(([key, clusterNodes]) => ({
    id: `cluster-${key}`,
    label: key,
    nodeCount: clusterNodes.length,
    color: clusterNodes[0]?.color || GRAPH_COLORS.primary,
    isExpanded: true,
    nodeIds: clusterNodes.map(n => n.id),
  }));
}

// Transform raw API data into visualization-ready format
function transformGraphData(
  rawNodes: Array<{ id: string; label: string; entry_type: string; category: string }>,
  rawEdges: Array<{ source: string; target: string; weight: number }>,
  clusterBy: ClusterByType
): TransformedGraphData {
  // Get all unique categories and domains
  const categories = [...new Set(rawNodes.map(n => n.category).filter(Boolean))];
  const domains = [...new Set(rawNodes.map(n => (n as any).source_domain).filter(Boolean))];

  // Count max connections for node sizing
  const connectionCounts = new Map<string, number>();
  rawEdges.forEach(edge => {
    connectionCounts.set(edge.source, (connectionCounts.get(edge.source) || 0) + 1);
    connectionCounts.set(edge.target, (connectionCounts.get(edge.target) || 0) + 1);
  });
  const maxConnections = Math.max(...Array.from(connectionCounts.values()), 1);

  // Transform nodes with extended properties
  const nodes: GraphNodeExtended[] = rawNodes.map(node => {
    const connections = connectionCounts.get(node.id) || 0;

    // Determine cluster based on clustering type
    let clusterId: string;
    let clusterLabel: string;
    switch (clusterBy) {
      case 'domain':
        clusterId = `cluster-${(node as any).source_domain || 'Unknown'}`;
        clusterLabel = (node as any).source_domain || 'Unknown';
        break;
      case 'category':
        clusterId = `cluster-${node.category || 'Uncategorized'}`;
        clusterLabel = node.category || 'Uncategorized';
        break;
      case 'type':
        clusterId = `cluster-${node.entry_type || 'Unknown'}`;
        clusterLabel = node.entry_type || 'Unknown';
        break;
      default:
        clusterId = 'cluster-all';
        clusterLabel = 'All';
    }

    const nodeColor = clusterBy === 'type'
      ? getEntryTypeColor(node.entry_type)
      : getCategoryColor(node.category, categories);

    return {
      ...node,
      source_domain: (node as any).source_domain,
      description: (node as any).description,
      tags: (node as any).tags,
      created_at: (node as any).created_at,
      clusterId,
      clusterLabel,
      isClusterNode: false,
      color: nodeColor,
      size: getNodeSize(connections, maxConnections),
      opacity: 1,
    };
  });

  // Transform edges
  const edges: GraphEdgeExtended[] = rawEdges.map(edge => ({
    ...edge,
    opacity: 0.15 + edge.weight * 0.1,
    isHighlighted: false,
  }));

  // Create clusters
  const clusters = createClusters(nodes, clusterBy);

  return {
    nodes,
    edges,
    clusters,
    categories,
    domains,
    maxConnections,
  };
}

export interface UseGraphDataOptions {
  clusterBy: ClusterByType;
}

export function useGraphData(options: UseGraphDataOptions) {
  const { clusterBy } = options;

  const {
    data: rawData,
    isLoading,
    error,
    refetch,
  } = useQuery({
    queryKey: ['graph'],
    queryFn: api.getEntryGraph,
    staleTime: 5 * 60 * 1000, // 5 minutes
  });

  const transformedData = useMemo(() => {
    if (!rawData) return null;
    return transformGraphData(
      rawData.nodes,
      rawData.edges,
      clusterBy
    );
  }, [rawData, clusterBy]);

  return {
    data: transformedData,
    rawData,
    isLoading,
    error,
    refetch,
    nodeCount: rawData?.nodes.length || 0,
    edgeCount: rawData?.edges.length || 0,
  };
}

// Find connected nodes using BFS
export function findConnectedNodes(
  startId: string,
  edges: GraphEdgeExtended[],
  maxDepth: number = 2
): Set<string> {
  const visited = new Set<string>([startId]);
  const queue: Array<{ id: string; depth: number }> = [{ id: startId, depth: 0 }];

  while (queue.length > 0) {
    const { id, depth } = queue.shift()!;
    if (depth >= maxDepth) continue;

    for (const edge of edges) {
      let neighbor: string | null = null;
      if (edge.source === id) neighbor = edge.target as string;
      else if (edge.target === id) neighbor = edge.source as string;

      if (neighbor && !visited.has(neighbor)) {
        visited.add(neighbor);
        queue.push({ id: neighbor, depth: depth + 1 });
      }
    }
  }

  return visited;
}

// Find edges in the path
export function findConnectedEdges(
  connectedNodes: Set<string>,
  edges: GraphEdgeExtended[]
): Set<string> {
  const connectedEdges = new Set<string>();

  edges.forEach((edge) => {
    const sourceId = typeof edge.source === 'string' ? edge.source : (edge.source as any).id;
    const targetId = typeof edge.target === 'string' ? edge.target : (edge.target as any).id;

    if (connectedNodes.has(sourceId) && connectedNodes.has(targetId)) {
      connectedEdges.add(`${sourceId}-${targetId}`);
    }
  });

  return connectedEdges;
}
