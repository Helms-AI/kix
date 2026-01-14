import type { IntegrationType } from './index';

interface WizardProgressProps {
  currentStep: number;
  totalSteps: number;
  integrationType: IntegrationType;
}

const getStepLabels = (integrationType: IntegrationType) => {
  if (integrationType === 'local') {
    return ['Integration', 'Details'];
  }
  return ['Integration', 'Token', 'Repository', 'Template', 'Details'];
};

export default function WizardProgress({ currentStep, totalSteps, integrationType }: WizardProgressProps) {
  const labels = getStepLabels(integrationType);

  return (
    <div className="flex items-center justify-center">
      <div className="flex items-center gap-2">
        {Array.from({ length: totalSteps }, (_, i) => {
          const stepNum = i + 1;
          const isActive = stepNum === currentStep;
          const isCompleted = stepNum < currentStep;

          return (
            <div key={i} className="flex items-center">
              {/* Step indicator */}
              <div className="flex flex-col items-center">
                <div
                  className={`
                    relative w-9 h-9 rounded-lg flex items-center justify-center
                    transition-all duration-300 ease-out
                    ${isActive
                      ? 'bg-cyan-500/20 border-2 border-cyan-500 text-cyan-400 shadow-lg shadow-cyan-500/20'
                      : isCompleted
                        ? 'bg-cyan-500 border-2 border-cyan-500 text-white'
                        : 'bg-slate-800 border-2 border-slate-700 text-slate-500'
                    }
                  `}
                >
                  {isCompleted ? (
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
                    </svg>
                  ) : (
                    <span className="text-sm font-semibold">{stepNum}</span>
                  )}
                </div>
                <span
                  className={`
                    mt-2 text-xs font-medium tracking-wide
                    transition-colors duration-300
                    ${isActive ? 'text-cyan-400' : isCompleted ? 'text-slate-400' : 'text-slate-600'}
                  `}
                >
                  {labels[i]}
                </span>
              </div>

              {/* Connector line */}
              {i < totalSteps - 1 && (
                <div className="relative w-8 h-0.5 mx-2 mt-[-20px]">
                  <div className="absolute inset-0 bg-slate-700 rounded-full" />
                  <div
                    className={`
                      absolute inset-y-0 left-0 bg-cyan-500 rounded-full
                      transition-all duration-500 ease-out
                    `}
                    style={{
                      width: isCompleted ? '100%' : '0%',
                    }}
                  />
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
