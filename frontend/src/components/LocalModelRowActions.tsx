import { ChevronDown, ChevronRight } from 'lucide-react';
import type { ModelInfo } from '../types/apps';
import { IconButton } from './ui';
import { LocalModelDownloadActions } from './LocalModelDownloadActions';
import { LocalModelInstalledActions } from './LocalModelInstalledActions';
import type { LocalModelRowState } from './LocalModelRowState';

interface LocalModelRowActionsProps {
  model: ModelInfo;
  rowState: LocalModelRowState;
  selectedAppId: string | null;
  onCancelDownload?: (repoId: string) => void;
  onConvertModel?: (modelId: string) => void;
  onDeleteModel?: (modelId: string) => void;
  onPauseDownload?: (repoId: string) => void;
  onRecoverPartialDownload?: (model: ModelInfo) => void;
  onResumeDownload?: (repoId: string) => void;
  onToggleLink: (modelId: string) => void;
  onToggleRelated: (modelId: string) => void;
}

export function LocalModelRowActions({
  model,
  rowState,
  selectedAppId,
  onCancelDownload,
  onConvertModel,
  onDeleteModel,
  onPauseDownload,
  onRecoverPartialDownload,
  onResumeDownload,
  onToggleLink,
  onToggleRelated,
}: LocalModelRowActionsProps) {
  return (
    <div className="flex items-center gap-1">
      {rowState.canShowRelated && (
        <IconButton
          icon={rowState.isExpanded ? <ChevronDown /> : <ChevronRight />}
          tooltip={rowState.isExpanded ? 'Hide related' : 'Show related'}
          onClick={() => onToggleRelated(model.id)}
          size="sm"
          active={rowState.isExpanded}
        />
      )}
      {rowState.isDownloading ? (
        <LocalModelDownloadActions
          model={model}
          rowState={rowState}
          onCancelDownload={onCancelDownload}
          onPauseDownload={onPauseDownload}
          onResumeDownload={onResumeDownload}
        />
      ) : (
        <LocalModelInstalledActions
          model={model}
          rowState={rowState}
          selectedAppId={selectedAppId}
          onConvertModel={onConvertModel}
          onDeleteModel={onDeleteModel}
          onRecoverPartialDownload={onRecoverPartialDownload}
          onToggleLink={onToggleLink}
        />
      )}
    </div>
  );
}
