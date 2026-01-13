import { useState, useRef, useEffect } from 'react';
import {
  Search,
  LayoutGrid,
  Layers,
  Settings,
  X,
  ChevronDown,
} from 'lucide-react';
import clsx from 'clsx';
import type { GraphViewMode, GraphLayoutType, ClusterByType } from '../../types/graph';
import type { SearchResult } from '../../hooks/useGraphSearch';

interface GraphControlsProps {
  view: GraphViewMode;
  layout: GraphLayoutType;
  clusterBy: ClusterByType;
  showLabels: boolean;
  searchQuery: string;
  searchResults: SearchResult[];
  onViewChange: (view: GraphViewMode) => void;
  onLayoutChange: (layout: GraphLayoutType) => void;
  onClusterByChange: (clusterBy: ClusterByType) => void;
  onShowLabelsChange: (show: boolean) => void;
  onSearchChange: (query: string) => void;
  onSearchResultClick: (nodeId: string) => void;
  onClearSearch: () => void;
}

const LAYOUT_OPTIONS: Array<{ value: GraphLayoutType; label: string; icon: string }> = [
  { value: 'force', label: 'Force', icon: '⚡' },
  { value: 'hierarchical', label: 'Hierarchical', icon: '🌳' },
  { value: 'radial', label: 'Radial', icon: '🎯' },
  { value: 'grid', label: 'Grid', icon: '⊞' },
];

const CLUSTER_OPTIONS: Array<{ value: ClusterByType; label: string }> = [
  { value: 'none', label: 'No Clustering' },
  { value: 'domain', label: 'By Domain' },
  { value: 'category', label: 'By Category' },
  { value: 'type', label: 'By Type' },
];

