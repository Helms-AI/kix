import { useEffect, RefObject } from 'react';

interface KeyboardHandlers {
  onExecuteQuery: () => void;
  onFocusSearch: () => void;
  onFocusEditor: () => void;
  onToggleLeftSidebar: () => void;
  onToggleRightSidebar: () => void;
}

/**
 * Hook for managing keyboard shortcuts within the explorer
 * Scoped to the container element to avoid conflicts
 */
export function useExplorerKeyboard(
  containerRef: RefObject<HTMLDivElement>,
  handlers: KeyboardHandlers
) {
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const handleKeyboard = (e: KeyboardEvent) => {
      // Only handle if focus is within the explorer
      if (
        !container.contains(document.activeElement) &&
        document.activeElement !== document.body
      ) {
        return;
      }

      const isMod = e.metaKey || e.ctrlKey;

      // Execute query: Cmd/Ctrl + Enter
      if (isMod && e.key === 'Enter') {
        e.preventDefault();
        handlers.onExecuteQuery();
      }

      // Focus search: Cmd/Ctrl + K
      if (isMod && e.key === 'k') {
        e.preventDefault();
        handlers.onFocusSearch();
      }

      // Toggle left sidebar: Cmd/Ctrl + B
      if (isMod && e.key === 'b' && !e.shiftKey) {
        e.preventDefault();
        handlers.onToggleLeftSidebar();
      }

      // Toggle right sidebar: Cmd/Ctrl + Shift + B
      if (isMod && e.key === 'b' && e.shiftKey) {
        e.preventDefault();
        handlers.onToggleRightSidebar();
      }

      // Focus query editor: Cmd/Ctrl + E
      if (isMod && e.key === 'e') {
        e.preventDefault();
        handlers.onFocusEditor();
      }
    };

    window.addEventListener('keydown', handleKeyboard);
    return () => window.removeEventListener('keydown', handleKeyboard);
  }, [containerRef, handlers]);
}
