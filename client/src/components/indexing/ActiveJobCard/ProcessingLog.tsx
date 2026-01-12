import { useRef, useEffect, useState, useCallback, memo } from 'react';
import { ArrowDown, Filter, Trash2 } from 'lucide-react';
import type { JobLogEntry } from '../../../api/indexingClient';
import { LogEntry } from './LogEntry';

interface ProcessingLogProps {
  entries: JobLogEntry[];
  maxHeight?: number;
  onClear?: () => void;
}

export const ProcessingLog = memo(function ProcessingLog({
  entries,
  maxHeight = 200,
  onClear,
}: ProcessingLogProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);
  const [showErrorsOnly, setShowErrorsOnly] = useState(false);
  const prevEntryCountRef = useRef(entries.length);

  // Filter entries
  const filteredEntries = showErrorsOnly
    ? entries.filter((e) => e.type === 'error')
    : entries;

  const errorCount = entries.filter((e) => e.type === 'error').length;

  // Auto-scroll when new entries arrive
  useEffect(() => {
    if (autoScroll && containerRef.current && entries.length > prevEntryCountRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
    prevEntryCountRef.current = entries.length;
  }, [entries.length, autoScroll]);

  // Detect user scroll to pause auto-scroll
  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;

    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    const isAtBottom = scrollHeight - scrollTop - clientHeight < 30;

    // Only update if state actually changes
    if (isAtBottom && !autoScroll) {
      setAutoScroll(true);
    } else if (!isAtBottom && autoScroll) {
      setAutoScroll(false);
    }
  }, [autoScroll]);

  const scrollToBottom = useCallback(() => {
    if (containerRef.current) {
      containerRef.current.scrollTo({
        top: containerRef.current.scrollHeight,
        behavior: 'smooth',
      });
      setAutoScroll(true);
    }
  }, []);

  const toggleFilter = useCallback(() => {
    setShowErrorsOnly((prev) => !prev);
  }, []);

  return (
    <div className="flex flex-col rounded-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2.5 bg-slate-800/60 border-b border-slate-700/50">
        <span className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
          Live Log
        </span>

        <div className="flex items-center gap-1.5">
          {/* Filter toggle */}
          <button
            onClick={toggleFilter}
            className={`
              flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium
              transition-all duration-200
              ${showErrorsOnly
                ? 'bg-red-900/40 text-red-400 border border-red-800/60 shadow-inner'
                : 'text-slate-400 hover:text-slate-300 hover:bg-slate-700/60'
              }
            `}
            title={showErrorsOnly ? 'Show all entries' : 'Show errors only'}
          >
            <Filter className="w-3 h-3" />
            {showErrorsOnly ? `Errors (${errorCount})` : 'All'}
          </button>

          {/* Clear button */}
          {onClear && entries.length > 0 && (
            <button
              onClick={onClear}
              className="p-1.5 rounded-md text-slate-500 hover:text-slate-300 hover:bg-slate-700/60 transition-colors"
              title="Clear log"
            >
              <Trash2 className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </div>

      {/* Log container */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="overflow-y-auto bg-slate-900/40 scrollbar-thin scrollbar-track-slate-800 scrollbar-thumb-slate-700"
        style={{ maxHeight }}
      >
        {filteredEntries.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-10 text-slate-500">
            <div className="w-8 h-8 mb-2 rounded-full bg-slate-800/80 flex items-center justify-center">
              <div className="w-2 h-2 rounded-full bg-slate-600 animate-pulse" />
            </div>
            <span className="text-sm">
              {showErrorsOnly ? 'No errors yet' : 'Waiting for log entries...'}
            </span>
          </div>
        ) : (
          <div className="divide-y divide-slate-800/60">
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

      {/* Auto-scroll paused indicator */}
      {!autoScroll && entries.length > 0 && (
        <div className="flex items-center justify-center gap-2 py-2 bg-slate-800/50 border-t border-slate-700/50">
          <span className="text-xs text-slate-500">Auto-scroll paused</span>
          <button
            onClick={scrollToBottom}
            className="flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium text-cyan-400 hover:text-cyan-300 hover:bg-cyan-900/30 transition-colors"
          >
            <ArrowDown className="w-3 h-3" />
            Resume
          </button>
        </div>
      )}
    </div>
  );
});
