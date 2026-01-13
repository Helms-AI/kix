import { useState, useCallback, useMemo, useRef, useEffect } from 'react';
import Graph2D from './Graph2D';
import Graph3D from './Graph3D';
import GraphControls from './GraphControls';
import GraphLegend from './GraphLegend';
import NodeDetail from './NodeDetail';
import { useGraphData, findConnectedNodes, findConnectedEdges } from '../../hooks/useGraphData';
import { useGraphControls } from '../../hooks/useGraphControls';
import { useGraphSearch } from '../../hooks/useGraphSearch';
import type { GraphNodeExtended } from '../../types/graph';
import { GRAPH_COLORS } from '../../types/graph';

export default function KnowledgeGraph() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState({ width: 800, height: 600 });
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);

  // Graph controls (URL-synced)
  const {
    settings,
    setView,
    setLayout,
    setClusterBy,
    setShowLabels,
    selectNode,
    setSearchQuery,
  } = useGraphControls();

  // Graph data
  const { data, isLoading, nodeCount, edgeCount } = useGraphData({
    clusterBy: settings.clusterBy,
  });

  // Search functionality
  const {
    results: searchResults,
    matchingNodeIds,
    isSearching,
    clearSearch,
  } = useGraphSearch({
    nodes: data?.nodes || [],
    query: settings.searchQuery,
    onQueryChange: setSearchQuery,
  });

  // Calculate highlighted nodes and edges based on selection or search
  const { highlightedNodes, highlightedEdges } = useMemo(() => {
    if (!data) {
      return { highlightedNodes: new Set<string>(), highlightedEdges: new Set<string>() };
    }

    // If searching, highlight search results
    if (isSearching && matchingNodeIds.size > 0) {
      const connectedNodes = new Set<string>();
      matchingNodeIds.forEach(nodeId => {
        const connected = findConnectedNodes(nodeId, data.edges, settings.highlightDepth);
        connected.forEach(id => connectedNodes.add(id));
      });
      const connectedEdges = findConnectedEdges(connectedNodes, data.edges);
      return { highlightedNodes: connectedNodes, highlightedEdges: connectedEdges };
    }

    // If a node is selected, highlight its connections
    if (settings.selectedNodeId) {
      const connectedNodes = findConnectedNodes(
        settings.selectedNodeId,
        data.edges,
        settings.highlightDepth
      );
      const connectedEdges = findConnectedEdges(connectedNodes, data.edges);
      return { highlightedNodes: connectedNodes, highlightedEdges: connectedEdges };
    }

    return { highlightedNodes: new Set<string>(), highlightedEdges: new Set<string>() };
  }, [data, settings.selectedNodeId, settings.highlightDepth, isSearching, matchingNodeIds]);

  // Get selected node data
  const selectedNode = useMemo(() => {
    if (!settings.selectedNodeId || !data) return null;
    return data.nodes.find(n => n.id === settings.selectedNodeId) || null;
  }, [settings.selectedNodeId, data]);

  // Get related count for selected node
  const relatedCount = useMemo(() => {
    if (!selectedNode || !data) return 0;
    return data.edges.filter(
      e => e.source === selectedNode.id || e.target === selectedNode.id
    ).length;
  }, [selectedNode, data]);

  // Legend items
  const legendItems = useMemo(() => {
    if (!data) return [];

    const counts = new Map<string, { count: number; color: string }>();
    data.nodes.forEach(node => {
      const key = settings.clusterBy === 'domain'
        ? node.source_domain || 'Unknown'
        : settings.clusterBy === 'type'
        ? node.entry_type
        : node.category;

      if (!counts.has(key)) {
        counts.set(key, {
          count: 0,
          color: node.color || GRAPH_COLORS.primary,
        });
      }
      counts.get(key)!.count++;
    });

    return Array.from(counts.entries())
      .map(([label, { count, color }]) => ({ label, count, color }))
      .sort((a, b) => b.count - a.count);
  }, [data, settings.clusterBy]);

  // Handle node click
  const handleNodeClick = useCallback((node: GraphNodeExtended) => {
    selectNode(settings.selectedNodeId === node.id ? null : node.id);
  }, [selectNode, settings.selectedNodeId]);

  // Handle node hover
  const handleNodeHover = useCallback((node: GraphNodeExtended | null) => {
    setHoveredNodeId(node?.id || null);
  }, []);

  // Handle background click
  const handleBackgroundClick = useCallback(() => {
    selectNode(null);
  }, [selectNode]);

  // Handle search result click
  const handleSearchResultClick = useCallback((nodeId: string) => {
    selectNode(nodeId);
    setSearchQuery('');
  }, [selectNode, setSearchQuery]);

  // Resize observer
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        setDimensions({ width, height });
      }
    });

    resizeObserver.observe(container);
    return () => resizeObserver.disconnect();
  }, []);

  // Loading state
  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full min-h-[400px]">
        <div className="flex flex-col items-center gap-4">
          <div className="relative">
            <div className="w-16 h-16 border-4 border-cyan-500/30 rounded-full" />
            <div className="absolute inset-0 w-16 h-16 border-4 border-cyan-500 border-t-transparent rounded-full animate-spin" />
          </div>
          <p className="text-slate-400 font-mono text-sm">Loading knowledge graph...</p>
        </div>
      </div>
    );
  }

  // No data state
  if (!data || data.nodes.length === 0) {
    return (
      <div className="flex items-center justify-center h-full min-h-[400px]">
        <div className="text-center">
          <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-slate-800 flex items-center justify-center">
            <svg className="w-8 h-8 text-slate-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5}
                d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
          </div>
          <h3 className="text-lg font-medium text-slate-300 mb-2">No data available</h3>
          <p className="text-sm text-slate-500">Index some documents to see the knowledge graph</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header Stats */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-slate-800 bg-slate-900/50">
        <h1 className="text-xl font-bold text-white">Knowledge Graph</h1>
        <div className="flex items-center gap-4 text-sm">
          <span className="text-slate-400">
            <span className="text-cyan-400 font-mono">{nodeCount}</span> nodes
          </span>
          <span className="text-slate-400">
            <span className="text-cyan-400 font-mono">{edgeCount}</span> connections
          </span>
        </div>
      </div>

      {/* Controls */}
      <GraphControls
        view={settings.view}
        layout={settings.layout}
        clusterBy={settings.clusterBy}
        showLabels={settings.showLabels}
        searchQuery={settings.searchQuery}
        searchResults={searchResults}
        onViewChange={setView}
        onLayoutChange={setLayout}
        onClusterByChange={setClusterBy}
        onShowLabelsChange={setShowLabels}
        onSearchChange={setSearchQuery}
        onSearchResultClick={handleSearchResultClick}
        onClearSearch={clearSearch}
      />

      {/* Graph Container */}
      <div
        ref={containerRef}
        className="relative flex-1 overflow-hidden"
        style={{ minHeight: '400px' }}
      >
        {/* 2D or 3D Graph */}
        {settings.view === '2d' ? (
          <Graph2D
            nodes={data.nodes}
            edges={data.edges}
            highlightedNodes={highlightedNodes}
            highlightedEdges={highlightedEdges}
            selectedNodeId={settings.selectedNodeId}
            hoveredNodeId={hoveredNodeId}
            showLabels={settings.showLabels}
            onNodeClick={handleNodeClick}
            onNodeHover={handleNodeHover}
            onBackgroundClick={handleBackgroundClick}
            width={dimensions.width}
            height={dimensions.height}
          />
        ) : (
          <Graph3D
            nodes={data.nodes}
            edges={data.edges}
            highlightedNodes={highlightedNodes}
            highlightedEdges={highlightedEdges}
            selectedNodeId={settings.selectedNodeId}
            hoveredNodeId={hoveredNodeId}
            showLabels={settings.showLabels}
            onNodeClick={handleNodeClick}
            onNodeHover={handleNodeHover}
            onBackgroundClick={handleBackgroundClick}
            width={dimensions.width}
            height={dimensions.height}
          />
        )}

        {/* Legend */}
        <GraphLegend
          items={legendItems}
          clusterBy={settings.clusterBy}
          show={settings.showLegend}
        />

        {/* Node Detail Panel */}
        <NodeDetail
          node={selectedNode}
          relatedCount={relatedCount}
          onClose={() => selectNode(null)}
        />

        {/* Instructions overlay */}
        {!selectedNode && !isSearching && (
          <div className="absolute bottom-4 right-4 text-xs text-slate-500 bg-slate-900/80 backdrop-blur-sm
                          px-3 py-2 rounded-lg border border-slate-800">
            <span className="hidden sm:inline">Click node to select • </span>
            <span>Double-click to view • </span>
            <span className="hidden sm:inline">Drag to pan • </span>
            <span>Scroll to zoom</span>
          </div>
        )}
      </div>
    </div>
  );
}
