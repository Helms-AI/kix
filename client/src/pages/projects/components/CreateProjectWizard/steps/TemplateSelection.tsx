import type { ProjectTemplate } from '../index';

interface TemplateSelectionProps {
  selected: ProjectTemplate;
  onSelect: (template: ProjectTemplate) => void;
}

interface TemplateConfig {
  id: ProjectTemplate;
  name: string;
  description: string;
  bestFor: string;
  icon: JSX.Element;
  statuses: string[];
  customFields?: string[];
  accent: string;
}

const templates: TemplateConfig[] = [
  {
    id: 'kanban',
    name: 'Kanban',
    description: 'Visual workflow management with simple status columns',
    bestFor: 'General task management',
    accent: 'cyan',
    icon: (
      <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 17V7m0 10a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2h2a2 2 0 012 2m0 10a2 2 0 002 2h2a2 2 0 002-2M9 7a2 2 0 012-2h2a2 2 0 012 2m0 10V7m0 10a2 2 0 002 2h2a2 2 0 002-2V7a2 2 0 00-2-2h-2a2 2 0 00-2 2" />
      </svg>
    ),
    statuses: ['Todo', 'In Progress', 'Done'],
  },
  {
    id: 'bug_tracking',
    name: 'Bug Tracking',
    description: 'Triage workflow with severity and priority tracking',
    bestFor: 'Issue tracking & QA',
    accent: 'rose',
    icon: (
      <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
      </svg>
    ),
    statuses: ['Triage', 'Confirmed', 'In Progress', 'Fixed', 'Closed'],
    customFields: ['Priority', 'Severity'],
  },
  {
    id: 'sprint_planning',
    name: 'Sprint Planning',
    description: 'Agile workflow with sprint cycles and story points',
    bestFor: 'Scrum teams',
    accent: 'violet',
    icon: (
      <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M13 10V3L4 14h7v7l9-11h-7z" />
      </svg>
    ),
    statuses: ['Backlog', 'Sprint', 'In Progress', 'Review', 'Done'],
    customFields: ['Sprint', 'Story Points'],
  },
  {
    id: 'feature_roadmap',
    name: 'Feature Roadmap',
    description: 'Quarter-based planning with team assignment',
    bestFor: 'Product planning',
    accent: 'amber',
    icon: (
      <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7" />
      </svg>
    ),
    statuses: ['Planned', 'In Design', 'Development', 'Shipped'],
    customFields: ['Quarter', 'Team'],
  },
];

const accentColors: Record<string, { bg: string; border: string; text: string; glow: string; statusBg: string }> = {
  cyan: {
    bg: 'bg-cyan-500/10',
    border: 'border-cyan-500',
    text: 'text-cyan-400',
    glow: 'shadow-cyan-500/25',
    statusBg: 'bg-cyan-500/20',
  },
  rose: {
    bg: 'bg-rose-500/10',
    border: 'border-rose-500',
    text: 'text-rose-400',
    glow: 'shadow-rose-500/25',
    statusBg: 'bg-rose-500/20',
  },
  violet: {
    bg: 'bg-violet-500/10',
    border: 'border-violet-500',
    text: 'text-violet-400',
    glow: 'shadow-violet-500/25',
    statusBg: 'bg-violet-500/20',
  },
  amber: {
    bg: 'bg-amber-500/10',
    border: 'border-amber-500',
    text: 'text-amber-400',
    glow: 'shadow-amber-500/25',
    statusBg: 'bg-amber-500/20',
  },
};

