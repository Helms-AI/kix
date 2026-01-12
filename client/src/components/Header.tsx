import { BookOpen } from 'lucide-react';

/**
 * Application header component with branding and version info.
 * Fixed position at top, spans full width above sidebar.
 */
export function Header() {
  return (
    <header className="fixed top-0 left-0 right-0 z-50 h-16 bg-slate-900/95 backdrop-blur-xl border-b border-slate-800/80">
      {/* Subtle gradient accent line at top */}
      <div className="absolute top-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-cyan-500/50 to-transparent" />

      <div className="h-full flex items-center justify-between px-4 lg:px-6">
        {/* Left: Logo + Branding */}
        <div className="flex items-center gap-3">
          {/* Spacer for mobile hamburger menu */}
          <div className="lg:hidden w-12" />

          {/* Logo Icon with glow effect */}
          <div className="relative group">
            <div className="absolute inset-0 bg-gradient-to-br from-cyan-500 to-teal-500 rounded-xl blur-md opacity-40 group-hover:opacity-60 transition-opacity duration-500" />
            <div className="relative w-10 h-10 rounded-xl bg-gradient-to-br from-cyan-500 to-teal-500 flex items-center justify-center shadow-lg shadow-cyan-500/20">
              <BookOpen className="w-5 h-5 text-white" />
            </div>
          </div>

          {/* Title + Subtitle */}
          <div className="flex flex-col">
            <h1 className="text-lg font-bold text-white leading-tight tracking-tight">
              Kix
            </h1>
            <p className="text-[10px] text-slate-500 font-mono uppercase tracking-widest hidden sm:block">
              Knowledge Indexing System
            </p>
          </div>
        </div>

        {/* Right: Version Badge */}
        <div className="flex items-center gap-4">
          <span className="text-[10px] text-slate-600 font-mono tracking-wide px-2 py-1 rounded-md bg-slate-800/50 border border-slate-700/50">
            v0.1.0
          </span>
        </div>
      </div>
    </header>
  );
}
