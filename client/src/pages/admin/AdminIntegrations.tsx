import { useState, useEffect } from 'react';
import clsx from 'clsx';
import {
  Plug,
  CheckCircle,
  AlertCircle,
  Loader2,
  ExternalLink,
  Eye,
  EyeOff,
  RefreshCw,
  Trash2,
} from 'lucide-react';
import { githubApi } from '../../api/projectClient';
import type { GitHubTokenStatus } from '../../types/project';

interface IntegrationSectionProps {
  title: string;
  description: string;
  icon: React.ReactNode;
  children: React.ReactNode;
  status?: 'connected' | 'disconnected' | 'error';
}

function IntegrationSection({
  title,
  description,
  icon,
  children,
  status,
}: IntegrationSectionProps) {
  return (
    <div className="admin-card admin-card-glow overflow-hidden">
      <div className="relative z-10">
        {/* Header */}
        <div className="p-6 border-b border-slate-700/50">
          <div className="flex items-start justify-between gap-4">
            <div className="flex items-center gap-4">
              <div className="w-12 h-12 rounded-xl bg-slate-800 border border-slate-700 flex items-center justify-center">
                {icon}
              </div>
              <div>
                <div className="flex items-center gap-3">
                  <h3 className="text-lg font-semibold text-white">{title}</h3>
                  {status && (
                    <span
                      className={clsx(
                        'px-2.5 py-0.5 text-xs font-medium rounded-full',
                        status === 'connected' && 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20',
                        status === 'disconnected' && 'bg-slate-500/10 text-slate-400 border border-slate-500/20',
                        status === 'error' && 'bg-red-500/10 text-red-400 border border-red-500/20'
                      )}
                    >
                      {status === 'connected' && 'Connected'}
                      {status === 'disconnected' && 'Not Configured'}
                      {status === 'error' && 'Error'}
                    </span>
                  )}
                </div>
                <p className="text-sm text-slate-400 mt-1">{description}</p>
              </div>
            </div>
          </div>
        </div>

        {/* Content */}
        <div className="p-6">{children}</div>
      </div>
    </div>
  );
}

