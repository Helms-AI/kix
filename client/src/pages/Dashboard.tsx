import { useQuery } from '@tanstack/react-query';
import { FileText, Box, ArrowRight, TrendingUp, Code, FileType } from 'lucide-react';
import { Link } from 'react-router-dom';
import { api } from '../api/client';

interface TypeCardProps {
  title: string;
  value: number;
  icon: React.ComponentType<{ className?: string }>;
  gradient: string;
  hoverBorder: string;
  to: string;
}

function TypeCard({ title, value, icon: Icon, gradient, hoverBorder, to }: TypeCardProps) {
  return (
    <Link
      to={to}
      className={`stat-card group cursor-pointer hover:${hoverBorder} transition-all duration-300`}
    >
      <div className={`absolute top-0 left-0 w-full h-1 bg-gradient-to-r ${gradient}`} />
      <div className="flex items-start justify-between">
        <div>
          <p className="text-sm text-slate-400 font-mono uppercase tracking-wider">{title}</p>
          <p className="text-4xl font-bold text-white mt-2 tabular-nums">{value}</p>
        </div>
        <div className={`p-3 rounded-lg bg-gradient-to-br ${gradient} opacity-80 group-hover:opacity-100 transition-opacity`}>
          <Icon className="w-6 h-6 text-white" />
        </div>
      </div>
      <div className="absolute bottom-4 right-4 opacity-0 group-hover:opacity-100 transition-opacity">
        <ArrowRight className="w-5 h-5 text-slate-400" />
      </div>
    </Link>
  );
}

function CategoryBar({ name, count, maxCount }: { name: string; count: number; maxCount: number }) {
  const percentage = (count / maxCount) * 100;
  return (
    <div className="group">
      <div className="flex justify-between items-center mb-2">
        <span className="text-sm text-slate-300 font-medium truncate pr-2">{name}</span>
        <span className="text-sm text-cyan-400 font-mono tabular-nums">{count}</span>
      </div>
      <div className="h-2 bg-slate-800 rounded-full overflow-hidden">
        <div
          className="h-full bg-gradient-to-r from-cyan-500 to-teal-500 rounded-full transition-all duration-500 group-hover:from-cyan-400 group-hover:to-teal-400"
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  );
}

