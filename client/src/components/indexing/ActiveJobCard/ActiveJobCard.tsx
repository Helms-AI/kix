import { memo } from 'react';
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

export const ActiveJobCard = memo(function ActiveJobCard({
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
        bg-slate-800/80 backdrop-blur-sm rounded-xl border overflow-hidden
        transition-all duration-300 ease-out
        ${isActive
          ? 'border-cyan-700/50 shadow-lg shadow-cyan-950/30'
          : 'border-slate-700/50'
        }
        ${isExpanded
          ? 'ring-1 ring-cyan-500/20'
          : 'hover:border-cyan-700/40 hover:shadow-md hover:shadow-cyan-950/20'
        }
      `}
    >
      {/* Compact view - always visible */}
      <CompactJobView
        job={job}
        liveData={liveData}
        isExpanded={isExpanded}
        onToggle={onToggleExpand}
        onCancel={() => onCancel(job.id)}
      />

      {/* Expanded view - animated visibility */}
      <div
        className={`
          transition-all duration-300 ease-out overflow-hidden
          ${isExpanded
            ? 'max-h-[600px] opacity-100'
            : 'max-h-0 opacity-0'
          }
        `}
      >
        <ExpandedJobView
          job={job}
          liveData={liveData}
          onClearLog={onClearLog ? () => onClearLog(job.id) : undefined}
        />
      </div>

      {/* Active job glow effect */}
      {isActive && (
        <div
          className="absolute inset-0 pointer-events-none rounded-xl opacity-20"
          style={{
            background: 'radial-gradient(ellipse at top, rgba(34, 211, 238, 0.15) 0%, transparent 50%)',
          }}
        />
      )}
    </div>
  );
});
