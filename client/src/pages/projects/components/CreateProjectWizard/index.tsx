import { useState, useCallback, useEffect } from 'react';
import { projectApi } from '../../../../api/projectClient';
import type { CreateProjectRequest, Project } from '../../../../types/project';
import ProjectDetails from './steps/ProjectDetails';

export interface WizardState {
  projectName: string;
  projectDescription: string;
  projectColor: string;
}

interface CreateProjectWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess?: (projectId: string) => void;
}

const initialState: WizardState = {
  projectName: '',
  projectDescription: '',
  projectColor: '#3B82F6',
};

export default function CreateProjectWizard({ isOpen, onClose, onSuccess }: CreateProjectWizardProps) {
  const [state, setState] = useState<WizardState>(initialState);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);

  // Reset state when wizard closes
  useEffect(() => {
    if (!isOpen) {
      setState(initialState);
      setSubmitError(null);
    }
  }, [isOpen]);

  const updateState = useCallback((updates: Partial<WizardState>) => {
    setState(prev => ({ ...prev, ...updates }));
  }, []);

  const canSubmit = state.projectName.trim().length > 0;

  const handleSubmit = async () => {
    if (!canSubmit) return;

    setIsSubmitting(true);
    setSubmitError(null);

    try {
      const request: CreateProjectRequest = {
        name: state.projectName.trim(),
        description: state.projectDescription.trim() || undefined,
        color: state.projectColor,
      };

      const result = await projectApi.createProject(request) as Project;
      onSuccess?.(result.id);
      onClose();
    } catch (error) {
      setSubmitError(error instanceof Error ? error.message : 'Failed to create project');
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-lg mx-4 bg-slate-900 border border-slate-700/50 rounded-xl shadow-2xl">
        {/* Decorative top border */}
        <div className="absolute top-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-cyan-500/50 to-transparent rounded-t-xl" />

        {/* Header */}
        <div className="px-6 pt-6 pb-4 border-b border-slate-800">
          <div className="flex items-center justify-between">
            <h2 className="text-xl font-semibold text-white tracking-tight">
              Create New Project
            </h2>
            <button
              onClick={onClose}
              className="p-2 text-slate-400 hover:text-white hover:bg-slate-800 rounded-lg transition-colors"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="px-6 py-6">
          <ProjectDetails
            name={state.projectName}
            description={state.projectDescription}
            color={state.projectColor}
            isGitHub={false}
            repoName={null}
            onNameChange={(name) => updateState({ projectName: name })}
            onDescriptionChange={(desc) => updateState({ projectDescription: desc })}
            onColorChange={(color) => updateState({ projectColor: color })}
          />

          {submitError && (
            <div className="mt-4 p-3 bg-red-500/10 border border-red-500/30 rounded-lg text-red-400 text-sm">
              {submitError}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-6 py-4 border-t border-slate-800 bg-slate-900/50 flex justify-end gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-slate-300 hover:text-white hover:bg-slate-800 rounded-lg transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            disabled={!canSubmit || isSubmitting}
            className="px-4 py-2 text-sm font-medium text-white bg-cyan-600 hover:bg-cyan-500 disabled:opacity-50 disabled:cursor-not-allowed rounded-lg transition-colors flex items-center gap-2"
          >
            {isSubmitting ? (
              <>
                <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                </svg>
                Creating...
              </>
            ) : (
              'Create Project'
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
