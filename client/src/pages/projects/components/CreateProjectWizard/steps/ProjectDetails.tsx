import { useState } from 'react';

interface ProjectDetailsProps {
  name: string;
  description: string;
  color: string;
  isGitHub: boolean;
  repoName: string | null;
  onNameChange: (name: string) => void;
  onDescriptionChange: (description: string) => void;
  onColorChange: (color: string) => void;
}

const PRESET_COLORS = [
  { name: 'Blue', value: '#3B82F6' },
  { name: 'Cyan', value: '#06B6D4' },
  { name: 'Emerald', value: '#10B981' },
  { name: 'Amber', value: '#F59E0B' },
  { name: 'Rose', value: '#F43F5E' },
  { name: 'Purple', value: '#8B5CF6' },
  { name: 'Indigo', value: '#6366F1' },
  { name: 'Slate', value: '#64748B' },
];

export default function ProjectDetails({
  name,
  description,
  color,
  isGitHub,
  repoName,
  onNameChange,
  onDescriptionChange,
  onColorChange,
}: ProjectDetailsProps) {
  const [isCustomColor, setIsCustomColor] = useState(
    !PRESET_COLORS.some(p => p.value === color)
  );

  return (
    <div className="space-y-6">
      <div className="text-center">
        <h3 className="text-lg font-medium text-white mb-2">
          Project Details
        </h3>
        <p className="text-sm text-slate-400">
          {isGitHub ? 'Configure your GitHub-connected project' : 'Set up your local project'}
        </p>
      </div>

      <div className="space-y-5 mt-8">
        {/* GitHub Summary Banner */}
        {isGitHub && repoName && (
          <div className="p-4 bg-gradient-to-r from-cyan-500/10 to-transparent border border-cyan-500/30 rounded-lg">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-lg bg-cyan-500/20 flex items-center justify-center">
                <svg className="w-5 h-5 text-cyan-400" fill="currentColor" viewBox="0 0 24 24">
                  <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" />
                </svg>
              </div>
              <div>
                <p className="text-xs text-slate-400 uppercase tracking-wide">Connected to GitHub</p>
                <p className="text-white font-medium">{repoName}</p>
              </div>
            </div>
          </div>
        )}

        {/* Project Name */}
        <div>
          <label className="block text-sm font-medium text-slate-300 mb-2">
            Project Name <span className="text-red-400">*</span>
          </label>
          <input
            type="text"
            value={name}
            onChange={(e) => onNameChange(e.target.value)}
            placeholder="My Awesome Project"
            className="
              w-full px-4 py-3
              bg-slate-800 border-2 border-slate-700 rounded-lg
              text-white text-sm placeholder:text-slate-500
              focus:outline-none focus:border-cyan-500
              transition-colors duration-200
            "
          />
          {isGitHub && repoName && name === repoName && (
            <p className="mt-2 text-xs text-slate-400 flex items-center gap-1.5">
              <svg className="w-3.5 h-3.5 text-cyan-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              Auto-filled from repository name
            </p>
          )}
        </div>

        {/* Description */}
        <div>
          <label className="block text-sm font-medium text-slate-300 mb-2">
            Description
            <span className="text-slate-500 font-normal ml-2">(optional)</span>
          </label>
          <textarea
            value={description}
            onChange={(e) => onDescriptionChange(e.target.value)}
            placeholder="Describe your project's purpose and goals..."
            rows={3}
            className="
              w-full px-4 py-3
              bg-slate-800 border-2 border-slate-700 rounded-lg
              text-white text-sm placeholder:text-slate-500
              focus:outline-none focus:border-cyan-500
              transition-colors duration-200
              resize-none
            "
          />
        </div>

        {/* Color Picker */}
        <div>
          <label className="block text-sm font-medium text-slate-300 mb-3">
            Project Color
          </label>

          {/* Preset Colors */}
          <div className="flex flex-wrap gap-2 mb-4">
            {PRESET_COLORS.map((preset) => (
              <button
                key={preset.value}
                type="button"
                onClick={() => {
                  onColorChange(preset.value);
                  setIsCustomColor(false);
                }}
                className={`
                  relative w-10 h-10 rounded-lg
                  transition-all duration-200
                  ${color === preset.value && !isCustomColor
                    ? 'ring-2 ring-offset-2 ring-offset-slate-900 ring-white scale-110'
                    : 'hover:scale-105'
                  }
                `}
                style={{ backgroundColor: preset.value }}
                title={preset.name}
              >
                {color === preset.value && !isCustomColor && (
                  <svg
                    className="absolute inset-0 m-auto w-5 h-5 text-white drop-shadow-md"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
                  </svg>
                )}
              </button>
            ))}

            {/* Custom color button */}
            <button
              type="button"
              onClick={() => setIsCustomColor(true)}
              className={`
                relative w-10 h-10 rounded-lg border-2 border-dashed
                transition-all duration-200
                ${isCustomColor
                  ? 'border-white ring-2 ring-offset-2 ring-offset-slate-900 ring-white'
                  : 'border-slate-600 hover:border-slate-500'
                }
              `}
              style={isCustomColor ? { backgroundColor: color } : undefined}
              title="Custom color"
            >
              {!isCustomColor && (
                <svg className="absolute inset-0 m-auto w-5 h-5 text-slate-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6v6m0 0v6m0-6h6m-6 0H6" />
                </svg>
              )}
              {isCustomColor && (
                <svg
                  className="absolute inset-0 m-auto w-5 h-5 text-white drop-shadow-md"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
                </svg>
              )}
            </button>
          </div>

          {/* Custom Color Input */}
          {isCustomColor && (
            <div className="flex items-center gap-3">
              <input
                type="color"
                value={color}
                onChange={(e) => onColorChange(e.target.value)}
                className="
                  w-12 h-10 rounded-lg cursor-pointer
                  bg-slate-800 border-2 border-slate-700
                  [&::-webkit-color-swatch-wrapper]:p-1
                  [&::-webkit-color-swatch]:rounded
                "
              />
              <input
                type="text"
                value={color.toUpperCase()}
                onChange={(e) => {
                  const val = e.target.value;
                  if (/^#[0-9A-Fa-f]{0,6}$/.test(val)) {
                    onColorChange(val);
                  }
                }}
                placeholder="#000000"
                className="
                  flex-1 px-4 py-2
                  bg-slate-800 border-2 border-slate-700 rounded-lg
                  text-white text-sm font-mono placeholder:text-slate-500
                  focus:outline-none focus:border-cyan-500
                  transition-colors duration-200
                "
              />
            </div>
          )}
        </div>

        {/* Preview */}
        <div className="pt-4 border-t border-slate-800">
          <p className="text-xs text-slate-500 uppercase tracking-wide mb-3">Preview</p>
          <div className="flex items-center gap-4 p-4 bg-slate-800/50 rounded-lg">
            <div
              className="w-12 h-12 rounded-xl flex items-center justify-center text-white font-bold text-lg"
              style={{ backgroundColor: color }}
            >
              {name ? name.charAt(0).toUpperCase() : 'P'}
            </div>
            <div>
              <h4 className="text-white font-medium">
                {name || 'Project Name'}
              </h4>
              {description ? (
                <p className="text-sm text-slate-400 line-clamp-1 mt-0.5">{description}</p>
              ) : (
                <p className="text-sm text-slate-500 italic mt-0.5">No description</p>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
