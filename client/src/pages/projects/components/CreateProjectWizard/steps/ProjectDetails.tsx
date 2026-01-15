import { useState } from 'react';

interface ProjectDetailsProps {
  name: string;
  description: string;
  color: string;
  isGitHub?: boolean;
  repoName?: string | null;
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
          Configure your project settings
        </p>
      </div>

      <div className="space-y-5 mt-6">
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
