import type { CSSProperties } from 'react';
import { Download, Pause } from 'lucide-react';
import type { ModelInfo } from '../types/apps';
import { HoldToDeleteButton } from './ui';
import type { LocalModelRowState } from './LocalModelRowState';

interface LocalModelDownloadActionsProps {
  model: ModelInfo;
  rowState: LocalModelRowState;
  onCancelDownload?: (repoId: string) => void;
  onPauseDownload?: (repoId: string) => void;
  onResumeDownload?: (repoId: string) => void;
}

function getDownloadActionTitle(model: ModelInfo, rowState: LocalModelRowState): string | undefined {
  if (rowState.canPause) {
    return 'Pause download';
  }
  if (rowState.isPaused) {
    return 'Resume download';
  }
  if (model.downloadStatus === 'error') {
    return 'Retry download';
  }
  return undefined;
}

function handleDownloadAction({
  model,
  rowState,
  onPauseDownload,
  onResumeDownload,
}: LocalModelDownloadActionsProps): void {
  const repoId = model.downloadRepoId;
  if (!repoId) {
    return;
  }
  if (rowState.canPause && onPauseDownload) {
    onPauseDownload(repoId);
    return;
  }
  if (rowState.canResume && onResumeDownload) {
    onResumeDownload(repoId);
  }
}

function DownloadProgressRings({ rowState }: { rowState: LocalModelRowState }) {
  if (!rowState.isActiveDownload) {
    return null;
  }

  return (
    <>
      <span
        className={`download-progress-ring ${rowState.isQueued ? 'is-waiting' : ''} ${rowState.isPaused ? 'is-paused' : ''}`}
        style={{ '--progress': `${rowState.ringDegrees}deg` } as CSSProperties}
      />
      {!rowState.isQueued && !rowState.isPaused && <span className="download-scan-ring" />}
    </>
  );
}

function DownloadActionIcon({ canPause }: { canPause: boolean }) {
  if (canPause) {
    return (
      <>
        <Download className="h-3.5 w-3.5 group-hover:hidden" />
        <Pause className="h-3.5 w-3.5 hidden group-hover:block" />
      </>
    );
  }

  return <Download className="h-3.5 w-3.5" />;
}

export function LocalModelDownloadActions({
  model,
  rowState,
  onCancelDownload,
  onPauseDownload,
  onResumeDownload,
}: LocalModelDownloadActionsProps) {
  return (
    <>
      <button
        className={`relative flex h-6 w-6 items-center justify-center rounded-md border-0 bg-transparent ${
          rowState.canResume || rowState.canPause ? 'cursor-pointer' : 'cursor-default'
        } ${rowState.canResume ? 'download-resume-btn' : ''} text-[hsl(var(--text-muted))]`}
        title={getDownloadActionTitle(model, rowState)}
        onClick={() => handleDownloadAction({ model, rowState, onPauseDownload, onResumeDownload })}
      >
        <DownloadProgressRings rowState={rowState} />
        <DownloadActionIcon canPause={rowState.canPause} />
      </button>
      {onCancelDownload && model.downloadRepoId && (
        <HoldToDeleteButton
          onDelete={() => onCancelDownload(model.downloadRepoId as string)}
          tooltip="Hold to remove download"
        />
      )}
    </>
  );
}
