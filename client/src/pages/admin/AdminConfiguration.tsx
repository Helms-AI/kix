import { useState } from 'react';
import clsx from 'clsx';
import {
  Save,
  RotateCcw,
  Database,
  Cpu,
  Globe,
  Bot,
  CheckCircle,
  AlertCircle,
  Loader2,
  Settings,
  Sliders
} from 'lucide-react';

interface RuntimeSettings {
  cache: {
    searchTTL: number;
    embeddingTTL: number;
    searchCapacity: number;
    embeddingCapacity: number;
  };
  jobs: {
    maxConcurrent: number;
    retryLimit: number;
    timeout: number;
  };
  api: {
    rateLimit: number;
    maxPageSize: number;
    corsEnabled: boolean;
  };
  crawler: {
    userAgent: string;
    maxDepth: number;
    delayMs: number;
    maxPages: number;
  };
}

interface SettingInputProps {
  label: string;
  description: string;
  value: number | string | boolean;
  onChange: (value: number | string | boolean) => void;
  type: 'number' | 'text' | 'boolean';
  min?: number;
  max?: number;
  unit?: string;
}

function SettingInput({
  label,
  description,
  value,
  onChange,
  type,
  min,
  max,
  unit
}: SettingInputProps) {
  return (
    <div className="group py-5 border-b border-slate-700/50 last:border-0">
      <div className="flex items-start justify-between gap-4">
        <div className="flex-1">
          <label className="block text-sm font-medium text-white group-hover:text-cyan-400 transition-colors">
            {label}
          </label>
          <p className="text-sm text-slate-400 mt-1">{description}</p>
        </div>
        <div className="flex items-center gap-3">
          {type === 'boolean' ? (
            <button
              onClick={() => onChange(!value)}
              className={clsx(
                'toggle-switch',
                value && 'toggle-switch-active'
              )}
            >
              <span className="toggle-knob" />
            </button>
          ) : type === 'number' ? (
            <div className="flex items-center gap-2">
              <input
                type="number"
                value={value as number}
                onChange={(e) => onChange(Number(e.target.value))}
                min={min}
                max={max}
                className="admin-input w-28 text-right font-mono-data"
              />
              {unit && (
                <span className="text-sm text-slate-400 min-w-[60px]">{unit}</span>
              )}
            </div>
          ) : (
            <input
              type="text"
              value={String(value)}
              onChange={(e) => onChange(e.target.value)}
              className="admin-input w-72"
            />
          )}
        </div>
      </div>
    </div>
  );
}

interface SettingsSectionProps {
  title: string;
  icon: React.ComponentType<{ className?: string }>;
  children: React.ReactNode;
}

function SettingsSection({ title, icon: Icon, children }: SettingsSectionProps) {
  return (
    <div className="admin-card admin-card-glow p-6">
      <div className="relative z-10">
        <div className="section-header">
          <div className="section-header-icon">
            <Icon className="w-5 h-5 text-cyan-400" />
          </div>
          <h3 className="text-lg font-semibold text-white">{title}</h3>
        </div>
        <div className="space-y-1">{children}</div>
      </div>
    </div>
  );
}

