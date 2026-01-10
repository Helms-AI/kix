import { useState, useEffect, useCallback } from 'react';
import { AlertTriangle, X, Trash2, CheckCircle2, XCircle } from 'lucide-react';
import clsx from 'clsx';

type ModalState = 'confirm' | 'loading' | 'success' | 'error';

interface DeleteAllDataModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => Promise<{ documents_deleted: number; chunks_deleted: number }>;
}

export function DeleteAllDataModal({ isOpen, onClose, onConfirm }: DeleteAllDataModalProps) {
  const [state, setState] = useState<ModalState>('confirm');
  const [safetyChecked, setSafetyChecked] = useState(false);
  const [deletedCounts, setDeletedCounts] = useState({ documents: 0, chunks: 0 });
  const [errorMessage, setErrorMessage] = useState('');

  // Reset state when modal opens
  useEffect(() => {
    if (isOpen) {
      setState('confirm');
      setSafetyChecked(false);
      setErrorMessage('');
    }
  }, [isOpen]);

  // Handle escape key
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && state === 'confirm') onClose();
    };
    if (isOpen) {
      document.addEventListener('keydown', handleEscape);
      return () => document.removeEventListener('keydown', handleEscape);
    }
  }, [isOpen, state, onClose]);

  // Prevent body scroll when modal is open
  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = 'hidden';
      return () => {
        document.body.style.overflow = '';
      };
    }
  }, [isOpen]);

  const handleDelete = useCallback(async () => {
    setState('loading');
    try {
      const result = await onConfirm();
      setDeletedCounts({ documents: result.documents_deleted, chunks: result.chunks_deleted });
      setState('success');
      setTimeout(() => onClose(), 2000);
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : 'An error occurred');
      setState('error');
    }
  }, [onConfirm, onClose]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      {/* Backdrop with danger tint */}
      <div
        className={clsx(
          'absolute inset-0 transition-colors duration-500',
          state === 'loading' ? 'bg-red-950/80' : 'bg-black/70',
          'backdrop-blur-sm'
        )}
        onClick={state === 'confirm' ? onClose : undefined}
      />

      {/* Modal */}
      <div
        className={clsx(
          'relative w-full max-w-md overflow-hidden rounded-2xl',
          'bg-gradient-to-b from-slate-900 via-slate-900 to-slate-950',
          'border border-slate-700/50 shadow-2xl shadow-black/50',
          'transform transition-all duration-300',
          isOpen ? 'scale-100 opacity-100' : 'scale-95 opacity-0'
        )}
      >
        {/* Warning stripes header */}
        <div className="h-2 bg-[repeating-linear-gradient(135deg,#f97316,#f97316_10px,#0f172a_10px,#0f172a_20px)]" />

        {/* Scanning line animation */}
        {state === 'loading' && (
          <div className="absolute inset-0 overflow-hidden pointer-events-none">
            <div className="absolute w-full h-1 bg-gradient-to-r from-transparent via-red-500/50 to-transparent animate-danger-scan" />
          </div>
        )}

        {/* Content */}
        <div className="p-6">
          {/* Close button */}
          {state === 'confirm' && (
            <button
              onClick={onClose}
              className="absolute top-4 right-4 p-1.5 text-slate-500 hover:text-slate-300 hover:bg-slate-800/50 rounded-lg transition-all"
            >
              <X className="w-5 h-5" />
            </button>
          )}

          {/* Confirm State */}
          {state === 'confirm' && (
            <>
              {/* Warning Icon */}
              <div className="flex justify-center mb-6">
                <div className="relative">
                  <div className="absolute inset-0 bg-orange-500/20 rounded-full blur-xl animate-pulse" />
                  <div className="relative w-16 h-16 rounded-full bg-gradient-to-br from-orange-500/20 to-red-500/20 border border-orange-500/30 flex items-center justify-center">
                    <AlertTriangle className="w-8 h-8 text-orange-400 animate-pulse" />
                  </div>
                </div>
              </div>

              {/* Title */}
              <h2 className="text-xl font-bold text-white text-center mb-2 tracking-tight">
                Delete All Indexed Data
              </h2>

              {/* Description */}
              <p className="text-slate-400 text-center text-sm mb-6 leading-relaxed">
                This will permanently delete all <span className="text-orange-400 font-medium">documents</span> and{' '}
                <span className="text-orange-400 font-medium">chunks</span> from the index.
                This action cannot be undone.
              </p>

              {/* Safety Switch */}
              <label className="group flex items-start gap-3 p-4 rounded-xl bg-slate-800/50 border border-slate-700/50 hover:border-orange-500/30 cursor-pointer transition-all mb-6">
                <div className="relative flex-shrink-0 mt-0.5">
                  <input
                    type="checkbox"
                    checked={safetyChecked}
                    onChange={(e) => setSafetyChecked(e.target.checked)}
                    className="sr-only"
                  />
                  <div
                    className={clsx(
                      'w-5 h-5 rounded border-2 transition-all duration-200',
                      safetyChecked
                        ? 'bg-orange-500 border-orange-500'
                        : 'bg-transparent border-slate-600 group-hover:border-orange-500/50'
                    )}
                  >
                    {safetyChecked && (
                      <svg
                        className="w-full h-full text-white"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="3"
                      >
                        <polyline points="20 6 9 17 4 12" />
                      </svg>
                    )}
                  </div>
                </div>
                <span className="text-sm text-slate-300 leading-relaxed">
                  I understand that this will <span className="text-red-400 font-semibold">permanently delete</span> all
                  indexed data and this action is irreversible.
                </span>
              </label>

              {/* Actions */}
              <div className="flex gap-3">
                <button
                  onClick={onClose}
                  className="flex-1 px-4 py-2.5 text-sm font-medium text-slate-300 bg-slate-800 hover:bg-slate-700 rounded-xl transition-colors"
                >
                  Cancel
                </button>
                <button
                  onClick={handleDelete}
                  disabled={!safetyChecked}
                  className={clsx(
                    'flex-1 flex items-center justify-center gap-2 px-4 py-2.5 text-sm font-semibold rounded-xl transition-all duration-200',
                    safetyChecked
                      ? 'bg-gradient-to-r from-red-600 to-red-500 hover:from-red-500 hover:to-red-400 text-white shadow-lg shadow-red-500/25'
                      : 'bg-slate-800 text-slate-500 cursor-not-allowed'
                  )}
                >
                  <Trash2 className="w-4 h-4" />
                  Delete All Data
                </button>
              </div>
            </>
          )}

          {/* Loading State */}
          {state === 'loading' && (
            <div className="py-8 text-center">
              <div className="relative w-16 h-16 mx-auto mb-6">
                <div className="absolute inset-0 rounded-full border-4 border-slate-700" />
                <div className="absolute inset-0 rounded-full border-4 border-red-500 border-t-transparent animate-spin" />
                <Trash2 className="absolute inset-0 m-auto w-6 h-6 text-red-400" />
              </div>
              <p className="text-lg font-medium text-white mb-1">Deleting Data...</p>
              <p className="text-sm text-slate-400">Please wait while we remove all indexed content.</p>
            </div>
          )}

          {/* Success State */}
          {state === 'success' && (
            <div className="py-8 text-center">
              <div className="w-16 h-16 mx-auto mb-6 rounded-full bg-emerald-500/20 border border-emerald-500/30 flex items-center justify-center">
                <CheckCircle2 className="w-8 h-8 text-emerald-400" />
              </div>
              <p className="text-lg font-medium text-white mb-2">Data Deleted Successfully</p>
              <p className="text-sm text-slate-400">
                Removed {deletedCounts.documents.toLocaleString()} documents and{' '}
                {deletedCounts.chunks.toLocaleString()} chunks.
              </p>
            </div>
          )}

          {/* Error State */}
          {state === 'error' && (
            <div className="py-8 text-center">
              <div className="w-16 h-16 mx-auto mb-6 rounded-full bg-red-500/20 border border-red-500/30 flex items-center justify-center">
                <XCircle className="w-8 h-8 text-red-400" />
              </div>
              <p className="text-lg font-medium text-white mb-2">Deletion Failed</p>
              <p className="text-sm text-red-400 mb-6">{errorMessage}</p>
              <button
                onClick={() => setState('confirm')}
                className="px-6 py-2 text-sm font-medium text-slate-300 bg-slate-800 hover:bg-slate-700 rounded-xl transition-colors"
              >
                Try Again
              </button>
            </div>
          )}
        </div>

        {/* Warning stripes footer */}
        <div className="h-2 bg-[repeating-linear-gradient(135deg,#f97316,#f97316_10px,#0f172a_10px,#0f172a_20px)]" />
      </div>
    </div>
  );
}
