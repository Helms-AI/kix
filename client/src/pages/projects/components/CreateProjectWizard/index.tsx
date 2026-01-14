import { useState, useCallback, useEffect } from 'react';
import { projectApi, githubApi } from '../../../../api/projectClient';
import type { CreateProjectRequest, GitHubTokenStatus, Project } from '../../../../types/project';
import WizardProgress from './WizardProgress';
import WizardNavigation from './WizardNavigation';
import IntegrationChoice from './steps/IntegrationChoice';
import TokenConfiguration from './steps/TokenConfiguration';
import RepositorySelection from './steps/RepositorySelection';
import TemplateSelection from './steps/TemplateSelection';
import ProjectDetails from './steps/ProjectDetails';

export type IntegrationType = 'github' | 'local' | null;
export type TokenSource = 'global' | 'new' | null;
export type ProjectTemplate = 'kanban' | 'bug_tracking' | 'sprint_planning' | 'feature_roadmap' | null;

export interface WizardState {
  // Step 1: Integration choice
  integrationType: IntegrationType;

  // Step 2: Token configuration
  tokenSource: TokenSource;
  newToken: string;
  tokenError: string | null;

  // Step 3: Repository selection
  selectedOrg: string | null;
  selectedRepo: string | null;

  // Step 4: Template selection (GitHub only)
  selectedTemplate: ProjectTemplate;

  // Step 5: Project details
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
  integrationType: null,
  tokenSource: null,
  newToken: '',
  tokenError: null,
  selectedOrg: null,
  selectedRepo: null,
  selectedTemplate: null,
  projectName: '',
  projectDescription: '',
  projectColor: '#3B82F6',
};

