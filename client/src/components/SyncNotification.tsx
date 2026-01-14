import React, { useState, useEffect } from 'react';
import {
  CheckCircle,
  AlertCircle,
  XCircle,
  ChevronDown,
  ChevronUp,
  X,
  ArrowDownToLine,
  ArrowUpFromLine,
  GitMerge,
  Sparkles,
} from 'lucide-react';
import clsx from 'clsx';
import type { GitHubSyncResult } from '../types/project';

interface SyncNotificationProps {
  result: GitHubSyncResult | null;
  onClose: () => void;
  autoHideDelay?: number;
}

export const SyncNotification: React.FC<SyncNotificationProps> = ({
  result,
  onClose,
  autoHideDelay = 10000,
}) => {
  const hasErrors = result ? result.errors.length > 0 : false;
  const [isExpanded, setIsExpanded] = useState(hasErrors);
  const [isPinned, setIsPinned] = useState(false);
  const [showPulse, setShowPulse] = useState(true);

  useEffect(() => {
    if (!result || isPinned) return;

    const timer = setTimeout(() => {
      onClose();
    }, autoHideDelay);

    return () => clearTimeout(timer);
  }, [result, isPinned, autoHideDelay, onClose]);

  useEffect(() => {
    if (result) {
      const pulseTimer = setTimeout(() => setShowPulse(false), 1500);
      return () => clearTimeout(pulseTimer);
    }
  }, [result]);

  if (!result) return null;

  const totalChanges = result.pulled + result.pushed + result.merged;
  const isSuccess = result.success && !hasErrors;
  const isWarning = result.success && hasErrors;

  const getStatusColor = () => {
    if (!result.success) return 'border-red-500/50 bg-red-500/10';
    if (hasErrors) return 'border-yellow-500/50 bg-yellow-500/10';
    return 'border-emerald-500/50 bg-emerald-500/10';
  };

  const getStatusIcon = () => {
    if (!result.success) return <XCircle className="w-5 h-5 text-red-400" />;
    if (hasErrors) return <AlertCircle className="w-5 h-5 text-yellow-400" />;
    return <CheckCircle className="w-5 h-5 text-emerald-400" />;
  };

  const getActionIcon = (action: string) => {
    switch (action) {
      case 'pulled':
        return <ArrowDownToLine className="w-4 h-4 text-blue-400" />;
      case 'pushed':
        return <ArrowUpFromLine className="w-4 h-4 text-purple-400" />;
      case 'merged':
        return <GitMerge className="w-4 h-4 text-emerald-400" />;
      default:
        return <Sparkles className="w-4 h-4 text-slate-400" />;
    }
  };

  const getActionLabel = (action: string) => {
    switch (action) {
      case 'pulled':
        return 'Pulled from GitHub';
      case 'pushed':
        return 'Pushed to GitHub';
      case 'merged':
        return 'Auto-merged';
      default:
        return action;
    }
  };

  return (
    <div
      className={clsx(
        'fixed bottom-4 right-4 z-50 w-96 max-w-[calc(100vw-2rem)]',
        'rounded-lg border shadow-xl backdrop-blur-sm',
        'transform transition-all duration-300 ease-out',
        'animate-in slide-in-from-bottom-4 fade-in',
        showPulse && 'notification-pulse',
        getStatusColor()
      )}
      onClick={() => setIsPinned(true)}
    >
      {/* Header */}
      <div className="flex items-start gap-3 p-4 pb-3">
        {getStatusIcon()}

        <div className="flex-1 min-w-0">
          <div className="flex items-center justify-between mb-1">
            <h4 className="font-semibold text-slate-100">
              {isSuccess ? 'Sync Complete' : isWarning ? 'Sync Completed with Warnings' : 'Sync Failed'}
            </h4>
            <button
              onClick={(e) => {
                e.stopPropagation();
                onClose();
              }}
              className="p-1 rounded hover:bg-slate-700/50 transition-colors"
              aria-label="Close notification"
            >
              <X className="w-4 h-4 text-slate-400 hover:text-slate-200" />
            </button>
          </div>

          {/* Summary Stats */}
          {totalChanges > 0 && (
            <div className="flex items-center gap-3 text-xs">
              {result.pulled > 0 && (
                <div className="flex items-center gap-1.5 text-blue-400 font-medium">
                  <ArrowDownToLine className="w-3.5 h-3.5" />
                  <span>{result.pulled} pulled</span>
                </div>
              )}
              {result.pushed > 0 && (
                <div className="flex items-center gap-1.5 text-purple-400 font-medium">
                  <ArrowUpFromLine className="w-3.5 h-3.5" />
                  <span>{result.pushed} pushed</span>
                </div>
              )}
              {result.merged > 0 && (
                <div className="flex items-center gap-1.5 text-emerald-400 font-medium">
                  <GitMerge className="w-3.5 h-3.5" />
                  <span>{result.merged} merged</span>
                </div>
              )}
            </div>
          )}

          {/* Conflicts Resolved Badge */}
          {result.conflicts_resolved > 0 && (
            <div className="mt-2.5 inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-500/20 border border-emerald-500/30 text-emerald-400 text-xs font-medium">
              <Sparkles className="w-3 h-3" />
              <span>{result.conflicts_resolved} conflicts auto-resolved</span>
            </div>
          )}

          {/* No changes message */}
          {totalChanges === 0 && result.errors.length === 0 && (
            <p className="text-xs text-slate-400 mt-0.5">Everything is up to date</p>
          )}
        </div>
      </div>

      {/* Expandable Details */}
      {(result.change_details.length > 0 || result.errors.length > 0) && (
        <>
          <button
            onClick={(e) => {
              e.stopPropagation();
              setIsExpanded(!isExpanded);
              setIsPinned(true);
            }}
            className="w-full px-4 py-2.5 flex items-center justify-center gap-1.5 text-xs font-medium text-slate-400 hover:text-slate-300 hover:bg-slate-800/50 border-t border-slate-700/50 transition-colors"
            aria-expanded={isExpanded}
            aria-label={isExpanded ? 'Hide details' : `Show ${totalChanges} changes`}
          >
            {isExpanded ? (
              <>
                <ChevronUp className="w-4 h-4" />
                Hide details
              </>
            ) : (
              <>
                <ChevronDown className="w-4 h-4" />
                Show {totalChanges > 0 ? `${totalChanges} changes` : 'details'}
                {result.errors.length > 0 && (
                  <span className="ml-1 px-1.5 py-0.5 rounded-full bg-red-500/20 text-red-400 text-xs">
                    {result.errors.length}
                  </span>
                )}
              </>
            )}
          </button>

          {isExpanded && (
            <div className="px-4 pb-4 space-y-3 max-h-80 overflow-y-auto scrollbar-thin">
              {/* Errors - Show first for better visibility */}
              {result.errors.length > 0 && (
                <div className="space-y-2">
                  <div className="flex items-center gap-1.5 text-xs font-semibold text-red-400">
                    <XCircle className="w-3.5 h-3.5" />
                    Errors
                  </div>
                  {result.errors.map((error, index) => (
                    <div
                      key={index}
                      className="text-xs text-red-300 p-2.5 rounded-lg bg-red-500/10 border border-red-500/20"
                    >
                      {error}
                    </div>
                  ))}
                </div>
              )}

              {/* Change Details */}
              {result.change_details.length > 0 && (
                <div className="space-y-2">
                  {result.errors.length > 0 && (
                    <div className="flex items-center gap-1.5 text-xs font-semibold text-slate-300 mt-1">
                      <CheckCircle className="w-3.5 h-3.5" />
                      Changes
                    </div>
                  )}
                  {result.change_details.slice(0, 10).map((change, index) => (
                    <div
                      key={index}
                      className="flex items-start gap-2.5 text-xs p-2.5 rounded-lg bg-slate-800/50 hover:bg-slate-800/70 transition-colors"
                    >
                      {getActionIcon(change.action)}
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 mb-1">
                          <span className="text-slate-200 font-semibold">#{change.issue_number}</span>
                          <span className="text-slate-400 truncate">{change.title}</span>
                        </div>
                        <div className="text-slate-500 text-xs">
                          {getActionLabel(change.action)}
                          {change.fields_affected.length > 0 && (
                            <span className="text-slate-600">
                              {' • '}{change.fields_affected.join(', ')}
                            </span>
                          )}
                        </div>
                      </div>
                    </div>
                  ))}
                  {result.change_details.length > 10 && (
                    <div className="text-xs text-slate-500 text-center py-1">
                      +{result.change_details.length - 10} more changes
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </>
      )}

      {/* Progress indicator for auto-hide */}
      {!isPinned && (
        <div className="h-0.5 bg-slate-700/50 overflow-hidden rounded-b-lg">
          <div
            className="h-full bg-slate-500/50 animate-shrink-width"
            style={{
              animationDuration: `${autoHideDelay}ms`,
            }}
          />
        </div>
      )}
    </div>
  );
};

export default SyncNotification;