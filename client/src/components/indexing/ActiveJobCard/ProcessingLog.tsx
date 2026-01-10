import { useRef, useEffect, useState, useCallback } from 'react';
import { ArrowDown, Filter, Trash2 } from 'lucide-react';
import type { JobLogEntry } from '../../../api/indexingClient';
import { LogEntry } from './LogEntry';

interface ProcessingLogProps {
  entries: JobLogEntry[];
  maxHeight?: number;
  onClear?: () => void;
}

export function ProcessingLog({
  entries,
  maxHeight = 200,
  onClear,
}: ProcessingLogProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);
  const [showErrorsOnly, setShowErrorsOnly] = useState(false);
  const [isUserScrolling, setIsUserScrolling] = useState(false);
  const lastEntryCountRef = useRef(entries.length);

  // Filter entries
  const filteredEntries = showErrorsOnly
    ? entries.filter((e) => e.type === 'error')
    : entries;

  const errorCount = entries.filter((e) => e.type === 'error').length;

  // Handle auto-scroll
  useEffect(() => {
    if (autoScroll && containerRef.current && entries.length > lastEntryCountRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
    lastEntryCountRef.current = entries.length;
  }, [entries.length, autoScroll]);

  // Detect user scroll
  const handleScroll = useCallback(() => {
    if (!containerRef.current || isUserScrolling) return;

    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;

    if (!isAtBottom && autoScroll) {
      setAutoScroll(false);
    }
  }, [autoScroll, isUserScrolling]);

  // Track when user is actively scrolling
  const handleScrollStart = useCallback(() => {
    setIsUserScrolling(true);
    // Reset after a short delay
    setTimeout(() => setIsUserScrolling(false), 150);
  }, []);

  const scrollToBottom = useCallback(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
      setAutoScroll(true);
    }
  }, []);

  return (
    <div className="flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-slate-700/50">
        <span className="text-xs font-medium text-slate-400 uppercase tracking-wide">
          Live Log
        </span>

        <div className="flex items-center gap-2">
          {/* Filter toggle */}
          <button
            onClick={() => setShowErrorsOnly(!showErrorsOnly)}
            className={`
              flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors
              ${showErrorsOnly
                ? 'bg-red-900/30 text-red-400 border border-red-800/50'
                : 'text-slate-400 hover:text-slate-300 hover:bg-slate-800'
              }
            `}
            title={showErrorsOnly ? 'Show all entries' : 'Show errors only'}
          >
            <Filter className="w-3 h-3" />
            {showErrorsOnly ? `Errors (${errorCount})` : 'All'}
          </button>

          {/* Scroll to bottom button */}
          {!autoScroll && (
            <button
              onClick={scrollToBottom}
              className="flex items-center gap-1.5 px-2 py-1 rounded text-xs text-cyan-400 hover:bg-cyan-900/30 transition-colors"
              title="Scroll to bottom"
            >
              <ArrowDown className="w-3 h-3" />
              Resume
            </button>
          )}

          {/* Clear button */}
          {onClear && entries.length > 0 && (
            <button
              onClick={onClear}
              className="flex items-center gap-1.5 px-2 py-1 rounded text-xs text-slate-400 hover:text-slate-300 hover:bg-slate-800 transition-colors"
              title="Clear log"
            >
              <Trash2 className="w-3 h-3" />
            </button>
          )}
        </div>
      </div>

      {/* Log container */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        onMouseDown={handleScrollStart}
        onTouchStart={handleScrollStart}
        className="overflow-y-auto scrollbar-thin"
        style={{ maxHeight }}
      >
        {filteredEntries.length === 0 ? (
          <div className="flex items-center justify-center py-8 text-slate-500 text-sm">
            {showErrorsOnly ? 'No errors yet' : 'Waiting for log entries...'}
          </div>
        ) : (
          <div className="divide-y divide-slate-800/50">
            {filteredEntries.map((entry, index) => (
              <LogEntry
                key={entry.id}
                entry={entry}
                isNew={index === filteredEntries.length - 1}
              />
            ))}
          </div>
        )}
      </div>

      {/* Auto-scroll indicator */}
      {!autoScroll && entries.length > 0 && (
        <div className="flex items-center justify-center py-1.5 bg-slate-800/50 border-t border-slate-700/50">
          <span className="text-xs text-slate-500">
            Auto-scroll paused
          </span>
        </div>
      )}
    </div>
  );
}
