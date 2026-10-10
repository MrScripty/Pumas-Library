/**
 * Remote Models List Component
 *
 * Displays HuggingFace search results with download functionality.
 * Extracted from ModelManager.tsx
 */

import { useState } from 'react';
import { Search } from 'lucide-react';
import type { RemoteModelInfo } from '../types/apps';
import type { DownloadStatus } from '../hooks/modelDownloadState';
import { RemoteModelListItem } from './RemoteModelListItem';
import { RemoteModelSummary } from './RemoteModelSummary';
import {
  getRemoteDownloadArtifactLabel,
  getRemoteDownloadOptions,
  getRemoteDownloadFlags,
} from './RemoteModelListItemState';
import { EmptyState, ListItem } from './ui';

interface RemoteModelsListProps {
  models: RemoteModelInfo[];
  isLoading: boolean;
  error: string | null;
  searchQuery: string;
  downloadStatusByRepo: Record<string, DownloadStatus>;
  downloadErrors: Record<string, string>;
  hydratingRepoIds: Set<string>;
  hydratedRepoIds?: Set<string>;
  hydrationErrors?: Record<string, string>;
  onHydrateModelDetails?: (model: RemoteModelInfo) => Promise<void>;
  onStartDownload: (model: RemoteModelInfo, quant?: string | null, filenames?: string[] | null) => Promise<void>;
  onCancelDownload: (downloadKey: string) => Promise<void>;
  onPauseDownload: (downloadKey: string) => Promise<void>;
  onResumeDownload: (downloadKey: string) => Promise<void>;
  onOpenUrl: (url: string) => void;
  onSearchDeveloper?: (developer: string) => void;
  onClearFilters?: () => void;
  selectedKind: string;
  onHfAuthClick?: () => void;
  isCachedSearch?: boolean;
}

function findDownloadForRepo(
  downloadStatusByRepo: Record<string, DownloadStatus>,
  repoId: string
): [string, DownloadStatus] | null {
  return findDownloadsForRepo(downloadStatusByRepo, repoId)[0] ?? null;
}

function findDownloadsForRepo(
  downloadStatusByRepo: Record<string, DownloadStatus>,
  repoId: string
): Array<[string, DownloadStatus]> {
  const matches = Object.entries(downloadStatusByRepo).filter(
    ([key, status]) => (status.repoId ?? key) === repoId
  );
  if (downloadStatusByRepo[repoId] && !matches.some(([key]) => key === repoId)) {
    matches.unshift([repoId, downloadStatusByRepo[repoId]]);
  }
  return matches;
}

