import { ArrowRightLeft, Download, Link2 } from 'lucide-react';
import type { ModelInfo } from '../types/apps';
import { LocalModelDownloadProgressRing } from './LocalModelDownloadProgressRing';
import { HoldToDeleteButton, IconButton } from './ui';
import type { LocalModelRowState } from './LocalModelRowState';

interface LocalModelInstalledActionsProps {
  model: ModelInfo;
  rowState: LocalModelRowState;
  selectedAppId: string | null;
  onConvertModel?: (modelId: string) => void;
  onDeleteModel?: (modelId: string) => void;
  onRecoverPartialDownload?: (model: ModelInfo) => void;
  onToggleLink: (modelId: string) => void;
}

function getConvertTooltip(model: ModelInfo): string {
  if (model.primaryFormat === 'safetensors') {
    return 'Convert / Quantize';
  }
  return 'Convert / Re-quantize';
}

function RecoverPartialDownloadIcon({ rowState }: { rowState: LocalModelRowState }) {
  return (
    <>
      {rowState.hasRetainedProgressRing && (
        <LocalModelDownloadProgressRing isRetained={true} ringDegrees={rowState.ringDegrees} />
      )}
      <Download />
    </>
  );
}

export function LocalModelInstalledActions({
  model,
  rowState,
  selectedAppId,
  onConvertModel,
  onDeleteModel,
  onRecoverPartialDownload,
  onToggleLink,
}: LocalModelInstalledActionsProps) {
  const showRetainedDownloadIndicator = rowState.hasRetainedProgressRing;

  return (
    <>
      <IconButton
        icon={<Link2 />}
        tooltip={
          rowState.isLinked
            ? `Linked to ${selectedAppId || 'app'}`
            : `Excluded from ${selectedAppId || 'app'}`
        }
        onClick={() => onToggleLink(model.id)}
        disabled={rowState.isPartialDownload}
        size="sm"
        active={rowState.isLinked}
        className={rowState.isLinked ? 'text-[hsl(var(--accent-success))]' : 'opacity-40'}
      />
      {showRetainedDownloadIndicator && (
        <IconButton
          icon={<RecoverPartialDownloadIcon rowState={rowState} />}
          tooltip={
            rowState.canRecoverPartial && onRecoverPartialDownload
              ? rowState.isRecoveringPartial
                ? 'Resuming partial download...'
                : 'Resume partial download'
              : 'Partial download'
          }
          onClick={
            rowState.canRecoverPartial && onRecoverPartialDownload && !rowState.isRecoveringPartial
              ? () => onRecoverPartialDownload(model)
              : undefined
          }
          disabled={!rowState.canRecoverPartial || rowState.isRecoveringPartial}
          size="sm"
        />
      )}
      {rowState.isConvertible && onConvertModel && (
        <IconButton
          icon={<ArrowRightLeft />}
          tooltip={getConvertTooltip(model)}
          onClick={() => onConvertModel(model.id)}
          size="sm"
        />
      )}
      {onDeleteModel && (
        <HoldToDeleteButton onDelete={() => onDeleteModel(model.id)} />
      )}
    </>
  );
}