export default function CreateProjectWizard({ isOpen, onClose, onSuccess }: CreateProjectWizardProps) {
  const [currentStep, setCurrentStep] = useState(1);
  const [state, setState] = useState<WizardState>(initialState);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [submitWarning, setSubmitWarning] = useState<string | null>(null);
  const [globalTokenStatus, setGlobalTokenStatus] = useState<GitHubTokenStatus | null>(null);
  const [isLoadingTokenStatus, setIsLoadingTokenStatus] = useState(false);

  // Fetch global token status when wizard opens
  useEffect(() => {
    if (isOpen) {
      setIsLoadingTokenStatus(true);
      githubApi.getTokenStatus()
        .then(setGlobalTokenStatus)
        .catch(() => setGlobalTokenStatus(null))
        .finally(() => setIsLoadingTokenStatus(false));
    }
  }, [isOpen]);

  // Reset state when wizard closes
  useEffect(() => {
    if (!isOpen) {
      setState(initialState);
      setCurrentStep(1);
      setSubmitError(null);
      setSubmitWarning(null);
    }
  }, [isOpen]);

  const updateState = useCallback((updates: Partial<WizardState>) => {
    setState(prev => ({ ...prev, ...updates }));
  }, []);

  // Calculate total steps based on integration type
  const getTotalSteps = () => {
    if (state.integrationType === 'local') return 2; // Step 1 + Step 5 (details)
    return 5; // All steps for GitHub: Integration, Token, Repo, Template, Details
  };

  // Get display step number (for local projects, final step shows as step 2)
  const getDisplayStep = () => {
    if (state.integrationType === 'local' && currentStep === 5) return 2;
    return currentStep;
  };

  const canProceed = () => {
    switch (currentStep) {
      case 1:
        return state.integrationType !== null;
      case 2:
        if (state.tokenSource === 'global') return true;
        if (state.tokenSource === 'new') return state.newToken.length > 0 && !state.tokenError;
        return false;
      case 3:
        return state.selectedOrg !== null && state.selectedRepo !== null;
      case 4:
        return state.selectedTemplate !== null;
      case 5:
        return state.projectName.trim().length > 0;
      default:
        return false;
    }
  };

  const handleNext = () => {
    if (currentStep === 1 && state.integrationType === 'local') {
      setCurrentStep(5); // Skip to project details for local
    } else if (currentStep < 5) {
      setCurrentStep(prev => prev + 1);
    }
  };

  const handleBack = () => {
    if (currentStep === 5 && state.integrationType === 'local') {
      setCurrentStep(1); // Go back to integration choice for local
    } else if (currentStep > 1) {
      setCurrentStep(prev => prev - 1);
    }
  };

  const handleSubmit = async () => {
    setIsSubmitting(true);
    setSubmitError(null);
    setSubmitWarning(null);

    try {
      const request: CreateProjectRequest = {
        name: state.projectName.trim(),
        description: state.projectDescription.trim() || undefined,
        color: state.projectColor,
      };

      if (state.integrationType === 'github') {
        request.github_owner = state.selectedOrg!;
        request.github_repo = state.selectedRepo!;
        request.template = state.selectedTemplate!;

        if (state.tokenSource === 'new' && state.newToken) {
          request.token = state.newToken;
        } else {
          request.use_global_token = true;
        }
      }

      // Response may include a warning even on success (e.g., Project V2 creation failed)
      const result = await projectApi.createProject(request) as Project & { warning?: string; github_project_url?: string };

      // Check for warnings (e.g., Project V2 creation failed)
      if (result.warning) {
        setSubmitWarning(result.warning);
        // Don't close - let user see the warning and decide
        setIsSubmitting(false);
        // Still call onSuccess so project shows up in list
        onSuccess?.(result.id);
        return;
      }

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
      <div className="relative w-full max-w-2xl mx-4 bg-slate-900 border border-slate-700/50 rounded-xl shadow-2xl">
        {/* Decorative top border */}
        <div className="absolute top-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-cyan-500/50 to-transparent rounded-t-xl" />

        {/* Header */}
        <div className="px-8 pt-8 pb-6 border-b border-slate-800">
          <div className="flex items-center justify-between mb-6">
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

          <WizardProgress
            currentStep={getDisplayStep()}
            totalSteps={getTotalSteps()}
            integrationType={state.integrationType}
          />
        </div>

        {/* Content */}
        <div className="px-8 py-6 min-h-[360px]">
          {currentStep === 1 && (
            <IntegrationChoice
              selected={state.integrationType}
              onSelect={(type) => updateState({ integrationType: type })}
            />
          )}

          {currentStep === 2 && (
            <TokenConfiguration
              tokenSource={state.tokenSource}
              newToken={state.newToken}
              tokenError={state.tokenError}
              globalTokenStatus={globalTokenStatus}
              isLoadingTokenStatus={isLoadingTokenStatus}
              onTokenSourceChange={(source) => updateState({ tokenSource: source, tokenError: null })}
              onTokenChange={(token) => updateState({ newToken: token })}
              onTokenError={(error) => updateState({ tokenError: error })}
            />
          )}

          {currentStep === 3 && (
            <RepositorySelection
              selectedOrg={state.selectedOrg}
              selectedRepo={state.selectedRepo}
              token={state.tokenSource === 'new' ? state.newToken : undefined}
              onOrgChange={(org) => updateState({ selectedOrg: org, selectedRepo: null })}
              onRepoChange={(repo, repoName) => {
                updateState({ selectedRepo: repo });
                // Auto-fill project name if empty
                if (!state.projectName && repoName) {
                  updateState({ projectName: repoName });
                }
              }}
            />
          )}

          {currentStep === 4 && (
            <TemplateSelection
              selected={state.selectedTemplate}
              onSelect={(template) => updateState({ selectedTemplate: template })}
            />
          )}

          {currentStep === 5 && (
            <ProjectDetails
              name={state.projectName}
              description={state.projectDescription}
              color={state.projectColor}
              isGitHub={state.integrationType === 'github'}
              repoName={state.selectedRepo}
              onNameChange={(name) => updateState({ projectName: name })}
              onDescriptionChange={(desc) => updateState({ projectDescription: desc })}
              onColorChange={(color) => updateState({ projectColor: color })}
            />
          )}

          {submitError && (
            <div className="mt-4 p-3 bg-red-500/10 border border-red-500/30 rounded-lg text-red-400 text-sm">
              {submitError}
            </div>
          )}

          {submitWarning && (
            <div className="mt-4 p-4 bg-amber-500/10 border border-amber-500/30 rounded-lg">
              <div className="flex items-start gap-3">
                <svg className="w-5 h-5 text-amber-400 flex-shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                </svg>
                <div className="flex-1">
                  <p className="text-amber-400 text-sm font-medium mb-1">Project created with warning</p>
                  <p className="text-amber-300/80 text-sm">{submitWarning}</p>
                  <p className="text-slate-400 text-xs mt-2">
                    Your GitHub token may need the <code className="px-1 py-0.5 bg-slate-800 rounded text-slate-300">project</code> scope to create GitHub Projects V2.
                  </p>
                </div>
              </div>
              <div className="mt-3 flex justify-end">
                <button
                  onClick={onClose}
                  className="px-4 py-2 text-sm font-medium text-white bg-amber-600 hover:bg-amber-500 rounded-lg transition-colors"
                >
                  Close
                </button>
              </div>
            </div>
          )}
        </div>

        {/* Footer - hide when showing warning since it has its own close button */}
        {!submitWarning && (
          <div className="px-8 py-5 border-t border-slate-800 bg-slate-900/50">
            <WizardNavigation
              currentStep={currentStep}
              totalSteps={getTotalSteps()}
              canProceed={canProceed()}
              isSubmitting={isSubmitting}
            integrationType={state.integrationType}
            onBack={handleBack}
            onNext={handleNext}
            onCancel={onClose}
            onSubmit={handleSubmit}
            />
          </div>
        )}
      </div>
    </div>
  );
}
