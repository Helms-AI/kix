import type { IntegrationType } from './index';

interface WizardNavigationProps {
  currentStep: number;
  totalSteps: number;
  canProceed: boolean;
  isSubmitting: boolean;
  integrationType: IntegrationType;
  onBack: () => void;
  onNext: () => void;
  onCancel: () => void;
  onSubmit: () => void;
}

export default function WizardNavigation({
  currentStep,
  totalSteps,
  canProceed,
  isSubmitting,
  onBack,
  onNext,
  onCancel,
  onSubmit,
}: WizardNavigationProps) {
  const isFirstStep = currentStep === 1;
  const isLastStep = currentStep === 5 || (totalSteps === 2 && currentStep === 5);
  const showBackButton = !isFirstStep;

  return (
    <div className="flex items-center justify-between">
      {/* Left side: Back and Cancel */}
      <div className="flex items-center gap-3">
        {showBackButton && (
          <button
            onClick={onBack}
            disabled={isSubmitting}
            className="
              px-4 py-2.5 flex items-center gap-2
              text-sm font-medium text-slate-400
              hover:text-white hover:bg-slate-800
              rounded-lg transition-colors
              disabled:opacity-50 disabled:cursor-not-allowed
            "
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
            Back
          </button>
        )}
        <button
          onClick={onCancel}
          disabled={isSubmitting}
          className="
            px-4 py-2.5
            text-sm font-medium text-slate-400
            hover:text-white hover:bg-slate-800
            rounded-lg transition-colors
            disabled:opacity-50 disabled:cursor-not-allowed
          "
        >
          Cancel
        </button>
      </div>

      {/* Right side: Next/Create */}
      <div>
        {isLastStep ? (
          <button
            onClick={onSubmit}
            disabled={!canProceed || isSubmitting}
            className="
              px-6 py-2.5 flex items-center gap-2
              text-sm font-semibold text-white
              bg-gradient-to-r from-cyan-600 to-cyan-500
              hover:from-cyan-500 hover:to-cyan-400
              rounded-lg shadow-lg shadow-cyan-500/25
              transition-all duration-200
              disabled:opacity-50 disabled:cursor-not-allowed disabled:shadow-none
            "
          >
            {isSubmitting ? (
              <>
                <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path
                    className="opacity-75"
                    fill="currentColor"
                    d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                  />
                </svg>
                Creating...
              </>
            ) : (
              <>
                Create Project
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                </svg>
              </>
            )}
          </button>
        ) : (
          <button
            onClick={onNext}
            disabled={!canProceed}
            className="
              px-5 py-2.5 flex items-center gap-2
              text-sm font-semibold text-white
              bg-slate-700 hover:bg-slate-600
              rounded-lg transition-colors
              disabled:opacity-50 disabled:cursor-not-allowed
            "
          >
            Next
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}
