import { useRef, useCallback, useEffect, useMemo } from 'react';
import ForceGraph2D, { ForceGraphMethods } from 'react-force-graph-2d';
import type { GraphNodeExtended, GraphEdgeExtended } from '../../types/graph';
import { GRAPH_COLORS, GRAPH_FONTS, GRAPH_SIZES } from '../../types/graph';

interface Graph2DProps {
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

export default function Graph2D({
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
}: Graph2DProps) {
  const graphRef = useRef<ForceGraphMethods>();

  // Graph data format for react-force-graph
  const graphData = useMemo(() => ({
    nodes: nodes.map(n => ({
      ...n,
      fx: n.fx === null ? undefined : n.fx,
      fy: n.fy === null ? undefined : n.fy,
    })),
    links: edges.map(e => ({
      source: e.source,
      target: e.target,
      weight: e.weight,
      color: e.color,
      opacity: e.opacity,
    })),
  }), [nodes, edges]);

  // Custom node canvas rendering
  const nodeCanvasRenderer = useCallback((
    node: any,
    ctx: CanvasRenderingContext2D,
    globalScale: number
  ) => {
    const isSelected = selectedNodeId === node.id;
    const isHovered = hoveredNodeId === node.id;
    const isHighlighted = highlightedNodes.has(node.id);
    const isActive = isSelected || isHovered || isHighlighted;

    const baseSize = node.size || GRAPH_SIZES.nodeDefaultRadius;
    const size = isActive ? baseSize * 1.3 : baseSize;
    const color = node.color || GRAPH_COLORS.primary;

    const x = node.x || 0;
    const y = node.y || 0;

    // Outer glow for active nodes
    if (isActive) {
      const gradient = ctx.createRadialGradient(x, y, size, x, y, size * 2.5);
      gradient.addColorStop(0, `${color}40`);
      gradient.addColorStop(1, 'transparent');
      ctx.beginPath();
      ctx.arc(x, y, size * 2.5, 0, 2 * Math.PI);
      ctx.fillStyle = gradient;
      ctx.fill();
    }

    // Node circle
    ctx.beginPath();
    ctx.arc(x, y, size, 0, 2 * Math.PI);

    // Gradient fill
    const fillGradient = ctx.createRadialGradient(
      x - size * 0.3,
      y - size * 0.3,
      0,
      x,
      y,
      size
    );
    fillGradient.addColorStop(0, color);
    fillGradient.addColorStop(1, `${color}cc`);
    ctx.fillStyle = fillGradient;
    ctx.fill();

    // Border
    ctx.strokeStyle = isSelected ? '#fff' : isActive ? color : `${color}88`;
    ctx.lineWidth = isSelected ? 2.5 : isActive ? 2 : 1.5;
    ctx.stroke();

    // Label (show when zoomed in, hovered, or labels enabled)
    const shouldShowLabel = showLabels || isActive || globalScale > 1.2;
    if (shouldShowLabel) {
      const label = node.label;
      const fontSize = isActive
        ? GRAPH_SIZES.labelFontSizeSelected
        : GRAPH_SIZES.labelFontSize;
      const adjustedFontSize = Math.max(fontSize / globalScale, 8);

      ctx.font = `${isActive ? 'bold ' : ''}${adjustedFontSize}px ${GRAPH_FONTS.labels}`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'top';

      // Text shadow for readability
      ctx.fillStyle = 'rgba(0, 0, 0, 0.8)';
      ctx.fillText(label, x + 1, y + size + 5);

      // Main text
      ctx.fillStyle = isActive ? '#fff' : GRAPH_COLORS.labelDefault;
      ctx.fillText(label, x, y + size + 4);
    }
  }, [selectedNodeId, hoveredNodeId, highlightedNodes, showLabels]);

  // Custom link rendering
  const linkColor = useCallback((link: any) => {
    const sourceId = typeof link.source === 'string' ? link.source : link.source?.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target?.id;
    const edgeKey = `${sourceId}-${targetId}`;
    const reverseKey = `${targetId}-${sourceId}`;

    const isHighlighted = highlightedEdges.has(edgeKey) || highlightedEdges.has(reverseKey);

    if (isHighlighted) {
      return GRAPH_COLORS.edgeHighlight;
    }

    // Dim non-highlighted edges when there's a selection
    if (highlightedNodes.size > 0) {
      return 'rgba(34, 211, 238, 0.05)';
    }

    const weight = link.weight || 1;
    const opacity = 0.1 + Math.min(weight * 0.08, 0.3);
    return `rgba(34, 211, 238, ${opacity})`;
  }, [highlightedEdges, highlightedNodes]);

  // Link width based on weight
  const linkWidth = useCallback((link: any) => {
    const weight = link.weight || 1;
    return GRAPH_SIZES.edgeMinWidth + weight * 0.4;
  }, []);

  // Handle node click
  const handleNodeClick = useCallback((node: any) => {
    onNodeClick(node as GraphNodeExtended);
  }, [onNodeClick]);

  // Handle node hover
  const handleNodeHover = useCallback((node: any) => {
    onNodeHover(node as GraphNodeExtended | null);
  }, [onNodeHover]);

  // Handle background click
  const handleBackgroundClick = useCallback(() => {
    onBackgroundClick();
  }, [onBackgroundClick]);

  // Zoom to fit on initial load
  useEffect(() => {
    const timer = setTimeout(() => {
      graphRef.current?.zoomToFit(400, 50);
    }, 500);
    return () => clearTimeout(timer);
  }, []);

  return (
    <ForceGraph2D
      ref={graphRef}
      graphData={graphData}
      width={width}
      height={height}
      backgroundColor={GRAPH_COLORS.canvasBg}
      // Node appearance
      nodeCanvasObject={nodeCanvasRenderer}
      nodeCanvasObjectMode={() => 'replace'}
      nodeRelSize={GRAPH_SIZES.nodeDefaultRadius}
      // Link appearance
      linkColor={linkColor}
      linkWidth={linkWidth}
      linkCurvature={0.1}
      linkDirectionalParticles={0}
      // Interactions
      onNodeClick={handleNodeClick}
      onNodeHover={handleNodeHover}
      onBackgroundClick={handleBackgroundClick}
      enableNodeDrag={true}
      enableZoomInteraction={true}
      enablePanInteraction={true}
      // Force simulation
      d3AlphaDecay={0.02}
      d3VelocityDecay={0.3}
      cooldownTicks={100}
      warmupTicks={50}
      // Performance
      autoPauseRedraw={true}
    />
  );
}
