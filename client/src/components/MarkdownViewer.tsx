import { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import { Eye, Code2 } from 'lucide-react';
import clsx from 'clsx';

type ViewMode = 'rendered' | 'code';

interface MarkdownViewerProps {
  content: string;
  className?: string;
  defaultView?: ViewMode;
}

/**
 * MarkdownViewer - Displays markdown content with a toggle between
 * rendered view and raw code view.
 */
export default function MarkdownViewer({
  content,
  className,
  defaultView = 'rendered',
}: MarkdownViewerProps) {
  const [viewMode, setViewMode] = useState<ViewMode>(defaultView);

  if (!content) {
    return (
      <p className={clsx('text-slate-500 italic', className)}>
        No content available
      </p>
    );
  }

  return (
    <div className={clsx('relative group', className)}>
      {/* Mode Toggle - Compact pill design */}
      <div
        className="absolute top-0 right-0 z-20 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity duration-200"
        onClick={(e) => e.stopPropagation()}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center bg-slate-800/90 backdrop-blur-sm rounded-full p-0.5 border border-slate-700/50 shadow-lg shadow-black/20">
          <button
            onClick={(e) => { e.stopPropagation(); setViewMode('rendered'); }}
            className={clsx(
              'flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium transition-all duration-200',
              viewMode === 'rendered'
                ? 'bg-cyan-600 text-white shadow-sm shadow-cyan-500/30'
                : 'text-slate-400 hover:text-slate-200'
            )}
            title="Rendered view"
            aria-pressed={viewMode === 'rendered'}
          >
            <Eye className="w-3 h-3" />
            <span className="sr-only sm:not-sr-only">View</span>
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); setViewMode('code'); }}
            className={clsx(
              'flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium transition-all duration-200',
              viewMode === 'code'
                ? 'bg-cyan-600 text-white shadow-sm shadow-cyan-500/30'
                : 'text-slate-400 hover:text-slate-200'
            )}
            title="Raw markdown"
            aria-pressed={viewMode === 'code'}
          >
            <Code2 className="w-3 h-3" />
            <span className="sr-only sm:not-sr-only">Code</span>
          </button>
        </div>
      </div>

      {/* Content Area */}
      <div className="relative overflow-hidden">
        {/* Rendered Markdown View */}
        <div
          className={clsx(
            'transition-all duration-300 ease-out',
            viewMode === 'rendered'
              ? 'opacity-100 translate-y-0'
              : 'opacity-0 absolute inset-0 -translate-y-2 pointer-events-none'
          )}
        >
          <div className="prose prose-invert prose-slate prose-sm max-w-none">
            <ReactMarkdown
              components={{
                // Custom code block styling
                code: ({ className, children, ...props }) => {
                  const isInline = !className;
                  if (isInline) {
                    return (
                      <code
                        className="px-1.5 py-0.5 bg-slate-800 text-cyan-300 rounded text-sm font-mono"
                        {...props}
                      >
                        {children}
                      </code>
                    );
                  }
                  return (
                    <code className={className} {...props}>
                      {children}
                    </code>
                  );
                },
                // Enhanced pre block styling
                pre: ({ children, ...props }) => (
                  <pre
                    className="bg-slate-800/70 border border-slate-700/50 rounded-lg p-4 overflow-x-auto font-mono text-sm"
                    {...props}
                  >
                    {children}
                  </pre>
                ),
                // Link styling
                a: ({ children, href, ...props }) => (
                  <a
                    href={href}
                    className="text-cyan-400 hover:text-cyan-300 underline underline-offset-2 transition-colors"
                    target="_blank"
                    rel="noopener noreferrer"
                    {...props}
                  >
                    {children}
                  </a>
                ),
                // Heading styles
                h1: ({ children, ...props }) => (
                  <h1 className="text-xl font-semibold text-white mt-4 mb-2" {...props}>
                    {children}
                  </h1>
                ),
                h2: ({ children, ...props }) => (
                  <h2 className="text-lg font-semibold text-white mt-3 mb-2" {...props}>
                    {children}
                  </h2>
                ),
                h3: ({ children, ...props }) => (
                  <h3 className="text-base font-semibold text-white mt-2 mb-1" {...props}>
                    {children}
                  </h3>
                ),
                // List styling
                ul: ({ children, ...props }) => (
                  <ul className="list-disc list-inside space-y-1 text-slate-300" {...props}>
                    {children}
                  </ul>
                ),
                ol: ({ children, ...props }) => (
                  <ol className="list-decimal list-inside space-y-1 text-slate-300" {...props}>
                    {children}
                  </ol>
                ),
                // Blockquote styling
                blockquote: ({ children, ...props }) => (
                  <blockquote
                    className="border-l-2 border-cyan-500/50 pl-4 italic text-slate-400"
                    {...props}
                  >
                    {children}
                  </blockquote>
                ),
                // Strong/Bold styling
                strong: ({ children, ...props }) => (
                  <strong className="font-semibold text-white" {...props}>
                    {children}
                  </strong>
                ),
                // Emphasis/Italic styling
                em: ({ children, ...props }) => (
                  <em className="italic text-slate-200" {...props}>
                    {children}
                  </em>
                ),
              }}
            >
              {content}
            </ReactMarkdown>
          </div>
        </div>

        {/* Raw Code View */}
        <div
          className={clsx(
            'transition-all duration-300 ease-out',
            viewMode === 'code'
              ? 'opacity-100 translate-y-0'
              : 'opacity-0 absolute inset-0 translate-y-2 pointer-events-none'
          )}
        >
          <div className="relative">
            {/* Subtle code label */}
            <div className="absolute top-2 right-2 px-2 py-0.5 bg-slate-700/50 rounded text-xs text-slate-500 font-mono">
              markdown
            </div>
            <pre className="bg-slate-800/50 border border-slate-700/30 rounded-lg p-4 pt-8 overflow-x-auto">
              <code className="text-sm font-mono text-slate-300 leading-relaxed whitespace-pre-wrap break-words">
                {content}
              </code>
            </pre>
          </div>
        </div>
      </div>
    </div>
  );
}