export default function AdminConfiguration() {
  const [hasChanges, setHasChanges] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'success' | 'error'>('idle');

  const [settings, setSettings] = useState<RuntimeSettings>({
    cache: {
      searchTTL: 30,
      embeddingTTL: 600,
      searchCapacity: 1000,
      embeddingCapacity: 500,
    },
    jobs: {
      maxConcurrent: 3,
      retryLimit: 3,
      timeout: 300,
    },
    api: {
      rateLimit: 100,
      maxPageSize: 50,
      corsEnabled: true,
    },
    crawler: {
      userAgent: 'KIX-Crawler/1.0',
      maxDepth: 3,
      delayMs: 1000,
      maxPages: 100,
    },
  });

  const updateSetting = (
    category: keyof RuntimeSettings,
    key: string,
    value: number | string | boolean
  ) => {
    setSettings((prev) => ({
      ...prev,
      [category]: {
        ...prev[category],
        [key]: value,
      },
    }));
    setHasChanges(true);
    setSaveStatus('idle');
  };

  const handleSave = async () => {
    setSaveStatus('saving');
    try {
      await new Promise((resolve) => setTimeout(resolve, 1500));
      setSaveStatus('success');
      setHasChanges(false);
      setTimeout(() => setSaveStatus('idle'), 3000);
    } catch {
      setSaveStatus('error');
      setTimeout(() => setSaveStatus('idle'), 3000);
    }
  };

  const handleReset = () => {
    setSettings({
      cache: {
        searchTTL: 30,
        embeddingTTL: 600,
        searchCapacity: 1000,
        embeddingCapacity: 500,
      },
      jobs: {
        maxConcurrent: 3,
        retryLimit: 3,
        timeout: 300,
      },
      api: {
        rateLimit: 100,
        maxPageSize: 50,
        corsEnabled: true,
      },
      crawler: {
        userAgent: 'KIX-Crawler/1.0',
        maxDepth: 3,
        delayMs: 1000,
        maxPages: 100,
      },
    });
    setHasChanges(false);
    setSaveStatus('idle');
  };

  return (
    <div className="space-y-8">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-cyan-500/20 to-teal-500/20 border border-cyan-500/20 flex items-center justify-center">
            <Sliders className="w-6 h-6 text-cyan-400" />
          </div>
          <div>
            <h2 className="text-xl font-bold text-white">Runtime Configuration</h2>
            <p className="text-sm text-slate-400 mt-0.5">
              Adjust system parameters in real-time
            </p>
          </div>
        </div>

        {/* Save Status */}
        {saveStatus !== 'idle' && (
          <div
            className={clsx(
              'flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium',
              saveStatus === 'saving' && 'bg-cyan-500/10 text-cyan-400 border border-cyan-500/20',
              saveStatus === 'success' && 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20',
              saveStatus === 'error' && 'bg-red-500/10 text-red-400 border border-red-500/20'
            )}
          >
            {saveStatus === 'saving' && (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                <span>Saving configuration...</span>
              </>
            )}
            {saveStatus === 'success' && (
              <>
                <CheckCircle className="w-4 h-4" />
                <span>Configuration saved!</span>
              </>
            )}
            {saveStatus === 'error' && (
              <>
                <AlertCircle className="w-4 h-4" />
                <span>Failed to save</span>
              </>
            )}
          </div>
        )}
      </div>

      {/* Cache Settings */}
      <SettingsSection title="Cache Settings" icon={Database}>
        <SettingInput
          label="Search Cache TTL"
          description="Time before search cache entries expire and are refreshed"
          value={settings.cache.searchTTL}
          onChange={(v) => updateSetting('cache', 'searchTTL', v)}
          type="number"
          min={30}
          max={3600}
          unit="seconds"
        />
        <SettingInput
          label="Embedding Cache TTL"
          description="Time before embedding cache entries expire"
          value={settings.cache.embeddingTTL}
          onChange={(v) => updateSetting('cache', 'embeddingTTL', v)}
          type="number"
          min={60}
          max={7200}
          unit="seconds"
        />
        <SettingInput
          label="Search Cache Capacity"
          description="Maximum number of search results to cache"
          value={settings.cache.searchCapacity}
          onChange={(v) => updateSetting('cache', 'searchCapacity', v)}
          type="number"
          min={100}
          max={10000}
          unit="entries"
        />
        <SettingInput
          label="Embedding Cache Capacity"
          description="Maximum number of embeddings to cache"
          value={settings.cache.embeddingCapacity}
          onChange={(v) => updateSetting('cache', 'embeddingCapacity', v)}
          type="number"
          min={100}
          max={5000}
          unit="entries"
        />
      </SettingsSection>

      {/* Job Settings */}
      <SettingsSection title="Job Settings" icon={Cpu}>
        <SettingInput
          label="Maximum Concurrent Jobs"
          description="Number of indexing jobs that can run simultaneously"
          value={settings.jobs.maxConcurrent}
          onChange={(v) => updateSetting('jobs', 'maxConcurrent', v)}
          type="number"
          min={1}
          max={10}
          unit="jobs"
        />
        <SettingInput
          label="Retry Limit"
          description="Maximum retries for failed jobs before marking as failed"
          value={settings.jobs.retryLimit}
          onChange={(v) => updateSetting('jobs', 'retryLimit', v)}
          type="number"
          min={0}
          max={5}
          unit="retries"
        />
        <SettingInput
          label="Job Timeout"
          description="Maximum time a single job can run before timing out"
          value={settings.jobs.timeout}
          onChange={(v) => updateSetting('jobs', 'timeout', v)}
          type="number"
          min={30}
          max={600}
          unit="seconds"
        />
      </SettingsSection>

      {/* API Settings */}
      <SettingsSection title="API Settings" icon={Globe}>
        <SettingInput
          label="Rate Limit"
          description="Maximum API requests allowed per minute"
          value={settings.api.rateLimit}
          onChange={(v) => updateSetting('api', 'rateLimit', v)}
          type="number"
          min={10}
          max={1000}
          unit="req/min"
        />
        <SettingInput
          label="Maximum Page Size"
          description="Maximum items returned per paginated API response"
          value={settings.api.maxPageSize}
          onChange={(v) => updateSetting('api', 'maxPageSize', v)}
          type="number"
          min={10}
          max={100}
          unit="items"
        />
        <SettingInput
          label="CORS Enabled"
          description="Allow cross-origin requests from other domains"
          value={settings.api.corsEnabled}
          onChange={(v) => updateSetting('api', 'corsEnabled', v)}
          type="boolean"
        />
      </SettingsSection>

      {/* Crawler Settings */}
      <SettingsSection title="Crawler Settings" icon={Bot}>
        <SettingInput
          label="User Agent"
          description="Identification string sent when crawling websites"
          value={settings.crawler.userAgent}
          onChange={(v) => updateSetting('crawler', 'userAgent', v)}
          type="text"
        />
        <SettingInput
          label="Maximum Depth"
          description="How many levels deep to crawl from the initial URL"
          value={settings.crawler.maxDepth}
          onChange={(v) => updateSetting('crawler', 'maxDepth', v)}
          type="number"
          min={0}
          max={5}
          unit="levels"
        />
        <SettingInput
          label="Crawl Delay"
          description="Delay between requests to respect server rate limits"
          value={settings.crawler.delayMs}
          onChange={(v) => updateSetting('crawler', 'delayMs', v)}
          type="number"
          min={0}
          max={5000}
          unit="ms"
        />
        <SettingInput
          label="Maximum Pages"
          description="Maximum number of pages to crawl per job"
          value={settings.crawler.maxPages}
          onChange={(v) => updateSetting('crawler', 'maxPages', v)}
          type="number"
          min={1}
          max={1000}
          unit="pages"
        />
      </SettingsSection>

      {/* Action Buttons */}
      <div className="flex justify-end gap-4 pt-4">
        <button
          onClick={handleReset}
          disabled={!hasChanges}
          className="btn-admin btn-admin-ghost flex items-center gap-2 disabled:opacity-50"
        >
          <RotateCcw className="w-4 h-4" />
          <span>Reset to Defaults</span>
        </button>
        <button
          onClick={handleSave}
          disabled={!hasChanges || saveStatus === 'saving'}
          className="btn-admin btn-admin-primary flex items-center gap-2 disabled:opacity-50"
        >
          {saveStatus === 'saving' ? (
            <Loader2 className="w-4 h-4 animate-spin" />
          ) : (
            <Save className="w-4 h-4" />
          )}
          <span>Save Configuration</span>
        </button>
      </div>
    </div>
  );
}
