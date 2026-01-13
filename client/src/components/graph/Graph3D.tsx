import { useRef, useCallback, useEffect, useMemo } from 'react';
import ForceGraph3D, { ForceGraphMethods } from 'react-force-graph-3d';
import type { GraphNodeExtended, GraphEdgeExtended } from '../../types/graph';
import { GRAPH_COLORS, GRAPH_SIZES } from '../../types/graph';

interface Graph3DProps {
  nodes: GraphNodeExtended[];
  edges: GraphEdgeExtended[];
  highlightedNodes: Set<string>;
  highlightedEdges: Set<string>;
  selectedNodeId: string | null;
  hoveredNodeId: string | null;
  showLabels: boolean;
  onNodeClick: (node: GraphNodeExtended) => void;
  onNodeHover: (node: GraphNodeExtended | null) => void;
  onBackgroundClick: () => void;
  width: number;
  height: number;
}

export default function Graph3D({
  nodes,
  edges,
  highlightedNodes,
  highlightedEdges,
  selectedNodeId,
  hoveredNodeId,
  showLabels,
  onNodeClick,
  onNodeHover,
  onBackgroundClick,
  width,
  height,
}: Graph3DProps) {
  const graphRef = useRef<ForceGraphMethods>();

  // Graph data format
  const graphData = useMemo(() => ({
    nodes: nodes.map(n => ({
      ...n,
      fx: n.fx === null ? undefined : n.fx,
      fy: n.fy === null ? undefined : n.fy,
      fz: n.fz === null ? undefined : n.fz,
    })),
    links: edges.map(e => ({
      source: e.source,
      target: e.target,
      weight: e.weight,
      color: e.color,
      opacity: e.opacity,
    })),
  }), [nodes, edges]);

  // Node color based on state
  const nodeColor = useCallback((node: any) => {
    const isSelected = selectedNodeId === node.id;
    const isHovered = hoveredNodeId === node.id;
    const isHighlighted = highlightedNodes.has(node.id);

    if (isSelected) return '#ffffff';
    if (isHovered || isHighlighted) return node.color || GRAPH_COLORS.primary;

    // Dim non-highlighted nodes when there's a selection
    if (highlightedNodes.size > 0) {
      return `${node.color || GRAPH_COLORS.primary}44`;
    }

    return node.color || GRAPH_COLORS.primary;
  }, [selectedNodeId, hoveredNodeId, highlightedNodes]);

  // Node size
  const nodeVal = useCallback((node: any) => {
    const isSelected = selectedNodeId === node.id;
    const isHovered = hoveredNodeId === node.id;
    const isHighlighted = highlightedNodes.has(node.id);

    const base = node.size || GRAPH_SIZES.nodeDefaultRadius;
    if (isSelected || isHovered) return base * 2;
    if (isHighlighted) return base * 1.5;
    return base;
  }, [selectedNodeId, hoveredNodeId, highlightedNodes]);

  // Link color
  const linkColor = useCallback((link: any) => {
    const sourceId = typeof link.source === 'string' ? link.source : link.source?.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target?.id;
    const edgeKey = `${sourceId}-${targetId}`;
    const reverseKey = `${targetId}-${sourceId}`;

    const isHighlighted = highlightedEdges.has(edgeKey) || highlightedEdges.has(reverseKey);

    if (isHighlighted) {
      return GRAPH_COLORS.edgeHighlight;
    }

    if (highlightedNodes.size > 0) {
      return 'rgba(34, 211, 238, 0.03)';
    }

    const weight = link.weight || 1;
    const opacity = 0.08 + Math.min(weight * 0.06, 0.2);
    return `rgba(34, 211, 238, ${opacity})`;
  }, [highlightedEdges, highlightedNodes]);

  // Link width
  const linkWidth = useCallback((link: any) => {
    const weight = link.weight || 1;
    return 0.5 + weight * 0.3;
  }, []);

  // Node label
  const nodeLabel = useCallback((node: any) => {
    return `<div style="
      background: rgba(15, 23, 42, 0.95);
      border: 1px solid rgba(34, 211, 238, 0.3);
      border-radius: 8px;
      padding: 8px 12px;
      font-family: 'JetBrains Mono', monospace;
      font-size: 12px;
      color: #fff;
      max-width: 250px;
    ">
      <div style="font-weight: 600; margin-bottom: 4px;">${node.label}</div>
      <div style="color: #94a3b8; font-size: 11px;">${node.category}</div>
      ${node.source_domain ? `<div style="color: #64748b; font-size: 10px; margin-top: 4px;">${node.source_domain}</div>` : ''}
    </div>`;
  }, []);

  // Handle clicks
  const handleNodeClick = useCallback((node: any) => {
    onNodeClick(node as GraphNodeExtended);
  }, [onNodeClick]);

  const handleNodeHover = useCallback((node: any) => {
    onNodeHover(node as GraphNodeExtended | null);
  }, [onNodeHover]);

  const handleBackgroundClick = useCallback(() => {
    onBackgroundClick();
  }, [onBackgroundClick]);

  // Auto-rotate and initial zoom
  useEffect(() => {
    const timer = setTimeout(() => {
      graphRef.current?.zoomToFit(600, 80);
    }, 800);
    return () => clearTimeout(timer);
  }, []);

  return (
    <ForceGraph3D
      ref={graphRef}
      graphData={graphData}
      width={width}
      height={height}
      backgroundColor={GRAPH_COLORS.canvasBg3d}
      // Node appearance
      nodeColor={nodeColor}
      nodeVal={nodeVal}
      nodeOpacity={0.9}
      nodeResolution={16}
      // Node labels
      nodeLabel={showLabels ? nodeLabel : undefined}
      // Link appearance
      linkColor={linkColor}
      linkWidth={linkWidth}
      linkOpacity={0.6}
      linkCurvature={0.15}
      // Interactions
      onNodeClick={handleNodeClick}
      onNodeHover={handleNodeHover}
      onBackgroundClick={handleBackgroundClick}
      enableNodeDrag={true}
      enableNavigationControls={true}
      // Force simulation
      d3AlphaDecay={0.02}
      d3VelocityDecay={0.35}
      cooldownTicks={100}
      warmupTicks={50}
      // 3D specific
      showNavInfo={false}
    />
  );
}
