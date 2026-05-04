import type { RemoteModelInfo } from '../types/apps';
import type { DownloadStatus } from '../hooks/modelDownloadState';
import { RemoteModelSummary } from './RemoteModelSummary';
import { RemoteModelListItemActions } from './RemoteModelListItemActions';
import {
  formatDownloadRetryHint,
  getRemoteDownloadFlags,
  getRemoteDownloadOptions,
  getRemoteQuantLabels,
  getSelectedRemoteTotalBytes,
  hasExactDownloadDetails,
  hasRemoteFileGroups,
} from './RemoteModelListItemState';
import { ListItem } from './ui';

interface RemoteModelListItemProps {
  model: RemoteModelInfo;
  downloadKey: string;
  downloadStatus?: DownloadStatus;
  modelError?: string;
  isHydratingDetails: boolean;
  isMenuOpen: boolean;
  selectedGroups: Set<string>;
  onToggleMenu: () => void;
  onCloseMenu: () => void;
  onToggleGroup: (label: string) => void;
  onClearSelection: () => void;
  onHydrateModelDetails?: (model: RemoteModelInfo) => Promise<void>;
  onStartDownload: (model: RemoteModelInfo, quant?: string | null, filenames?: string[] | null) => Promise<void>;
  onCancelDownload: (downloadKey: string) => Promise<void>;
  onPauseDownload: (downloadKey: string) => Promise<void>;
  onResumeDownload: (downloadKey: string) => Promise<void>;
  onOpenUrl: (url: string) => void;
  onSearchDeveloper?: (developer: string) => void;
  onHfAuthClick?: () => void;
}

export function RemoteModelListItem({
  model,
  downloadKey,
  downloadStatus,
  modelError,
  isHydratingDetails,
  isMenuOpen,
  selectedGroups,
  onToggleMenu,
  onCloseMenu,
  onToggleGroup,
  onClearSelection,
  onHydrateModelDetails,
  onStartDownload,
  onCancelDownload,
  onPauseDownload,
  onResumeDownload,
  onOpenUrl,
  onSearchDeveloper,
  onHfAuthClick,
}: RemoteModelListItemProps) {
  const flags = getRemoteDownloadFlags(downloadStatus);
  const retryHint = formatDownloadRetryHint(downloadStatus);
  const progressValue = downloadStatus?.progress ?? 0;
  const progressDegrees = Math.min(360, Math.max(0, Math.round(progressValue * 360)));
  const hasExactDetails = hasExactDownloadDetails(model);
  const downloadOptions = getRemoteDownloadOptions(model);
  const hasFileGroups = hasRemoteFileGroups(downloadOptions);
  const quantLabels = getRemoteQuantLabels(downloadOptions, hasFileGroups);
  const selectedTotalBytes = getSelectedRemoteTotalBytes(downloadOptions, selectedGroups);

  return (
    <ListItem>
      <div className="flex items-start justify-between gap-2 p-2">
        <RemoteModelSummary
          isHydratingDetails={isHydratingDetails}
          model={model}
          modelError={modelError}
          quantLabels={quantLabels}
          retryHint={retryHint}
          onHfAuthClick={onHfAuthClick}
          onSearchDeveloper={onSearchDeveloper}
        />

        <RemoteModelListItemActions
          downloadOptions={downloadOptions}
          flags={flags}
          hasExactDetails={hasExactDetails}
          hasFileGroups={hasFileGroups}
          isHydratingDetails={isHydratingDetails}
          isMenuOpen={isMenuOpen}
          model={model}
          downloadKey={downloadKey}
          progressDegrees={progressDegrees}
          selectedGroups={selectedGroups}
          selectedTotalBytes={selectedTotalBytes}
          onCancelDownload={onCancelDownload}
          onClearSelection={onClearSelection}
          onCloseMenu={onCloseMenu}
          onHydrateModelDetails={onHydrateModelDetails}
          onOpenUrl={onOpenUrl}
          onPauseDownload={onPauseDownload}
          onResumeDownload={onResumeDownload}
          onStartDownload={onStartDownload}
          onToggleGroup={onToggleGroup}
          onToggleMenu={onToggleMenu}
        />
      </div>
    </ListItem>
  );
}
