import type { InstallationProgress } from '../types/versions';

const STAGE_LABELS: Record<InstallationProgress['stage'], string> = {
  download: 'Downloading',
  extract: 'Extracting',
  venv: 'Creating Environment',
  dependencies: 'Installing Dependencies',
  setup: 'Final Setup',
};

export interface InstallActivityPresentation {
  active: boolean;
  indeterminate: boolean;
  phase: string;
}

export function getInstallActivityPresentation({
  appId,
  installingTag,
  progress,
}: {
  appId?: string | null;
  installingTag?: string | null;
  progress?: InstallationProgress | null;
}): InstallActivityPresentation {
  const currentProgress = progress && (!installingTag || progress.tag === installingTag) ? progress : null;
  const terminal = Boolean(currentProgress?.completed_at || currentProgress?.error);
  const active = Boolean((installingTag || currentProgress?.tag) && !terminal);
  const indeterminate = active && (!currentProgress || (
    appId === 'torch' && currentProgress.stage === 'setup' && currentProgress.stage_progress <= 0
  ));
  const stageLabel = currentProgress ? STAGE_LABELS[currentProgress.stage] : null;

  return {
    active,
    indeterminate,
    phase: currentProgress?.current_item?.trim() || stageLabel || 'Starting installation…',
  };
}