export default function Dashboard() {
  const { data: stats, isLoading, error } = useQuery({
    queryKey: ['stats'],
    queryFn: api.getStats,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-[60vh]">
        <div className="flex flex-col items-center gap-4">
          <div className="w-12 h-12 border-4 border-cyan-500 border-t-transparent rounded-full animate-spin" />
          <p className="text-slate-400 font-mono">Loading statistics...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="card p-8 text-center">
        <p className="text-red-400">Failed to load statistics</p>
        <p className="text-slate-500 text-sm mt-2">{(error as Error).message}</p>
      </div>
    );
  }

  const maxCategoryCount = Math.max(...(stats?.categories || []).map((c) => c.count), 1);

  return (
    <div className="space-y-8">
      {/* Header */}
      <div>
        <h1 className="text-3xl font-bold text-white">Dashboard</h1>
        <p className="text-slate-400 mt-2">Knowledge Base</p>
      </div>

      {/* Type Cards Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5 gap-4">
        <TypeCard
          title="Total Entries"
          value={stats?.total_entries || 0}
          icon={Box}
          gradient="from-cyan-500 to-teal-500"
          hoverBorder="border-cyan-500/50"
          to="/entries"
        />
        <TypeCard
          title="Documents"
          value={stats?.document_entries || 0}
          icon={FileText}
          gradient="from-blue-500 to-indigo-500"
          hoverBorder="border-blue-500/50"
          to="/entries?type=document"
        />
        <TypeCard
          title="Articles"
          value={stats?.article_entries || 0}
          icon={FileText}
          gradient="from-emerald-500 to-green-500"
          hoverBorder="border-emerald-500/50"
          to="/entries?type=article"
        />
        <TypeCard
          title="PDFs"
          value={stats?.pdf_entries || 0}
          icon={FileType}
          gradient="from-rose-500 to-pink-500"
          hoverBorder="border-rose-500/50"
          to="/entries?type=pdf"
        />
        <TypeCard
          title="Code Chunks"
          value={stats?.chunk_stats?.code || 0}
          icon={Code}
          gradient="from-amber-500 to-orange-500"
          hoverBorder="border-amber-500/50"
          to="/entries?chunk_type=code"
        />
      </div>

      {/* Two Column Layout */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Categories Distribution */}
        <div className="card p-6">
          <div className="flex items-center justify-between mb-6">
            <h2 className="text-xl font-semibold text-white flex items-center gap-2">
              <TrendingUp className="w-5 h-5 text-cyan-400" />
              Categories
            </h2>
            <span className="text-xs text-slate-500 font-mono">
              {stats?.categories?.length || 0} categories
            </span>
          </div>
          <div className="space-y-4">
            {stats?.categories?.slice(0, 8).map((category) => (
              <CategoryBar
                key={category.name}
                name={category.name}
                count={category.count}
                maxCount={maxCategoryCount}
              />
            ))}
            {(!stats?.categories || stats.categories.length === 0) && (
              <p className="text-slate-500 text-sm text-center py-4">No categories yet</p>
            )}
          </div>
          {(stats?.categories?.length || 0) > 8 && (
            <Link
              to="/entries"
              className="mt-6 flex items-center justify-center gap-2 text-sm text-cyan-400 hover:text-cyan-300 transition-colors"
            >
              View all categories
              <ArrowRight className="w-4 h-4" />
            </Link>
          )}
        </div>

        {/* Quick Actions */}
        <div className="card p-6">
          <h2 className="text-xl font-semibold text-white mb-6">Quick Actions</h2>
          <div className="space-y-4">
            <Link
              to="/search"
              className="block p-4 rounded-lg bg-slate-800/50 hover:bg-slate-800 border border-slate-700 hover:border-cyan-500/50 transition-all group"
            >
              <div className="flex items-center gap-4">
                <div className="p-2 rounded-lg bg-cyan-500/10 text-cyan-400 group-hover:bg-cyan-500/20">
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                  </svg>
                </div>
                <div>
                  <h3 className="font-medium text-white">Semantic Search</h3>
                  <p className="text-sm text-slate-400">Find patterns by describing your problem</p>
                </div>
                <ArrowRight className="w-5 h-5 text-slate-500 group-hover:text-cyan-400 ml-auto transition-colors" />
              </div>
            </Link>

            <Link
              to="/entries"
              className="block p-4 rounded-lg bg-slate-800/50 hover:bg-slate-800 border border-slate-700 hover:border-violet-500/50 transition-all group"
            >
              <div className="flex items-center gap-4">
                <div className="p-2 rounded-lg bg-violet-500/10 text-violet-400 group-hover:bg-violet-500/20">
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
                  </svg>
                </div>
                <div>
                  <h3 className="font-medium text-white">Browse Entries</h3>
                  <p className="text-sm text-slate-400">Explore entries by category</p>
                </div>
                <ArrowRight className="w-5 h-5 text-slate-500 group-hover:text-violet-400 ml-auto transition-colors" />
              </div>
            </Link>

            <Link
              to="/graph"
              className="block p-4 rounded-lg bg-slate-800/50 hover:bg-slate-800 border border-slate-700 hover:border-emerald-500/50 transition-all group"
            >
              <div className="flex items-center gap-4">
                <div className="p-2 rounded-lg bg-emerald-500/10 text-emerald-400 group-hover:bg-emerald-500/20">
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
                  </svg>
                </div>
                <div>
                  <h3 className="font-medium text-white">Knowledge Graph</h3>
                  <p className="text-sm text-slate-400">Visualize entry relationships</p>
                </div>
                <ArrowRight className="w-5 h-5 text-slate-500 group-hover:text-emerald-400 ml-auto transition-colors" />
              </div>
            </Link>
          </div>
        </div>
      </div>
    </div>
  );
}