export function RemoteModelsList({
  models,
  isLoading,
  error,
  searchQuery,
  downloadStatusByRepo,
  downloadErrors,
  hydratingRepoIds,
  hydratedRepoIds,
  hydrationErrors,
  onHydrateModelDetails,
  onStartDownload,
  onCancelDownload,
  onPauseDownload,
  onResumeDownload,
  onOpenUrl,
  onSearchDeveloper,
  onClearFilters,
  selectedKind,
  onHfAuthClick,
  isCachedSearch = false,
}: RemoteModelsListProps) {
  const [openQuantMenuRepoId, setOpenQuantMenuRepoId] = useState<string | null>(null);
  // Track selected file groups per repo for multi-select checkbox mode
  const [selectedGroups, setSelectedGroups] = useState<Record<string, Set<string>>>({});

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-xs text-[hsl(var(--text-muted))]">
        <Search className="w-3.5 h-3.5 animate-pulse" />
        <span>{isCachedSearch ? 'Searching cached details...' : 'Searching Hugging Face...'}</span>
      </div>
    );
  }

  if (error) {
    return <div className="text-xs text-[hsl(var(--accent-error))]">{error}</div>;
  }

  if (models.length === 0) {
    return (
      <EmptyState
        icon={<Search />}
        message={isCachedSearch ? 'No retained public details match this search.' : searchQuery.trim()
          ? 'No Hugging Face models match your search.'
          : 'Type to search Hugging Face models.'}
        action={(searchQuery.trim() || selectedKind !== 'all') && onClearFilters ? {
          label: 'Clear filters',
          onClick: onClearFilters,
        } : undefined}
      />
    );
  }


  return (
    <>
      {models.map((model) => {
        const repoDownloads = findDownloadsForRepo(downloadStatusByRepo, model.repoId);
        const repoDownload = findDownloadForRepo(downloadStatusByRepo, model.repoId);
        const downloadKey = repoDownload?.[0] ?? model.repoId;
        const downloadStatus = repoDownload?.[1];
        const modelError = downloadErrors[downloadKey] ?? downloadErrors[model.repoId];
        const isHydratingDetails = hydratingRepoIds.has(model.repoId);
        const repoSelected = selectedGroups[model.repoId] ?? new Set<string>();
        const downloadOptions = getRemoteDownloadOptions(model);
        const activeArtifactLabels = repoDownloads
          .map(([, status]) => getRemoteDownloadArtifactLabel(status, downloadOptions))
          .filter((label): label is string => Boolean(label));

        if (isCachedSearch) {
          const flags = getRemoteDownloadFlags(downloadStatus);
          return <ListItem key={model.repoId}><div className="p-2 space-y-2">
            <RemoteModelSummary model={model} quantLabels={model.quants} isHydratingDetails={false}
              activeArtifactLabels={[...new Set(activeArtifactLabels)]} modelError={modelError}
              onSearchDeveloper={onSearchDeveloper} />
            <p className="text-xs text-[hsl(var(--text-secondary))]">Switch to Hugging Face search for current download details.</p>
            {downloadStatus && <div className="flex gap-2 text-xs">
              {flags.isDownloading && !flags.isQueued && !flags.isPausing && <button type="button"
                onClick={() => void onPauseDownload(downloadKey)}>Pause existing download</button>}
              {(flags.isPaused || flags.isErrored) && <button type="button"
                onClick={() => void onResumeDownload(downloadKey)}>Resume existing download</button>}
              {(flags.isDownloading || flags.isPaused || flags.isErrored) && <button type="button"
                onClick={() => void onCancelDownload(downloadKey)}>Cancel existing download</button>}
            </div>}
          </div></ListItem>;
        }

        return (
          <RemoteModelListItem
            key={model.repoId}
            model={model}
            downloadKey={downloadKey}
            downloadStatus={downloadStatus}
            activeArtifactLabels={[...new Set(activeArtifactLabels)]}
            modelError={modelError}
            isHydratingDetails={isHydratingDetails}
            hasHydratedDetails={hydratedRepoIds?.has(model.repoId) ?? false}
            hydrationError={hydrationErrors?.[model.repoId]}
            isMenuOpen={openQuantMenuRepoId === model.repoId}
            selectedGroups={repoSelected}
            onToggleMenu={() =>
              setOpenQuantMenuRepoId((prev) => (prev === model.repoId ? null : model.repoId))
            }
            onCloseMenu={() => setOpenQuantMenuRepoId(null)}
            onToggleGroup={(label) => {
              setSelectedGroups((prev) => {
                const current = new Set(prev[model.repoId] ?? []);
                if (current.has(label)) current.delete(label);
                else current.add(label);
                return { ...prev, [model.repoId]: current };
              });
            }}
            onClearSelection={() => {
              setSelectedGroups((prev) => {
                const next = { ...prev };
                delete next[model.repoId];
                return next;
              });
            }}
            onHydrateModelDetails={onHydrateModelDetails}
            onStartDownload={onStartDownload}
            onCancelDownload={onCancelDownload}
            onPauseDownload={onPauseDownload}
            onResumeDownload={onResumeDownload}
            onOpenUrl={onOpenUrl}
            onSearchDeveloper={onSearchDeveloper}
            onHfAuthClick={onHfAuthClick}
          />
        );
      })}
    </>
  );
}