export default function GraphControls({
  view,
  layout,
  clusterBy,
  showLabels,
  searchQuery,
  searchResults,
  onViewChange,
  onLayoutChange,
  onClusterByChange,
  onShowLabelsChange,
  onSearchChange,
  onSearchResultClick,
  onClearSearch,
}: GraphControlsProps) {
  const [isLayoutOpen, setIsLayoutOpen] = useState(false);
  const [isClusterOpen, setIsClusterOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [showSearchResults, setShowSearchResults] = useState(false);

  const searchRef = useRef<HTMLDivElement>(null);
  const layoutRef = useRef<HTMLDivElement>(null);
  const clusterRef = useRef<HTMLDivElement>(null);
  const settingsRef = useRef<HTMLDivElement>(null);

  // Close dropdowns on outside click
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (layoutRef.current && !layoutRef.current.contains(e.target as Node)) {
        setIsLayoutOpen(false);
      }
      if (clusterRef.current && !clusterRef.current.contains(e.target as Node)) {
        setIsClusterOpen(false);
      }
      if (settingsRef.current && !settingsRef.current.contains(e.target as Node)) {
        setIsSettingsOpen(false);
      }
      if (searchRef.current && !searchRef.current.contains(e.target as Node)) {
        setShowSearchResults(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  return (
    <div className="relative z-20 flex flex-wrap items-center gap-3 p-4 bg-slate-900/80 backdrop-blur-lg border-b border-slate-800">
      {/* Search */}
      <div ref={searchRef} className="relative flex-1 min-w-[200px] max-w-md">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-500" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => {
              onSearchChange(e.target.value);
              setShowSearchResults(true);
            }}
            onFocus={() => setShowSearchResults(true)}
            placeholder="Search nodes..."
            className="w-full pl-10 pr-10 py-2 bg-slate-800/80 border border-slate-700 rounded-lg
                       text-sm text-white placeholder:text-slate-500
                       focus:outline-none focus:border-cyan-500/50 focus:ring-1 focus:ring-cyan-500/20
                       transition-all"
          />
          {searchQuery && (
            <button
              onClick={() => {
                onClearSearch();
                setShowSearchResults(false);
              }}
              className="absolute right-3 top-1/2 -translate-y-1/2 p-0.5 text-slate-400 hover:text-white
                         rounded transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          )}
        </div>

        {/* Search Results Dropdown */}
        {showSearchResults && searchResults.length > 0 && (
          <div className="absolute top-full left-0 right-0 mt-2 py-2 bg-slate-900/95 backdrop-blur-lg
                          border border-slate-700 rounded-lg shadow-xl z-50 max-h-64 overflow-auto">
            {searchResults.map((result) => (
              <button
                key={result.node.id}
                onClick={() => {
                  onSearchResultClick(result.node.id);
                  setShowSearchResults(false);
                }}
                className="w-full px-4 py-2 text-left hover:bg-slate-800/80 transition-colors"
              >
                <div className="flex items-center gap-2">
                  <div
                    className="w-2 h-2 rounded-full"
                    style={{ backgroundColor: result.node.color }}
                  />
                  <span className="text-sm text-white font-medium truncate">
                    {result.node.label}
                  </span>
                  <span className="text-xs text-slate-500 ml-auto">
                    {Math.round(result.score * 100)}%
                  </span>
                </div>
                <div className="text-xs text-slate-400 mt-0.5 truncate pl-4">
                  {result.node.category}
                  {result.node.source_domain && ` • ${result.node.source_domain}`}
                </div>
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Divider */}
      <div className="h-6 w-px bg-slate-700 hidden sm:block" />

      {/* Layout Selector */}
      <div ref={layoutRef} className="relative">
        <button
          onClick={() => setIsLayoutOpen(!isLayoutOpen)}
          className={clsx(
            'flex items-center gap-2 px-3 py-2 rounded-lg text-sm transition-all',
            isLayoutOpen
              ? 'bg-cyan-500/20 text-cyan-400'
              : 'bg-slate-800/80 text-slate-300 hover:text-white hover:bg-slate-700/80'
          )}
        >
          <LayoutGrid className="w-4 h-4" />
          <span className="hidden sm:inline">
            {LAYOUT_OPTIONS.find((l) => l.value === layout)?.label}
          </span>
          <ChevronDown className={clsx('w-4 h-4 transition-transform', isLayoutOpen && 'rotate-180')} />
        </button>

        {isLayoutOpen && (
          <div className="absolute top-full left-0 mt-2 py-1 bg-slate-900/95 backdrop-blur-lg
                          border border-slate-700 rounded-lg shadow-xl z-50 min-w-[140px]">
            {LAYOUT_OPTIONS.map((option) => (
              <button
                key={option.value}
                onClick={() => {
                  onLayoutChange(option.value);
                  setIsLayoutOpen(false);
                }}
                className={clsx(
                  'w-full px-3 py-2 text-left text-sm flex items-center gap-2 transition-colors',
                  layout === option.value
                    ? 'bg-cyan-500/20 text-cyan-400'
                    : 'text-slate-300 hover:bg-slate-800/80 hover:text-white'
                )}
              >
                <span>{option.icon}</span>
                <span>{option.label}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Cluster By Selector */}
      <div ref={clusterRef} className="relative">
        <button
          onClick={() => setIsClusterOpen(!isClusterOpen)}
          className={clsx(
            'flex items-center gap-2 px-3 py-2 rounded-lg text-sm transition-all',
            clusterBy !== 'none'
              ? 'bg-cyan-500/20 text-cyan-400'
              : 'bg-slate-800/80 text-slate-300 hover:text-white hover:bg-slate-700/80'
          )}
        >
          <Layers className="w-4 h-4" />
          <span className="hidden sm:inline">
            {CLUSTER_OPTIONS.find((c) => c.value === clusterBy)?.label}
          </span>
          <ChevronDown className={clsx('w-4 h-4 transition-transform', isClusterOpen && 'rotate-180')} />
        </button>

        {isClusterOpen && (
          <div className="absolute top-full left-0 mt-2 py-1 bg-slate-900/95 backdrop-blur-lg
                          border border-slate-700 rounded-lg shadow-xl z-50 min-w-[160px]">
            {CLUSTER_OPTIONS.map((option) => (
              <button
                key={option.value}
                onClick={() => {
                  onClusterByChange(option.value);
                  setIsClusterOpen(false);
                }}
                className={clsx(
                  'w-full px-3 py-2 text-left text-sm transition-colors',
                  clusterBy === option.value
                    ? 'bg-cyan-500/20 text-cyan-400'
                    : 'text-slate-300 hover:bg-slate-800/80 hover:text-white'
                )}
              >
                {option.label}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* 2D/3D Toggle */}
      <div className="flex items-center bg-slate-800/80 rounded-lg p-1">
        <button
          onClick={() => onViewChange('2d')}
          className={clsx(
            'px-3 py-1.5 text-sm font-medium rounded-md transition-all',
            view === '2d'
              ? 'bg-cyan-500 text-white shadow-lg shadow-cyan-500/25'
              : 'text-slate-400 hover:text-white'
          )}
        >
          2D
        </button>
        <button
          onClick={() => onViewChange('3d')}
          className={clsx(
            'px-3 py-1.5 text-sm font-medium rounded-md transition-all',
            view === '3d'
              ? 'bg-cyan-500 text-white shadow-lg shadow-cyan-500/25'
              : 'text-slate-400 hover:text-white'
          )}
        >
          3D
        </button>
      </div>

      {/* Divider */}
      <div className="h-6 w-px bg-slate-700 hidden sm:block" />

      {/* Settings */}
      <div ref={settingsRef} className="relative">
        <button
          onClick={() => setIsSettingsOpen(!isSettingsOpen)}
          className={clsx(
            'p-2 rounded-lg transition-all',
            isSettingsOpen
              ? 'bg-cyan-500/20 text-cyan-400'
              : 'bg-slate-800/80 text-slate-400 hover:text-white hover:bg-slate-700/80'
          )}
        >
          <Settings className="w-4 h-4" />
        </button>

        {isSettingsOpen && (
          <div className="absolute top-full right-0 mt-2 p-4 bg-slate-900/95 backdrop-blur-lg
                          border border-slate-700 rounded-lg shadow-xl z-50 min-w-[200px]">
            <div className="space-y-4">
              <label className="flex items-center justify-between cursor-pointer">
                <span className="text-sm text-slate-300">Show Labels</span>
                <button
                  onClick={() => onShowLabelsChange(!showLabels)}
                  className={clsx(
                    'relative w-10 h-6 rounded-full transition-colors',
                    showLabels ? 'bg-cyan-500' : 'bg-slate-600'
                  )}
                >
                  <span
                    className={clsx(
                      'absolute top-1 w-4 h-4 bg-white rounded-full transition-transform',
                      showLabels ? 'left-5' : 'left-1'
                    )}
                  />
                </button>
              </label>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
