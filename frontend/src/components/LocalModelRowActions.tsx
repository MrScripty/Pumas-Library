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
  onCancelDownload?: (downloadKey: string) => void;
  onConvertModel?: (modelId: string) => void;
  onDeleteModel?: (modelId: string) => void;
  onPauseDownload?: (downloadKey: string) => void;
  onRecoverPartialDownload?: (model: ModelInfo) => void;
  onResumeDownload?: (downloadKey: string) => void;
  onServeModel?: (model: ModelInfo) => void;
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
  onServeModel,
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
          onServeModel={onServeModel}
          onToggleLink={onToggleLink}
        />
      )}
    </div>
  );
}
