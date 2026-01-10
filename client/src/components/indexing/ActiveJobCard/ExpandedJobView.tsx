import type { Job, EnhancedLiveJobData } from '../../../api/indexingClient';
import { JobMetrics } from './JobMetrics';
import { ETACountdown } from './ETACountdown';
import { ProcessingLog } from './ProcessingLog';

interface ExpandedJobViewProps {
  job: Job;
  liveData?: EnhancedLiveJobData;
  onClearLog?: () => void;
}

export function ExpandedJobView({
  job,
  liveData,
  onClearLog,
}: ExpandedJobViewProps) {
  const processed = liveData?.processed ?? job.progress?.processed ?? 0;
  const total = liveData?.total ?? job.progress?.total ?? 0;
  const percentage = liveData?.percentage ?? job.progress?.percentage ?? 0;
  const rate = liveData?.rate ?? job.progress?.rate ?? 0;
  const totalChunks = liveData?.totalChunks ?? 0;
  const totalEmbeddings = liveData?.totalEmbeddings ?? 0;
  const errorCount = liveData?.errors?.length ?? 0;
  const rateHistory = liveData?.rateHistory ?? [];
  const log = liveData?.log ?? [];
  const etaSeconds = liveData?.etaSeconds;

  return (
    <div className="px-4 pb-4 space-y-4 border-t border-slate-700/50">
      {/* Metrics Grid */}
      <JobMetrics
        processed={processed}
        total={total}
        totalChunks={totalChunks}
        totalEmbeddings={totalEmbeddings}
        errorCount={errorCount}
        rate={rate}
        rateHistory={rateHistory}
        className="pt-4"
      />

      {/* ETA Progress Bar */}
      {(job.status === 'running' || job.status === 'queued') && (
        <ETACountdown
          etaSeconds={etaSeconds}
          percentage={percentage}
        />
      )}

      {/* Processing Log */}
      <div className="bg-slate-900/50 rounded-lg border border-slate-800 overflow-hidden">
        <ProcessingLog
          entries={log}
          maxHeight={200}
          onClear={onClearLog}
        />
      </div>
    </div>
  );
}
