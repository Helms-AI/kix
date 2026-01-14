import { memo } from 'react';
import type { Job, EnhancedLiveJobData } from '../../../api/indexingClient';
import { JobMetrics } from './JobMetrics';
import { ETACountdown } from './ETACountdown';
import { PageStatusList } from './PageStatusList';
import { CodeExtractionPanel } from '../code-extraction';

interface ExpandedJobViewProps {
  job: Job;
  liveData?: EnhancedLiveJobData;
  onClearLog?: () => void;
}

export const ExpandedJobView = memo(function ExpandedJobView({
  job,
  liveData,
  onClearLog,
}: ExpandedJobViewProps) {
  // Extract values with safe defaults
  const processed = liveData?.processed ?? job.progress?.processed ?? 0;
  const total = liveData?.total ?? job.progress?.total ?? 0;
  const percentage = liveData?.percentage ?? job.progress?.percentage ?? 0;
  const rate = liveData?.rate ?? job.progress?.rate ?? 0;
  const totalChunks = liveData?.totalChunks ?? 0;
  const totalEmbeddings = liveData?.totalEmbeddings ?? 0;
  const errorCount = liveData?.errors?.length ?? 0;
  const rateHistory = liveData?.rateHistory ?? [];
  const pages = liveData?.pages ?? {};
  const etaSeconds = liveData?.etaSeconds;
  const codeExtraction = liveData?.codeExtraction;

  const isActive = job.status === 'running' || job.status === 'queued';

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

      {/* ETA Progress Bar - only show for active jobs */}
      {isActive && (
        <ETACountdown
          etaSeconds={etaSeconds}
          percentage={percentage}
        />
      )}

      {/* Code Extraction Panel - show compact version if we have extraction data */}
      {codeExtraction && codeExtraction.total_code_blocks > 0 && (
        <CodeExtractionPanel
          stats={codeExtraction}
          compact
        />
      )}

      {/* Page Status List */}
      <PageStatusList
        pages={pages}
        maxHeight={200}
        onClear={onClearLog}
      />
    </div>
  );
});
