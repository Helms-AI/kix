import { useState, useCallback, useEffect } from 'react';

const ZOOM_STORAGE_KEY = 'kix-zoom-level';
const DEFAULT_ZOOM = 100;
const MIN_ZOOM = 50;
const MAX_ZOOM = 150;
const ZOOM_STEP = 10;

export interface ZoomState {
  level: number;
  isDefault: boolean;
}

export interface ZoomActions {
  setLevel: (level: number) => void;
  increase: () => void;
  decrease: () => void;
  reset: () => void;
}

export function useZoom(): ZoomState & ZoomActions {
  // Initialize from localStorage
  const [level, setLevelState] = useState<number>(() => {
    if (typeof window === 'undefined') return DEFAULT_ZOOM;
    const stored = localStorage.getItem(ZOOM_STORAGE_KEY);
    if (stored) {
      const parsed = parseInt(stored, 10);
      if (!isNaN(parsed) && parsed >= MIN_ZOOM && parsed <= MAX_ZOOM) {
        return parsed;
      }
    }
    return DEFAULT_ZOOM;
  });

  // Apply zoom to root element
  useEffect(() => {
    const root = document.getElementById('root');
    if (!root) return;

    const zoomValue = level / 100;
    const isFirefox = navigator.userAgent.toLowerCase().includes('firefox');

    if (isFirefox) {
      // Firefox fallback: use transform scale
      root.style.transform = `scale(${zoomValue})`;
      root.style.transformOrigin = 'top left';
      document.body.style.width = `${100 / zoomValue}%`;
      document.body.style.height = `${100 / zoomValue}%`;
      root.style.zoom = '';
    } else {
      // Chrome, Safari, Edge: use native zoom
      root.style.zoom = String(zoomValue);
      root.style.transform = '';
      root.style.transformOrigin = '';
      document.body.style.width = '';
      document.body.style.height = '';
    }
  }, [level]);

  // Persist to localStorage
  useEffect(() => {
    localStorage.setItem(ZOOM_STORAGE_KEY, String(level));
  }, [level]);

  const setLevel = useCallback((newLevel: number) => {
    const clamped = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, newLevel));
    setLevelState(clamped);
  }, []);

  const increase = useCallback(() => {
    setLevelState(prev => Math.min(MAX_ZOOM, prev + ZOOM_STEP));
  }, []);

  const decrease = useCallback(() => {
    setLevelState(prev => Math.max(MIN_ZOOM, prev - ZOOM_STEP));
  }, []);

  const reset = useCallback(() => {
    setLevelState(DEFAULT_ZOOM);
  }, []);

  return {
    level,
    isDefault: level === DEFAULT_ZOOM,
    setLevel,
    increase,
    decrease,
    reset,
  };
}

export { MIN_ZOOM, MAX_ZOOM, DEFAULT_ZOOM, ZOOM_STEP };
