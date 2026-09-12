import { ArrowRightLeft, Download, Link2 } from 'lucide-react';
import type { ModelInfo } from '../types/apps';
import type { ServedModelStatus } from '../types/api-serving';
import { RuntimeModelServeAction } from '@runtime-model-serve-action';
import { LocalModelDownloadProgressRing } from './LocalModelDownloadProgressRing';
import { HoldToDeleteButton, IconButton } from './ui';
import type { LocalModelRowState } from './LocalModelRowState';

interface LocalModelInstalledActionsProps {
  model: ModelInfo;
  isLoading?: boolean;
  rowState: LocalModelRowState;
  selectedAppId: string | null;
  servedStatus?: ServedModelStatus | null;
  onDeleteModel?: (modelId: string) => void;
  onConvertModel?: (model: ModelInfo) => void;
  onRecoverPartialDownload?: (model: ModelInfo) => void;
  onServeModel?: (model: ModelInfo) => void;
  onToggleLink: (modelId: string) => void;
}

function getRecoveryTooltip(
  rowState: LocalModelRowState,
  canRecover: boolean
): string {
  if (!canRecover) {
    return rowState.partialError ?? 'Partial download';
  }
  return rowState.isRecoveringPartial
    ? 'Resuming partial download...'
    : `Resume partial download (${Math.round(rowState.ringDegrees / 3.6)}%)`;
}

function getRecoveryAction(
  model: ModelInfo,
  rowState: LocalModelRowState,
  onRecoverPartialDownload?: (model: ModelInfo) => void
): (() => void) | undefined {
  if (!rowState.canRecoverPartial || rowState.isRecoveringPartial || !onRecoverPartialDownload) {
    return undefined;
  }
  return () => onRecoverPartialDownload(model);
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
  isLoading = false,
  rowState,
  selectedAppId,
  servedStatus,
  onDeleteModel,
  onConvertModel,
  onRecoverPartialDownload,
  onServeModel,
  onToggleLink,
}: LocalModelInstalledActionsProps) {
  const showRetainedDownloadIndicator = rowState.hasRetainedProgressRing;
  const canRecoverPartial = rowState.canRecoverPartial && Boolean(onRecoverPartialDownload);

  return (
    <>
      {selectedAppId && (
        <IconButton
          icon={<Link2 />}
          tooltip={
            rowState.isLinked
              ? `Linked to ${selectedAppId}`
              : `Excluded from ${selectedAppId}`
          }
          onClick={() => onToggleLink(model.id)}
          disabled={rowState.isPartialDownload}
          size="sm"
          active={rowState.isLinked}
          className={rowState.isLinked ? 'text-[hsl(var(--accent-success))]' : 'opacity-40'}
        />
      )}
      {showRetainedDownloadIndicator && (
        <IconButton
          icon={<RecoverPartialDownloadIcon rowState={rowState} />}
          tooltip={getRecoveryTooltip(rowState, canRecoverPartial)}
          onClick={getRecoveryAction(model, rowState, onRecoverPartialDownload)}
          disabled={!rowState.canRecoverPartial || rowState.isRecoveringPartial}
          size="sm"
        />
      )}
      {onConvertModel && !rowState.isPartialDownload && !model.hasIntegrityIssue &&
        (model.primaryFormat === 'gguf' || model.primaryFormat === 'safetensors') && (
          <IconButton icon={<ArrowRightLeft />} tooltip="Convert model format"
            onClick={() => onConvertModel(model)} size="sm" />
        )}
      <RuntimeModelServeAction
        model={model}
        isLoading={isLoading}
        rowState={rowState}
        servedStatus={servedStatus}
        onServeModel={onServeModel}
      />
      {onDeleteModel && (
        <HoldToDeleteButton onDelete={() => onDeleteModel(model.id)} />
      )}
    </>
  );
}
