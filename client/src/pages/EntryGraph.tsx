import { useEffect, useRef, useState, useCallback } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { ZoomIn, ZoomOut, Maximize2, Filter, Info } from 'lucide-react';
import clsx from 'clsx';
import { api } from '../api/client';
import type { GraphNode, GraphEdge } from '../types';

interface SimulationNode extends GraphNode {
  x: number;
  y: number;
  vx: number;
  vy: number;
  fx?: number | null;
  fy?: number | null;
}

const CATEGORY_COLORS: string[] = [
  '#06b6d4', '#10b981', '#8b5cf6', '#f59e0b', '#ef4444',
  '#ec4899', '#6366f1', '#14b8a6', '#84cc16', '#f97316',
];

function getCategoryColor(category: string, categories: string[]): string {
  const index = categories.indexOf(category);
  return CATEGORY_COLORS[index % CATEGORY_COLORS.length];
}

export default function EntryGraph() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const animationRef = useRef<number>();
  const nodesRef = useRef<SimulationNode[]>([]);
  const edgesRef = useRef<GraphEdge[]>([]);

  const [zoom, setZoom] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const [selectedNode, setSelectedNode] = useState<SimulationNode | null>(null);
  const [hoveredNode, setHoveredNode] = useState<SimulationNode | null>(null);
  const [filterType, setFilterType] = useState<string>('all');
  const [showLabels, setShowLabels] = useState(true);

  const navigate = useNavigate();

  const { data: graphData, isLoading } = useQuery({
    queryKey: ['graph'],
    queryFn: api.getEntryGraph,
  });

  // Get unique categories for coloring
  const categories = [...new Set(graphData?.nodes.map((n) => n.category) || [])];

  // Initialize simulation
  useEffect(() => {
    if (!graphData) return;

    const width = containerRef.current?.clientWidth || 800;
    const height = containerRef.current?.clientHeight || 600;

    // Initialize nodes with positions
    nodesRef.current = graphData.nodes.map((node) => ({
      ...node,
      x: width / 2 + (Math.random() - 0.5) * 400,
      y: height / 2 + (Math.random() - 0.5) * 400,
      vx: 0,
      vy: 0,
    }));
    edgesRef.current = graphData.edges;

    // Center the view
    setOffset({ x: width / 2, y: height / 2 });
  }, [graphData]);

  // Force simulation
  const simulate = useCallback(() => {
    const nodes = nodesRef.current;
    const edges = edgesRef.current;
    if (nodes.length === 0) return;

    const width = containerRef.current?.clientWidth || 800;
    const height = containerRef.current?.clientHeight || 600;

    // Apply forces
    for (let i = 0; i < nodes.length; i += 1) {
      const node = nodes[i];
      if (node.fx !== null && node.fx !== undefined) continue;

      // Center gravity
      node.vx += (width / 2 - node.x) * 0.001;
      node.vy += (height / 2 - node.y) * 0.001;

      // Repulsion between nodes
      for (let j = 0; j < nodes.length; j++) {
        if (i === j) continue;
        const other = nodes[j];
        const dx = node.x - other.x;
        const dy = node.y - other.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const force = 2000 / (dist * dist);
        node.vx += (dx / dist) * force;
        node.vy += (dy / dist) * force;
      }
    }

    // Edge attraction
    for (const edge of edges) {
      const source = nodes.find((n) => n.id === edge.source);
      const target = nodes.find((n) => n.id === edge.target);
      if (!source || !target) continue;

      const dx = target.x - source.x;
      const dy = target.y - source.y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;
      const force = (dist - 150) * 0.01 * edge.weight;

      if (source.fx === null || source.fx === undefined) {
        source.vx += (dx / dist) * force;
        source.vy += (dy / dist) * force;
      }
      if (target.fx === null || target.fx === undefined) {
        target.vx -= (dx / dist) * force;
        target.vy -= (dy / dist) * force;
      }
    }

    // Apply velocity with damping
    for (const node of nodes) {
      if (node.fx !== null && node.fx !== undefined) {
        node.x = node.fx;
        node.y = node.fy!;
        continue;
      }
      node.vx *= 0.9;
      node.vy *= 0.9;
      node.x += node.vx;
      node.y += node.vy;

      // Boundary constraints
      node.x = Math.max(50, Math.min(width - 50, node.x));
      node.y = Math.max(50, Math.min(height - 50, node.y));
    }
  }, []);

  // Render loop
  const render = useCallback(() => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx) return;

    const width = canvas.width;
    const height = canvas.height;

    // Clear
    ctx.fillStyle = '#0f172a';
    ctx.fillRect(0, 0, width, height);

    // Draw grid
    ctx.strokeStyle = '#1e293b';
    ctx.lineWidth = 1;
    const gridSize = 50 * zoom;
    const offsetX = offset.x % gridSize;
    const offsetY = offset.y % gridSize;

    for (let x = offsetX; x < width; x += gridSize) {
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
    }
    for (let y = offsetY; y < height; y += gridSize) {
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
      ctx.stroke();
    }

    ctx.save();
    ctx.translate(offset.x, offset.y);
    ctx.scale(zoom, zoom);
    ctx.translate(-offset.x, -offset.y);

    const nodes = nodesRef.current;
    const edges = edgesRef.current;

    // Filter nodes
    const visibleNodes = filterType === 'all'
      ? nodes
      : nodes.filter((n) => n.entry_type === filterType);
    const visibleIds = new Set(visibleNodes.map((n) => n.id));

    // Draw edges
    for (const edge of edges) {
      if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) continue;

      const source = nodes.find((n) => n.id === edge.source);
      const target = nodes.find((n) => n.id === edge.target);
      if (!source || !target) continue;

      ctx.beginPath();
      ctx.moveTo(source.x, source.y);
      ctx.lineTo(target.x, target.y);
      ctx.strokeStyle = `rgba(34, 211, 238, ${0.1 + edge.weight * 0.1})`;
      ctx.lineWidth = 1 + edge.weight * 0.5;
      ctx.stroke();
    }

    // Draw nodes
    for (const node of visibleNodes) {
      const isSelected = selectedNode?.id === node.id;
      const isHovered = hoveredNode?.id === node.id;
      const color = getCategoryColor(node.category, categories);
      const radius = isSelected || isHovered ? 12 : 8;

      // Glow effect
      if (isSelected || isHovered) {
        ctx.beginPath();
        ctx.arc(node.x, node.y, radius + 8, 0, Math.PI * 2);
        ctx.fillStyle = `${color}33`;
        ctx.fill();
      }

      // Node circle
      ctx.beginPath();
      ctx.arc(node.x, node.y, radius, 0, Math.PI * 2);
      ctx.fillStyle = color;
      ctx.fill();
      ctx.strokeStyle = isSelected ? '#ffffff' : `${color}aa`;
      ctx.lineWidth = 2;
      ctx.stroke();

      // Label
      if (showLabels || isSelected || isHovered) {
        ctx.font = `${isSelected || isHovered ? 'bold ' : ''}12px JetBrains Mono, monospace`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'top';
        ctx.fillStyle = isSelected || isHovered ? '#ffffff' : '#94a3b8';
        ctx.fillText(node.label, node.x, node.y + radius + 6);
      }
    }

    ctx.restore();

    // Continue simulation
    simulate();
    animationRef.current = requestAnimationFrame(render);
  }, [zoom, offset, selectedNode, hoveredNode, filterType, showLabels, simulate, categories]);

  // Start animation
  useEffect(() => {
    animationRef.current = requestAnimationFrame(render);
    return () => {
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current);
      }
    };
  }, [render]);

  // Resize handler
  useEffect(() => {
    const handleResize = () => {
      const canvas = canvasRef.current;
      const container = containerRef.current;
      if (canvas && container) {
        canvas.width = container.clientWidth;
        canvas.height = container.clientHeight;
      }
    };
    handleResize();
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  // Mouse handlers
  const getNodeAtPosition = (x: number, y: number): SimulationNode | null => {
    const transformedX = (x - offset.x) / zoom + offset.x;
    const transformedY = (y - offset.y) / zoom + offset.y;

    for (const node of nodesRef.current) {
      const dx = node.x - transformedX;
      const dy = node.y - transformedY;
      if (Math.sqrt(dx * dx + dy * dy) < 15) {
        return node;
      }
    }
    return null;
  };

  const handleMouseDown = (e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;

    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const node = getNodeAtPosition(x, y);

    if (node) {
      setSelectedNode(node);
      node.fx = node.x;
      node.fy = node.y;
    } else {
      setIsDragging(true);
      setDragStart({ x: e.clientX - offset.x, y: e.clientY - offset.y });
    }
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;

    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    if (selectedNode) {
      const transformedX = (x - offset.x) / zoom + offset.x;
      const transformedY = (y - offset.y) / zoom + offset.y;
      selectedNode.fx = transformedX;
      selectedNode.fy = transformedY;
    } else if (isDragging) {
      setOffset({
        x: e.clientX - dragStart.x,
        y: e.clientY - dragStart.y,
      });
    } else {
      const node = getNodeAtPosition(x, y);
      setHoveredNode(node);
      if (canvasRef.current) {
        canvasRef.current.style.cursor = node ? 'pointer' : 'grab';
      }
    }
  };

  const handleMouseUp = () => {
    if (selectedNode) {
      selectedNode.fx = null;
      selectedNode.fy = null;
    }
    setSelectedNode(null);
    setIsDragging(false);
  };

  const handleDoubleClick = (e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;

    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const node = getNodeAtPosition(x, y);

    if (node) {
      navigate(`/entries/${encodeURIComponent(node.id)}`);
    }
  };

  const handleWheel = (e: React.WheelEvent) => {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    setZoom((z) => Math.max(0.3, Math.min(3, z * delta)));
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-[60vh]">
        <div className="flex flex-col items-center gap-4">
          <div className="w-12 h-12 border-4 border-cyan-500 border-t-transparent rounded-full animate-spin" />
          <p className="text-slate-400 font-mono">Loading graph...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold text-white">Knowledge Graph</h1>
          <p className="text-slate-400 mt-1">
            {graphData?.nodes.length || 0} entries, {graphData?.edges.length || 0} connections
          </p>
        </div>

        <div className="flex items-center gap-3">
          {/* Filter */}
          <div className="flex items-center gap-2">
            <Filter className="w-4 h-4 text-slate-500" />
            <select
              value={filterType}
              onChange={(e) => setFilterType(e.target.value)}
              className="input text-sm py-1.5"
            >
              <option value="all">All Types</option>
              <option value="messaging">Messaging</option>
              <option value="conversation">Conversation</option>
            </select>
          </div>

          {/* Labels Toggle */}
          <button
            onClick={() => setShowLabels(!showLabels)}
            className={clsx(
              'px-3 py-1.5 text-sm rounded-lg transition-colors',
              showLabels
                ? 'bg-cyan-500/20 text-cyan-400'
                : 'bg-slate-800 text-slate-400 hover:text-white'
            )}
          >
            Labels
          </button>

          {/* Zoom Controls */}
          <div className="flex items-center bg-slate-800 rounded-lg">
            <button
              onClick={() => setZoom((z) => Math.min(3, z * 1.2))}
              className="p-2 text-slate-400 hover:text-white transition-colors"
            >
              <ZoomIn className="w-4 h-4" />
            </button>
            <span className="px-2 text-xs text-slate-500 font-mono">{Math.round(zoom * 100)}%</span>
            <button
              onClick={() => setZoom((z) => Math.max(0.3, z * 0.8))}
              className="p-2 text-slate-400 hover:text-white transition-colors"
            >
              <ZoomOut className="w-4 h-4" />
            </button>
            <button
              onClick={() => {
                setZoom(1);
                const width = containerRef.current?.clientWidth || 800;
                const height = containerRef.current?.clientHeight || 600;
                setOffset({ x: width / 2, y: height / 2 });
              }}
              className="p-2 text-slate-400 hover:text-white transition-colors border-l border-slate-700"
            >
              <Maximize2 className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      {/* Graph Canvas */}
      <div
        ref={containerRef}
        className="card relative overflow-hidden"
        style={{ height: 'calc(100vh - 240px)', minHeight: '500px' }}
      >
        <canvas
          ref={canvasRef}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseUp}
          onDoubleClick={handleDoubleClick}
          onWheel={handleWheel}
          className="w-full h-full"
        />

        {/* Legend */}
        <div className="absolute bottom-4 left-4 card p-4 bg-slate-900/95">
          <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-3">Categories</h4>
          <div className="space-y-2">
            {categories.slice(0, 8).map((cat) => (
              <div key={cat} className="flex items-center gap-2">
                <div
                  className="w-3 h-3 rounded-full"
                  style={{ backgroundColor: getCategoryColor(cat, categories) }}
                />
                <span className="text-xs text-slate-400">{cat}</span>
              </div>
            ))}
            {categories.length > 8 && (
              <span className="text-xs text-slate-500">+{categories.length - 8} more</span>
            )}
          </div>
        </div>

        {/* Hovered Node Info */}
        {hoveredNode && (
          <div className="absolute top-4 right-4 card p-4 bg-slate-900/95 max-w-xs">
            <div className="flex items-start gap-3">
              <Info className="w-5 h-5 text-cyan-400 flex-shrink-0 mt-0.5" />
              <div>
                <h4 className="font-semibold text-white">{hoveredNode.label}</h4>
                <p className="text-sm text-slate-400 mt-1">{hoveredNode.category}</p>
                <p className="text-xs text-slate-500 mt-2">Double-click to view details</p>
              </div>
            </div>
          </div>
        )}

        {/* Instructions */}
        <div className="absolute top-4 left-4 text-xs text-slate-500 space-y-1">
          <p>Drag to pan • Scroll to zoom • Double-click node to view</p>
        </div>
      </div>
    </div>
  );
}
