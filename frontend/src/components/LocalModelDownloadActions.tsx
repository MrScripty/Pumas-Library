import { useId } from 'react';
import { Download, Pause } from 'lucide-react';
import type { ModelInfo } from '../types/apps';
import { LocalModelDownloadProgressRing } from './LocalModelDownloadProgressRing';
import { HoldToDeleteButton } from './ui';
import type { LocalModelRowState } from './LocalModelRowState';

interface LocalModelDownloadActionsProps {
  model: ModelInfo;
  rowState: LocalModelRowState;
  onCancelDownload?: (downloadKey: string) => void;
  onPauseDownload?: (downloadKey: string) => void;
  onResumeDownload?: (downloadKey: string) => void;
}

/** Text equivalent of the progress ring for assistive technology. */
function getDownloadProgressDescription(model: ModelInfo, rowState: LocalModelRowState): string {
  const percent = Math.round(rowState.ringDegrees / 3.6);
  if (rowState.isQueued) return 'Download queued';
  if (model.downloadStatus === 'error') return `Download failed at ${percent}%`;
  if (rowState.isPaused) return `Download paused at ${percent}%`;
  return `Download ${percent}% complete`;
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
  const downloadKey = model.downloadKey ?? model.downloadRepoId;
  if (!downloadKey) {
    return;
  }
  if (rowState.canPause && onPauseDownload) {
    onPauseDownload(downloadKey);
    return;
  }
  if (rowState.canResume && onResumeDownload) {
    onResumeDownload(downloadKey);
  }
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
  const showProgressRing = rowState.isActiveDownload || rowState.isPaused || model.downloadStatus === 'error';
  const progressDescriptionId = useId();

  return (
    <>
      <button
        aria-describedby={showProgressRing ? progressDescriptionId : undefined}
        className={`relative flex h-6 w-6 items-center justify-center rounded-md border-0 bg-transparent ${
          rowState.canResume || rowState.canPause ? 'cursor-pointer' : 'cursor-default'
        } ${rowState.canResume ? 'download-resume-btn' : ''} text-[hsl(var(--text-muted))]`}
        title={getDownloadActionTitle(model, rowState)}
        onClick={() => handleDownloadAction({ model, rowState, onPauseDownload, onResumeDownload })}
      >
        {showProgressRing && (
          <LocalModelDownloadProgressRing
            isPaused={rowState.isPaused || model.downloadStatus === 'error'}
            isQueued={rowState.isQueued}
            ringDegrees={rowState.ringDegrees}
          />
        )}
        <DownloadActionIcon canPause={rowState.canPause} />
      </button>
      {showProgressRing && (
        <span id={progressDescriptionId} className="sr-only">
          {getDownloadProgressDescription(model, rowState)}
        </span>
      )}
      {onCancelDownload && (model.downloadKey ?? model.downloadRepoId) && (
        <HoldToDeleteButton
          onDelete={() => onCancelDownload((model.downloadKey ?? model.downloadRepoId) as string)}
          tooltip="Hold to remove download"
        />
      )}
    </>
  );
}
