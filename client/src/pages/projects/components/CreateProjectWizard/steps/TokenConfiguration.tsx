import { useState, useCallback } from 'react';
import { githubApi } from '../../../../../api/projectClient';
import type { GitHubTokenStatus } from '../../../../../types/project';
import type { TokenSource } from '../index';

interface TokenConfigurationProps {
  tokenSource: TokenSource;
  newToken: string;
  tokenError: string | null;
  globalTokenStatus: GitHubTokenStatus | null;
  isLoadingTokenStatus: boolean;
  onTokenSourceChange: (source: TokenSource) => void;
  onTokenChange: (token: string) => void;
  onTokenError: (error: string | null) => void;
}

export default function TokenConfiguration({
  tokenSource,
  newToken,
  tokenError,
  globalTokenStatus,
  isLoadingTokenStatus,
  onTokenSourceChange,
  onTokenChange,
  onTokenError,
}: TokenConfigurationProps) {
  const [isValidating, setIsValidating] = useState(false);
  const [isTokenVisible, setIsTokenVisible] = useState(false);
  const [validationSuccess, setValidationSuccess] = useState(false);

  const hasGlobalToken = globalTokenStatus?.has_global_token ?? false;

  const validateToken = useCallback(async (token: string) => {
    if (!token || token.length < 10) {
      onTokenError('Token is too short');
      return;
    }

    // Basic format validation
    if (!token.startsWith('ghp_') && !token.startsWith('gho_') && !token.startsWith('github_pat_')) {
      onTokenError('Invalid token format. Token should start with ghp_, gho_, or github_pat_');
      return;
    }

    setIsValidating(true);
    onTokenError(null);

    try {
      // Verify the token works by checking access
      await githubApi.verifyAccess('octocat', 'hello-world', token);
      setValidationSuccess(true);
      onTokenError(null);
    } catch {
      onTokenError('Token validation failed. Please check your token and try again.');
      setValidationSuccess(false);
    } finally {
      setIsValidating(false);
    }
  }, [onTokenError]);

  const handleTokenBlur = () => {
    if (newToken && newToken.length > 0) {
      validateToken(newToken);
    }
  };

  return (
    <div className="space-y-6">
      <div className="text-center">
        <h3 className="text-lg font-medium text-white mb-2">
          GitHub Authentication
        </h3>
        <p className="text-sm text-slate-400">
          Choose how to authenticate with GitHub
        </p>
      </div>

      <div className="space-y-4 mt-8">
        {/* Global Token Option */}
        {isLoadingTokenStatus ? (
          <div className="p-4 rounded-lg border border-slate-700 bg-slate-800/50">
            <div className="flex items-center gap-3">
              <div className="w-5 h-5 border-2 border-slate-600 border-t-cyan-500 rounded-full animate-spin" />
              <span className="text-sm text-slate-400">Checking for existing token...</span>
            </div>
          </div>
        ) : hasGlobalToken ? (
          <button
            onClick={() => onTokenSourceChange('global')}
            className={`
              w-full p-4 rounded-lg border-2 text-left
              transition-all duration-200
              ${tokenSource === 'global'
                ? 'border-cyan-500 bg-cyan-500/10'
                : 'border-slate-700 bg-slate-800/50 hover:border-slate-600'
              }
            `}
          >
            <div className="flex items-start gap-4">
              <div
                className={`
                  w-5 h-5 mt-0.5 rounded-full border-2 flex-shrink-0
                  flex items-center justify-center
                  ${tokenSource === 'global'
                    ? 'border-cyan-500 bg-cyan-500'
                    : 'border-slate-600'
                  }
                `}
              >
                {tokenSource === 'global' && (
                  <div className="w-2 h-2 rounded-full bg-white" />
                )}
              </div>
              <div className="flex-1">
                <div className="flex items-center gap-3 mb-1">
                  <span className="font-medium text-white">Use Existing Global Token</span>
                  <span className="px-2 py-0.5 text-xs font-medium text-emerald-400 bg-emerald-500/10 rounded">
                    Configured
                  </span>
                </div>
                <p className="text-sm text-slate-400">
                  Use the global GitHub token configured in Administration settings.
                </p>
                {globalTokenStatus?.verified_user && (
                  <p className="text-sm text-slate-500 mt-2">
                    Connected as <span className="text-cyan-400">@{globalTokenStatus.verified_user}</span>
                    {globalTokenStatus.token_suffix && (
                      <span className="text-slate-600"> (****{globalTokenStatus.token_suffix})</span>
                    )}
                  </p>
                )}
              </div>
            </div>
          </button>
        ) : (
          <div className="p-4 rounded-lg border border-amber-500/30 bg-amber-500/10">
            <div className="flex items-start gap-3">
              <svg className="w-5 h-5 text-amber-500 mt-0.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
              <div>
                <p className="text-sm font-medium text-amber-400">No Global Token Configured</p>
                <p className="text-sm text-amber-400/70 mt-1">
                  You can configure a global token in Administration settings, or enter a project-specific token below.
                </p>
              </div>
            </div>
          </div>
        )}

        {/* New Token Option */}
        <button
          onClick={() => onTokenSourceChange('new')}
          className={`
            w-full p-4 rounded-lg border-2 text-left
            transition-all duration-200
            ${tokenSource === 'new'
              ? 'border-cyan-500 bg-cyan-500/10'
              : 'border-slate-700 bg-slate-800/50 hover:border-slate-600'
            }
          `}
        >
          <div className="flex items-start gap-4">
            <div
              className={`
                w-5 h-5 mt-0.5 rounded-full border-2 flex-shrink-0
                flex items-center justify-center
                ${tokenSource === 'new'
                  ? 'border-cyan-500 bg-cyan-500'
                  : 'border-slate-600'
                }
              `}
            >
              {tokenSource === 'new' && (
                <div className="w-2 h-2 rounded-full bg-white" />
              )}
            </div>
            <div className="flex-1">
              <span className="font-medium text-white">Enter New Token</span>
              <p className="text-sm text-slate-400 mt-1">
                Provide a project-specific GitHub Personal Access Token.
              </p>
            </div>
          </div>
        </button>

        {/* Token Input (when new token selected) */}
        {tokenSource === 'new' && (
          <div className="ml-9 space-y-3">
            <div className="relative">
              <input
                type={isTokenVisible ? 'text' : 'password'}
                value={newToken}
                onChange={(e) => {
                  onTokenChange(e.target.value);
                  setValidationSuccess(false);
                  onTokenError(null);
                }}
                onBlur={handleTokenBlur}
                placeholder="ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
                className={`
                  w-full px-4 py-3 pr-24
                  bg-slate-800 border-2 rounded-lg
                  text-white text-sm font-mono placeholder:text-slate-600
                  focus:outline-none transition-colors
                  ${tokenError
                    ? 'border-red-500/50 focus:border-red-500'
                    : validationSuccess
                      ? 'border-emerald-500/50 focus:border-emerald-500'
                      : 'border-slate-700 focus:border-cyan-500'
                  }
                `}
              />
              <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
                {isValidating && (
                  <div className="w-4 h-4 border-2 border-slate-600 border-t-cyan-500 rounded-full animate-spin" />
                )}
                {validationSuccess && !isValidating && (
                  <svg className="w-5 h-5 text-emerald-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                )}
                <button
                  type="button"
                  onClick={() => setIsTokenVisible(!isTokenVisible)}
                  className="p-1.5 text-slate-400 hover:text-white transition-colors"
                >
                  {isTokenVisible ? (
                    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21" />
                    </svg>
                  ) : (
                    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                    </svg>
                  )}
                </button>
              </div>
            </div>

            {tokenError && (
              <p className="text-sm text-red-400 flex items-center gap-2">
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                {tokenError}
              </p>
            )}

            <div className="flex flex-col gap-2">
              <a
                href="https://github.com/settings/tokens/new?description=KIX%20Project&scopes=repo,project"
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-2 text-sm text-cyan-400 hover:text-cyan-300 transition-colors"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
                </svg>
                Generate a new token on GitHub
              </a>
              <p className="text-xs text-slate-500">
                Required scopes: <code className="px-1 py-0.5 bg-slate-800 rounded text-slate-400">repo</code> and <code className="px-1 py-0.5 bg-slate-800 rounded text-slate-400">project</code> (for GitHub Projects V2)
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Scope requirements info */}
      <div className="p-3 bg-slate-800/50 rounded-lg border border-slate-700/50">
        <p className="text-xs text-slate-400">
          <span className="text-slate-300 font-medium">Required token scopes:</span>{' '}
          <code className="px-1 py-0.5 bg-slate-900 rounded text-cyan-400">repo</code> for repository access,{' '}
          <code className="px-1 py-0.5 bg-slate-900 rounded text-cyan-400">project</code> for GitHub Projects V2 boards.
        </p>
      </div>
    </div>
  );
}