function GitHubIntegration() {
  const [tokenStatus, setTokenStatus] = useState<GitHubTokenStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Token form state
  const [newToken, setNewToken] = useState('');
  const [isTokenVisible, setIsTokenVisible] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [saveResult, setSaveResult] = useState<{ success: boolean; message: string } | null>(null);

  // Load token status on mount
  useEffect(() => {
    loadTokenStatus();
  }, []);

  const loadTokenStatus = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const status = await githubApi.getTokenStatus();
      setTokenStatus(status);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load token status');
    } finally {
      setIsLoading(false);
    }
  };

  const handleSaveToken = async () => {
    if (!newToken.trim()) return;

    // Validate token format
    if (!newToken.startsWith('ghp_') && !newToken.startsWith('gho_') && !newToken.startsWith('github_pat_')) {
      setSaveResult({
        success: false,
        message: 'Invalid token format. Token should start with ghp_, gho_, or github_pat_',
      });
      return;
    }

    setIsSaving(true);
    setSaveResult(null);

    try {
      const result = await githubApi.setGlobalToken(newToken);
      setSaveResult({
        success: true,
        message: `Successfully connected as @${result.username}`,
      });
      setNewToken('');
      // Reload token status
      await loadTokenStatus();
    } catch (err) {
      setSaveResult({
        success: false,
        message: err instanceof Error ? err.message : 'Failed to save token',
      });
    } finally {
      setIsSaving(false);
    }
  };

  const handleRemoveToken = async () => {
    if (!confirm('Are you sure you want to remove the global GitHub token? Projects using this token will no longer be able to sync with GitHub.')) {
      return;
    }

    setIsSaving(true);
    try {
      // Note: You'll need to add a delete endpoint for this
      // For now, just show a placeholder
      setSaveResult({
        success: false,
        message: 'Token removal not yet implemented',
      });
    } finally {
      setIsSaving(false);
    }
  };

  const getConnectionStatus = (): 'connected' | 'disconnected' | 'error' => {
    if (error) return 'error';
    if (tokenStatus?.has_global_token) return 'connected';
    return 'disconnected';
  };

  return (
    <IntegrationSection
      title="GitHub"
      description="Connect to GitHub for issue tracking and project sync"
      icon={
        <svg className="w-6 h-6 text-white" fill="currentColor" viewBox="0 0 24 24">
          <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" />
        </svg>
      }
      status={getConnectionStatus()}
    >
      {isLoading ? (
        <div className="flex items-center justify-center py-8">
          <Loader2 className="w-6 h-6 text-cyan-400 animate-spin" />
        </div>
      ) : error ? (
        <div className="p-4 bg-red-500/10 border border-red-500/30 rounded-lg text-red-400">
          <div className="flex items-center gap-3">
            <AlertCircle className="w-5 h-5 flex-shrink-0" />
            <div>
              <p className="font-medium">Failed to load GitHub integration status</p>
              <p className="text-sm mt-1 opacity-80">{error}</p>
            </div>
          </div>
          <button
            onClick={loadTokenStatus}
            className="mt-4 flex items-center gap-2 text-sm text-red-400 hover:text-red-300"
          >
            <RefreshCw className="w-4 h-4" />
            Try Again
          </button>
        </div>
      ) : tokenStatus?.has_global_token ? (
        // Token is configured - show status
        <div className="space-y-6">
          {/* Connected Status */}
          <div className="p-4 bg-emerald-500/10 border border-emerald-500/30 rounded-lg">
            <div className="flex items-start justify-between">
              <div className="flex items-center gap-3">
                <CheckCircle className="w-5 h-5 text-emerald-400" />
                <div>
                  <p className="font-medium text-emerald-400">GitHub Connected</p>
                  {tokenStatus.verified_user && (
                    <p className="text-sm text-emerald-400/80 mt-1">
                      Authenticated as <span className="font-medium">@{tokenStatus.verified_user}</span>
                      {tokenStatus.token_suffix && (
                        <span className="text-emerald-400/60"> (****{tokenStatus.token_suffix})</span>
                      )}
                    </p>
                  )}
                </div>
              </div>
              <button
                onClick={handleRemoveToken}
                disabled={isSaving}
                className="p-2 text-slate-400 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
                title="Remove token"
              >
                <Trash2 className="w-4 h-4" />
              </button>
            </div>
          </div>

          {/* Token Info */}
          <div className="grid grid-cols-2 gap-4">
            <div className="p-4 bg-slate-800/50 rounded-lg border border-slate-700/50">
              <p className="text-xs text-slate-500 uppercase tracking-wide mb-1">Token Format</p>
              <p className="text-sm text-white font-mono">
                {tokenStatus.token_suffix ? `****${tokenStatus.token_suffix}` : 'Configured'}
              </p>
            </div>
            <div className="p-4 bg-slate-800/50 rounded-lg border border-slate-700/50">
              <p className="text-xs text-slate-500 uppercase tracking-wide mb-1">Scope</p>
              <p className="text-sm text-white">Global (all projects)</p>
            </div>
          </div>

          {/* Update Token Section */}
          <div className="pt-4 border-t border-slate-700/50">
            <p className="text-sm text-slate-400 mb-4">
              Update your GitHub Personal Access Token if it has expired or needs to be rotated.
            </p>
            <div className="flex gap-3">
              <div className="relative flex-1">
                <input
                  type={isTokenVisible ? 'text' : 'password'}
                  value={newToken}
                  onChange={(e) => {
                    setNewToken(e.target.value);
                    setSaveResult(null);
                  }}
                  placeholder="ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
                  className="admin-input w-full pr-10 font-mono text-sm"
                />
                <button
                  type="button"
                  onClick={() => setIsTokenVisible(!isTokenVisible)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-white"
                >
                  {isTokenVisible ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                </button>
              </div>
              <button
                onClick={handleSaveToken}
                disabled={!newToken.trim() || isSaving}
                className="btn-admin btn-admin-primary flex items-center gap-2 disabled:opacity-50"
              >
                {isSaving ? <Loader2 className="w-4 h-4 animate-spin" /> : <RefreshCw className="w-4 h-4" />}
                Update Token
              </button>
            </div>
          </div>
        </div>
      ) : (
        // No token configured - show setup form
        <div className="space-y-6">
          {/* Instructions */}
          <div className="p-4 bg-slate-800/50 border border-slate-700/50 rounded-lg">
            <h4 className="text-sm font-medium text-white mb-2">Set up GitHub Integration</h4>
            <ol className="text-sm text-slate-400 space-y-2 list-decimal list-inside">
              <li>Create a GitHub Personal Access Token with <code className="px-1.5 py-0.5 bg-slate-900 rounded text-cyan-400">repo</code> scope</li>
              <li>Paste the token below to connect KIX to GitHub</li>
              <li>The token will be encrypted and stored securely</li>
            </ol>
            <a
              href="https://github.com/settings/tokens/new?description=KIX%20Global%20Token&scopes=repo"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 mt-4 text-sm text-cyan-400 hover:text-cyan-300"
            >
              <ExternalLink className="w-4 h-4" />
              Generate a new token on GitHub
            </a>
          </div>

          {/* Token Input */}
          <div>
            <label className="block text-sm font-medium text-slate-300 mb-2">
              GitHub Personal Access Token
            </label>
            <div className="flex gap-3">
              <div className="relative flex-1">
                <input
                  type={isTokenVisible ? 'text' : 'password'}
                  value={newToken}
                  onChange={(e) => {
                    setNewToken(e.target.value);
                    setSaveResult(null);
                  }}
                  placeholder="ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
                  className="admin-input w-full pr-10 font-mono text-sm"
                />
                <button
                  type="button"
                  onClick={() => setIsTokenVisible(!isTokenVisible)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-white"
                >
                  {isTokenVisible ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                </button>
              </div>
              <button
                onClick={handleSaveToken}
                disabled={!newToken.trim() || isSaving}
                className="btn-admin btn-admin-primary flex items-center gap-2 disabled:opacity-50"
              >
                {isSaving ? <Loader2 className="w-4 h-4 animate-spin" /> : <CheckCircle className="w-4 h-4" />}
                Connect
              </button>
            </div>
          </div>

          {/* Result Message */}
          {saveResult && (
            <div
              className={clsx(
                'p-3 rounded-lg text-sm',
                saveResult.success
                  ? 'bg-emerald-500/10 border border-emerald-500/30 text-emerald-400'
                  : 'bg-red-500/10 border border-red-500/30 text-red-400'
              )}
            >
              <div className="flex items-center gap-2">
                {saveResult.success ? (
                  <CheckCircle className="w-4 h-4" />
                ) : (
                  <AlertCircle className="w-4 h-4" />
                )}
                {saveResult.message}
              </div>
            </div>
          )}
        </div>
      )}

      {/* Result Message for update */}
      {tokenStatus?.has_global_token && saveResult && (
        <div
          className={clsx(
            'mt-4 p-3 rounded-lg text-sm',
            saveResult.success
              ? 'bg-emerald-500/10 border border-emerald-500/30 text-emerald-400'
              : 'bg-red-500/10 border border-red-500/30 text-red-400'
          )}
        >
          <div className="flex items-center gap-2">
            {saveResult.success ? (
              <CheckCircle className="w-4 h-4" />
            ) : (
              <AlertCircle className="w-4 h-4" />
            )}
            {saveResult.message}
          </div>
        </div>
      )}
    </IntegrationSection>
  );
}

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

      {/* Integrations List */}
      <div className="space-y-6">
        <GitHubIntegration />
      </div>

      {/* Future Integrations Placeholder */}
      <div className="p-6 border-2 border-dashed border-slate-700/50 rounded-xl">
        <div className="text-center">
          <div className="w-12 h-12 mx-auto rounded-xl bg-slate-800 border border-slate-700 flex items-center justify-center mb-4">
            <Plug className="w-6 h-6 text-slate-500" />
          </div>
          <h3 className="text-lg font-medium text-slate-400 mb-2">More integrations coming soon</h3>
          <p className="text-sm text-slate-500 max-w-md mx-auto">
            We're working on integrations with Linear, Jira, GitLab, and other platforms.
          </p>
        </div>
      </div>
    </div>
  );
}
