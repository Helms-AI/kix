import { Github, BookOpen, RefreshCw, Wifi, WifiOff, Loader2 } from 'lucide-react';
import { useSSEContext } from '../contexts/SSEContext';
import clsx from 'clsx';

/**
 * Connection quality indicator with animated glow.
 */
function ConnectionIndicator({
  isConnected,
  isConnecting,
  reconnectAttempts,
  maxReconnectAttempts,
}: {
  isConnected: boolean;
  isConnecting: boolean;
  reconnectAttempts: number;
  maxReconnectAttempts: number;
}) {
  // Determine connection quality for visual feedback
  const quality = isConnected
    ? 'good'
    : isConnecting
      ? 'connecting'
      : reconnectAttempts < maxReconnectAttempts / 2
        ? 'degraded'
        : 'poor';

  return (
    <div className="relative flex items-center justify-center w-2.5 h-2.5">
      {/* Animated pulse ring for connected state */}
      {quality === 'good' && (
        <div className="absolute inset-0 rounded-full bg-emerald-400/40 animate-ping" />
      )}

      {/* Core indicator dot */}
      <div
        className={clsx(
          'relative w-2 h-2 rounded-full transition-colors duration-300',
          quality === 'good' && 'bg-emerald-400 shadow-lg shadow-emerald-400/50',
          quality === 'connecting' && 'bg-amber-400 shadow-lg shadow-amber-400/50 animate-pulse',
          quality === 'degraded' && 'bg-amber-400 shadow-lg shadow-amber-400/50',
          quality === 'poor' && 'bg-red-400 shadow-lg shadow-red-400/50'
        )}
      />
    </div>
  );
}

/**
 * Application footer component with quick links and SSE connection status.
 * Fixed position at bottom, spans full width.
 */
export function Footer() {
  const {
    isConnected,
    isConnecting,
    reconnectAttempts,
    maxReconnectAttempts,
    reconnect,
  } = useSSEContext();

  return (
    <footer className="fixed bottom-0 left-0 right-0 z-40 h-12 bg-slate-900/95 backdrop-blur-xl border-t border-slate-800/80">
      {/* Subtle gradient accent line at bottom */}
      <div className="absolute bottom-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-slate-700/50 to-transparent" />

      <div className="h-full flex items-center justify-between px-4 lg:px-6">
        {/* Left: Quick Links */}
        <div className="flex items-center gap-1 sm:gap-4">
          <a
            href="https://github.com"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1.5 px-2 py-1 rounded-md text-xs text-slate-500 hover:text-slate-300 hover:bg-slate-800/50 transition-all duration-200"
          >
            <Github className="w-3.5 h-3.5" />
            <span className="hidden sm:inline font-medium">GitHub</span>
          </a>
          <a
            href="/config/mcp"
            className="flex items-center gap-1.5 px-2 py-1 rounded-md text-xs text-slate-500 hover:text-slate-300 hover:bg-slate-800/50 transition-all duration-200"
          >
            <BookOpen className="w-3.5 h-3.5" />
            <span className="hidden sm:inline font-medium">Docs</span>
          </a>
        </div>

        {/* Right: SSE Connection Status */}
        <div className="flex items-center gap-2">
          {/* Status Badge */}
          <div
            className={clsx(
              'flex items-center gap-2 px-3 py-1.5 rounded-full text-xs font-mono transition-all duration-300',
              isConnected
                ? 'bg-emerald-950/40 text-emerald-400 border border-emerald-700/40'
                : isConnecting
                  ? 'bg-amber-950/40 text-amber-400 border border-amber-700/40'
                  : 'bg-red-950/40 text-red-400 border border-red-700/40'
            )}
          >
            {/* Connection Quality Indicator */}
            <ConnectionIndicator
              isConnected={isConnected}
              isConnecting={isConnecting}
              reconnectAttempts={reconnectAttempts}
              maxReconnectAttempts={maxReconnectAttempts}
            />

            {/* Status Icon */}
            {isConnecting ? (
              <Loader2 className="w-3 h-3 animate-spin" />
            ) : isConnected ? (
              <Wifi className="w-3 h-3" />
            ) : (
              <WifiOff className="w-3 h-3" />
            )}

            {/* Status Text */}
            <span className="hidden sm:inline">
              {isConnecting ? 'Connecting' : isConnected ? 'Live' : 'Offline'}
            </span>

            {/* Reconnect Attempts Counter */}
            {!isConnected && !isConnecting && reconnectAttempts > 0 && (
              <span className="text-[10px] text-slate-500 tabular-nums">
                {reconnectAttempts}/{maxReconnectAttempts}
              </span>
            )}
          </div>

          {/* Reconnect Button */}
          {!isConnected && !isConnecting && (
            <button
              onClick={reconnect}
              className="p-1.5 text-slate-500 hover:text-cyan-400 hover:bg-slate-800/50 rounded-lg transition-all duration-200"
              title="Reconnect to server"
            >
              <RefreshCw className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </div>
    </footer>
  );
}
