import { Plug } from 'lucide-react';

export default function AdminIntegrations() {
  return (
    <div className="space-y-8">
      {/* Header */}
      <div className="flex items-center gap-4">
        <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-cyan-500/20 to-teal-500/20 border border-cyan-500/20 flex items-center justify-center">
          <Plug className="w-6 h-6 text-cyan-400" />
        </div>
        <div>
          <h2 className="text-xl font-bold text-white">Integrations</h2>
          <p className="text-sm text-slate-400 mt-0.5">
            Connect KIX to external services and platforms
          </p>
        </div>
      </div>

      {/* Future Integrations Placeholder */}
      <div className="p-8 border-2 border-dashed border-slate-700/50 rounded-xl">
        <div className="text-center">
          <div className="w-16 h-16 mx-auto rounded-xl bg-slate-800 border border-slate-700 flex items-center justify-center mb-4">
            <Plug className="w-8 h-8 text-slate-500" />
          </div>
          <h3 className="text-xl font-medium text-slate-300 mb-2">Integrations coming soon</h3>
          <p className="text-sm text-slate-500 max-w-md mx-auto">
            We're working on integrations with popular development tools and platforms.
            Check back later for updates on GitHub, Linear, Jira, and more.
          </p>
        </div>
      </div>
    </div>
  );
}