export default function TemplateSelection({ selected, onSelect }: TemplateSelectionProps) {
  return (
    <div className="space-y-6">
      <div className="text-center">
        <h3 className="text-lg font-medium text-white mb-2">
          Choose Your Project Board
        </h3>
        <p className="text-sm text-slate-400">
          Select a template for your GitHub Project V2 board
        </p>
      </div>

      <div className="grid grid-cols-2 gap-4 mt-6">
        {templates.map((template) => {
          const isSelected = selected === template.id;
          const colors = accentColors[template.accent];

          return (
            <button
              key={template.id}
              onClick={() => onSelect(template.id)}
              className={`
                group relative p-5 rounded-xl border-2 text-left
                transition-all duration-300 ease-out overflow-hidden
                ${isSelected
                  ? `${colors.border} ${colors.bg} shadow-lg ${colors.glow}`
                  : 'border-slate-700/80 bg-slate-800/40 hover:border-slate-600 hover:bg-slate-800/60'
                }
              `}
            >
              {/* Background pattern */}
              <div
                className={`
                  absolute inset-0 opacity-[0.03] pointer-events-none
                  ${isSelected ? 'opacity-[0.05]' : ''}
                `}
                style={{
                  backgroundImage: `url("data:image/svg+xml,%3Csvg width='60' height='60' viewBox='0 0 60 60' xmlns='http://www.w3.org/2000/svg'%3E%3Cg fill='none' fill-rule='evenodd'%3E%3Cg fill='%23ffffff' fill-opacity='1'%3E%3Cpath d='M36 34v-4h-2v4h-4v2h4v4h2v-4h4v-2h-4zm0-30V0h-2v4h-4v2h4v4h2V6h4V4h-4zM6 34v-4H4v4H0v2h4v4h2v-4h4v-2H6zM6 4V0H4v4H0v2h4v4h2V6h4V4H6z'/%3E%3C/g%3E%3C/g%3E%3C/svg%3E")`,
                }}
              />

              {/* Selection indicator */}
              <div
                className={`
                  absolute top-3 right-3 w-5 h-5 rounded-full border-2
                  flex items-center justify-center transition-all duration-200
                  ${isSelected
                    ? `${colors.border} ${colors.bg}`
                    : 'border-slate-600 bg-transparent'
                  }
                `}
              >
                {isSelected && (
                  <svg
                    className={`w-3 h-3 ${colors.text}`}
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                  </svg>
                )}
              </div>

              {/* Header with icon */}
              <div className="flex items-start gap-3 mb-3">
                <div
                  className={`
                    w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0
                    transition-all duration-300
                    ${isSelected
                      ? `${colors.statusBg} ${colors.text}`
                      : 'bg-slate-700/70 text-slate-400 group-hover:text-slate-300'
                    }
                  `}
                >
                  {template.icon}
                </div>
                <div className="min-w-0 flex-1">
                  <h4
                    className={`
                      text-sm font-semibold mb-0.5 transition-colors duration-200
                      ${isSelected ? 'text-white' : 'text-slate-200 group-hover:text-white'}
                    `}
                  >
                    {template.name}
                  </h4>
                  <p className="text-xs text-slate-500 leading-snug line-clamp-2">
                    {template.description}
                  </p>
                </div>
              </div>

              {/* Status flow visualization */}
              <div className="mb-3">
                <div className="flex items-center gap-1 flex-wrap">
                  {template.statuses.map((status, idx) => (
                    <div key={status} className="flex items-center">
                      <span
                        className={`
                          px-2 py-0.5 text-[10px] font-medium rounded-md
                          ${isSelected
                            ? `${colors.statusBg} ${colors.text}`
                            : 'bg-slate-700/60 text-slate-400'
                          }
                        `}
                      >
                        {status}
                      </span>
                      {idx < template.statuses.length - 1 && (
                        <svg
                          className={`w-3 h-3 mx-0.5 ${isSelected ? colors.text : 'text-slate-600'}`}
                          fill="none"
                          stroke="currentColor"
                          viewBox="0 0 24 24"
                        >
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                        </svg>
                      )}
                    </div>
                  ))}
                </div>
              </div>

              {/* Custom fields */}
              {template.customFields && (
                <div className="flex flex-wrap gap-1.5 mb-3">
                  {template.customFields.map((field) => (
                    <span
                      key={field}
                      className={`
                        inline-flex items-center gap-1 px-2 py-0.5 text-[10px] font-medium rounded
                        border
                        ${isSelected
                          ? `border-slate-600/80 text-slate-300 bg-slate-800/50`
                          : 'border-slate-700/60 text-slate-500 bg-slate-800/30'
                        }
                      `}
                    >
                      <svg className="w-2.5 h-2.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />
                      </svg>
                      {field}
                    </span>
                  ))}
                </div>
              )}

              {/* Best for */}
              <div className="flex items-center gap-1.5">
                <span className="text-[10px] uppercase tracking-wider text-slate-500 font-medium">
                  Best for:
                </span>
                <span
                  className={`
                    text-[10px] font-medium
                    ${isSelected ? colors.text : 'text-slate-400'}
                  `}
                >
                  {template.bestFor}
                </span>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
