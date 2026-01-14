import { useState, useEffect, useCallback, useRef } from 'react';
import { githubApi } from '../../../../../api/projectClient';
import type { GitHubOrg, GitHubRepo } from '../../../../../types/project';

interface RepositorySelectionProps {
  selectedOrg: string | null;
  selectedRepo: string | null;
  token?: string; // Optional token for API calls (uses global if not provided)
  onOrgChange: (org: string | null) => void;
  onRepoChange: (repo: string | null, repoName?: string) => void;
}

export default function RepositorySelection({
  selectedOrg,
  selectedRepo,
  token,
  onOrgChange,
  onRepoChange,
}: RepositorySelectionProps) {
  // Organization state
  const [orgs, setOrgs] = useState<GitHubOrg[]>([]);
  const [isLoadingOrgs, setIsLoadingOrgs] = useState(true);
  const [orgError, setOrgError] = useState<string | null>(null);
  const [orgSearch, setOrgSearch] = useState('');
  const [isOrgDropdownOpen, setIsOrgDropdownOpen] = useState(false);

  // Repository state
  const [repos, setRepos] = useState<GitHubRepo[]>([]);
  const [isLoadingRepos, setIsLoadingRepos] = useState(false);
  const [repoError, setRepoError] = useState<string | null>(null);
  const [repoSearch, setRepoSearch] = useState('');
  const [isRepoDropdownOpen, setIsRepoDropdownOpen] = useState(false);
  const [hasMoreRepos, setHasMoreRepos] = useState(false);
  const [repoPage, setRepoPage] = useState(1);
  const [isLoadingMoreRepos, setIsLoadingMoreRepos] = useState(false);

  // Refs for click outside handling
  const orgDropdownRef = useRef<HTMLDivElement>(null);
  const repoDropdownRef = useRef<HTMLDivElement>(null);
  const repoListRef = useRef<HTMLDivElement>(null);
  const searchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Load organizations on mount
  useEffect(() => {
    const loadOrgs = async () => {
      setIsLoadingOrgs(true);
      setOrgError(null);
      try {
        const response = await githubApi.listOrgs(token);
        setOrgs(response.orgs);
      } catch (error) {
        setOrgError(error instanceof Error ? error.message : 'Failed to load organizations');
      } finally {
        setIsLoadingOrgs(false);
      }
    };
    loadOrgs();
  }, [token]);

  // Load repositories when org changes
  useEffect(() => {
    if (!selectedOrg) {
      setRepos([]);
      return;
    }

    const loadRepos = async () => {
      setIsLoadingRepos(true);
      setRepoError(null);
      setRepoPage(1);
      try {
        const response = await githubApi.listRepos(selectedOrg, { per_page: 50, token });
        setRepos(response.repos);
        setHasMoreRepos(response.has_more);
      } catch (error) {
        setRepoError(error instanceof Error ? error.message : 'Failed to load repositories');
      } finally {
        setIsLoadingRepos(false);
      }
    };
    loadRepos();
  }, [selectedOrg, token]);

  // Handle click outside to close dropdowns
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (orgDropdownRef.current && !orgDropdownRef.current.contains(event.target as Node)) {
        setIsOrgDropdownOpen(false);
      }
      if (repoDropdownRef.current && !repoDropdownRef.current.contains(event.target as Node)) {
        setIsRepoDropdownOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  // Debounced repo search
  const handleRepoSearch = useCallback((search: string) => {
    setRepoSearch(search);

    if (searchTimeoutRef.current) {
      clearTimeout(searchTimeoutRef.current);
    }

    if (!selectedOrg) return;

    searchTimeoutRef.current = setTimeout(async () => {
      setIsLoadingRepos(true);
      setRepoPage(1);
      try {
        const response = await githubApi.listRepos(selectedOrg, {
          search: search || undefined,
          per_page: 50,
          token,
        });
        setRepos(response.repos);
        setHasMoreRepos(response.has_more);
      } catch (error) {
        setRepoError(error instanceof Error ? error.message : 'Failed to search repositories');
      } finally {
        setIsLoadingRepos(false);
      }
    }, 300);
  }, [selectedOrg, token]);

  // Load more repos (infinite scroll)
  const loadMoreRepos = useCallback(async () => {
    if (!selectedOrg || isLoadingMoreRepos || !hasMoreRepos) return;

    setIsLoadingMoreRepos(true);
    try {
      const nextPage = repoPage + 1;
      const response = await githubApi.listRepos(selectedOrg, {
        search: repoSearch || undefined,
        page: nextPage,
        per_page: 50,
        token,
      });
      setRepos(prev => [...prev, ...response.repos]);
      setHasMoreRepos(response.has_more);
      setRepoPage(nextPage);
    } catch (error) {
      console.error('Failed to load more repos:', error);
    } finally {
      setIsLoadingMoreRepos(false);
    }
  }, [selectedOrg, repoPage, repoSearch, isLoadingMoreRepos, hasMoreRepos, token]);

  // Infinite scroll handler
  const handleRepoScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const target = e.target as HTMLDivElement;
    const nearBottom = target.scrollHeight - target.scrollTop - target.clientHeight < 100;
    if (nearBottom && hasMoreRepos && !isLoadingMoreRepos) {
      loadMoreRepos();
    }
  }, [hasMoreRepos, isLoadingMoreRepos, loadMoreRepos]);

  // Filter orgs by search
  const filteredOrgs = orgs.filter(org =>
    org.login.toLowerCase().includes(orgSearch.toLowerCase())
  );

  // Get selected org details
  const selectedOrgDetails = orgs.find(o => o.login === selectedOrg);
  const selectedRepoDetails = repos.find(r => r.name === selectedRepo);

  return (
    <div className="space-y-6">
      <div className="text-center">
        <h3 className="text-lg font-medium text-white mb-2">
          Select Repository
        </h3>
        <p className="text-sm text-slate-400">
          Choose the GitHub repository to connect
        </p>
      </div>

      <div className="space-y-5 mt-8">
        {/* Organization Dropdown */}
        <div ref={orgDropdownRef} className="relative">
          <label className="block text-sm font-medium text-slate-300 mb-2">
            Organization / User
          </label>
          <button
            type="button"
            onClick={() => setIsOrgDropdownOpen(!isOrgDropdownOpen)}
            disabled={isLoadingOrgs}
            className={`
              w-full px-4 py-3 flex items-center justify-between
              bg-slate-800 border-2 rounded-lg text-left
              transition-colors duration-200
              ${isOrgDropdownOpen ? 'border-cyan-500' : 'border-slate-700 hover:border-slate-600'}
              ${isLoadingOrgs ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
            `}
          >
            {isLoadingOrgs ? (
              <div className="flex items-center gap-3">
                <div className="w-5 h-5 border-2 border-slate-600 border-t-cyan-500 rounded-full animate-spin" />
                <span className="text-slate-400">Loading organizations...</span>
              </div>
            ) : selectedOrgDetails ? (
              <div className="flex items-center gap-3">
                <img
                  src={selectedOrgDetails.avatar_url}
                  alt={selectedOrgDetails.login}
                  className="w-6 h-6 rounded-full"
                />
                <span className="text-white font-medium">{selectedOrgDetails.login}</span>
                {selectedOrgDetails.type === 'user' && (
                  <span className="px-2 py-0.5 text-xs text-slate-400 bg-slate-700 rounded">
                    Personal
                  </span>
                )}
              </div>
            ) : (
              <span className="text-slate-400">Select organization or user</span>
            )}
            <svg
              className={`w-5 h-5 text-slate-400 transition-transform ${isOrgDropdownOpen ? 'rotate-180' : ''}`}
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </button>

          {/* Org Dropdown Menu */}
          {isOrgDropdownOpen && !isLoadingOrgs && (
            <div className="absolute z-[100] w-full mt-2 bg-slate-800 border border-slate-700 rounded-lg shadow-xl overflow-hidden">
              {/* Search input */}
              <div className="p-3 border-b border-slate-700">
                <div className="relative">
                  <svg className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                  </svg>
                  <input
                    type="text"
                    value={orgSearch}
                    onChange={(e) => setOrgSearch(e.target.value)}
                    placeholder="Search organizations..."
                    className="w-full pl-10 pr-4 py-2 bg-slate-900 border border-slate-700 rounded-md text-white text-sm placeholder:text-slate-500 focus:outline-none focus:border-cyan-500"
                    autoFocus
                  />
                </div>
              </div>

              {/* Org list */}
              <div className="max-h-64 overflow-y-auto">
                {orgError ? (
                  <div className="p-4 text-center text-red-400 text-sm">{orgError}</div>
                ) : filteredOrgs.length === 0 ? (
                  <div className="p-4 text-center text-slate-400 text-sm">No organizations found</div>
                ) : (
                  filteredOrgs.map((org) => (
                    <button
                      key={org.login}
                      onClick={() => {
                        onOrgChange(org.login);
                        setIsOrgDropdownOpen(false);
                        setOrgSearch('');
                        onRepoChange(null);
                        setRepoSearch('');
                      }}
                      className={`
                        w-full px-4 py-3 flex items-center gap-3 text-left
                        transition-colors duration-150
                        ${selectedOrg === org.login
                          ? 'bg-cyan-500/10 border-l-2 border-cyan-500'
                          : 'hover:bg-slate-700/50 border-l-2 border-transparent'
                        }
                      `}
                    >
                      <img
                        src={org.avatar_url}
                        alt={org.login}
                        className="w-8 h-8 rounded-full"
                      />
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="text-white font-medium truncate">{org.login}</span>
                          {org.type === 'user' && (
                            <span className="px-2 py-0.5 text-xs text-slate-400 bg-slate-700 rounded flex-shrink-0">
                              Personal
                            </span>
                          )}
                        </div>
                        {org.description && (
                          <p className="text-xs text-slate-400 truncate mt-0.5">{org.description}</p>
                        )}
                      </div>
                    </button>
                  ))
                )}
              </div>
            </div>
          )}
        </div>

        {/* Repository Dropdown */}
        <div ref={repoDropdownRef} className="relative">
          <label className="block text-sm font-medium text-slate-300 mb-2">
            Repository
          </label>
          <button
            type="button"
            onClick={() => selectedOrg && setIsRepoDropdownOpen(!isRepoDropdownOpen)}
            disabled={!selectedOrg || isLoadingRepos}
            className={`
              w-full px-4 py-3 flex items-center justify-between
              bg-slate-800 border-2 rounded-lg text-left
              transition-colors duration-200
              ${isRepoDropdownOpen ? 'border-cyan-500' : 'border-slate-700 hover:border-slate-600'}
              ${!selectedOrg || isLoadingRepos ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
            `}
          >
            {isLoadingRepos ? (
              <div className="flex items-center gap-3">
                <div className="w-5 h-5 border-2 border-slate-600 border-t-cyan-500 rounded-full animate-spin" />
                <span className="text-slate-400">Loading repositories...</span>
              </div>
            ) : selectedRepoDetails ? (
              <div className="flex items-center gap-3 flex-1 min-w-0">
                <svg className="w-5 h-5 text-slate-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                </svg>
                <span className="text-white font-medium truncate">{selectedRepoDetails.name}</span>
                {selectedRepoDetails.private && (
                  <span className="px-2 py-0.5 text-xs text-amber-400 bg-amber-500/10 rounded flex-shrink-0">
                    Private
                  </span>
                )}
              </div>
            ) : !selectedOrg ? (
              <span className="text-slate-500">Select an organization first</span>
            ) : (
              <span className="text-slate-400">Select repository</span>
            )}
            <svg
              className={`w-5 h-5 text-slate-400 transition-transform flex-shrink-0 ${isRepoDropdownOpen ? 'rotate-180' : ''}`}
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </button>

          {/* Repo Dropdown Menu */}
          {isRepoDropdownOpen && selectedOrg && !isLoadingRepos && (
            <div className="absolute z-[100] w-full mt-2 bg-slate-800 border border-slate-700 rounded-lg shadow-xl overflow-hidden">
              {/* Search input */}
              <div className="p-3 border-b border-slate-700">
                <div className="relative">
                  <svg className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                  </svg>
                  <input
                    type="text"
                    value={repoSearch}
                    onChange={(e) => handleRepoSearch(e.target.value)}
                    placeholder="Search repositories..."
                    className="w-full pl-10 pr-4 py-2 bg-slate-900 border border-slate-700 rounded-md text-white text-sm placeholder:text-slate-500 focus:outline-none focus:border-cyan-500"
                    autoFocus
                  />
                </div>
              </div>

              {/* Repo list */}
              <div
                ref={repoListRef}
                onScroll={handleRepoScroll}
                className="max-h-64 overflow-y-auto"
              >
                {repoError ? (
                  <div className="p-4 text-center text-red-400 text-sm">{repoError}</div>
                ) : repos.length === 0 ? (
                  <div className="p-4 text-center text-slate-400 text-sm">No repositories found</div>
                ) : (
                  <>
                    {repos.map((repo) => (
                      <button
                        key={repo.full_name}
                        onClick={() => {
                          onRepoChange(repo.name, repo.name);
                          setIsRepoDropdownOpen(false);
                        }}
                        className={`
                          w-full px-4 py-3 flex items-start gap-3 text-left
                          transition-colors duration-150
                          ${selectedRepo === repo.name
                            ? 'bg-cyan-500/10 border-l-2 border-cyan-500'
                            : 'hover:bg-slate-700/50 border-l-2 border-transparent'
                          }
                        `}
                      >
                        <svg className="w-5 h-5 text-slate-400 mt-0.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                        </svg>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2">
                            <span className="text-white font-medium truncate">{repo.name}</span>
                            {repo.private && (
                              <span className="px-2 py-0.5 text-xs text-amber-400 bg-amber-500/10 rounded flex-shrink-0">
                                Private
                              </span>
                            )}
                          </div>
                          {repo.description && (
                            <p className="text-xs text-slate-400 truncate mt-0.5">{repo.description}</p>
                          )}
                        </div>
                      </button>
                    ))}
                    {isLoadingMoreRepos && (
                      <div className="p-4 flex items-center justify-center">
                        <div className="w-5 h-5 border-2 border-slate-600 border-t-cyan-500 rounded-full animate-spin" />
                      </div>
                    )}
                  </>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Selected Summary */}
        {selectedOrg && selectedRepo && (
          <div className="p-4 bg-slate-800/50 border border-slate-700 rounded-lg">
            <p className="text-sm text-slate-400 mb-1">Selected repository:</p>
            <p className="text-white font-mono text-sm">
              {selectedOrg}/{selectedRepo}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
