import type { Job, EnhancedLiveJobData } from '../../../api/indexingClient';
import { CompactJobView } from './CompactJobView';
import { ExpandedJobView } from './ExpandedJobView';

interface ActiveJobCardProps {
  job: Job;
  liveData?: EnhancedLiveJobData;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onCancel: (jobId: string) => void;
  onClearLog?: (jobId: string) => void;
}

export function ActiveJobCard({
  job,
  liveData,
  isExpanded,
  onToggleExpand,
  onCancel,
  onClearLog,
}: ActiveJobCardProps) {
  const isActive = job.status === 'running' || job.status === 'queued';

  return (
    <div
      className={`
        bg-slate-800/80 rounded-xl border overflow-hidden transition-all duration-300
        ${isActive
          ? 'border-cyan-700/50 shadow-lg shadow-cyan-500/5'
          : 'border-slate-700/50'
        }
        ${isExpanded ? 'ring-1 ring-cyan-500/20' : 'hover:border-cyan-700/30'}
      `}
    >
      {/* Compact view (always visible) */}
      <CompactJobView
        job={job}
        liveData={liveData}
        isExpanded={isExpanded}
        onToggle={onToggleExpand}
        onCancel={() => onCancel(job.id)}
      />

      {/* Expanded view (conditional) */}
      <div
        className={`
          overflow-hidden transition-all duration-300 ease-out
          ${isExpanded ? 'max-h-[600px] opacity-100' : 'max-h-0 opacity-0'}
        `}
      >
        <ExpandedJobView
          job={job}
          liveData={liveData}
          onClearLog={onClearLog ? () => onClearLog(job.id) : undefined}
        />
      </div>
    </div>
  );
}
