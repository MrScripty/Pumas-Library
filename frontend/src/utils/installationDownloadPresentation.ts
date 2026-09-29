import { formatSpeed } from './formatters';

interface InstallationDownloadSnapshot {
  stage: string;
  download_active?: boolean;
  download_measurement_available?: boolean | null;
  download_speed?: number | null;
  download_source_url?: string | null;
  completed_at?: string;
}

export type InstallationDownloadPresentation = {
  kind: 'measured' | 'measuring' | 'waiting' | 'unavailable';
  label: string;
};

export function getInstallationDownloadPresentation(
  appId: string | null | undefined,
  progress: InstallationDownloadSnapshot | null | undefined,
): InstallationDownloadPresentation | null {
  if (!progress || progress.completed_at) {
    return null;
  }

  if (
    progress.download_measurement_available !== false &&
    progress.download_active !== false &&
    progress.download_speed !== null &&
    progress.download_speed !== undefined
  ) {
    return { kind: 'measured', label: formatSpeed(progress.download_speed) };
  }

  if (progress.download_measurement_available === false) {
    return { kind: 'unavailable', label: 'Speed monitoring unavailable' };
  }

  if (progress.download_active || progress.stage === 'download') {
    return { kind: 'measuring', label: 'Measuring speed…' };
  }

  if (progress.download_source_url) {
    return { kind: 'waiting', label: 'No package transfer active' };
  }

  if (appId === 'torch' && progress.stage === 'dependencies') {
    return {
      kind: 'waiting',
      label: progress.download_measurement_available === true
        ? 'No package transfer active'
        : 'Starting download monitor…',
    };
  }

  if (appId === 'torch' && progress.stage === 'resolving') {
    return { kind: 'waiting', label: 'Waiting for package transfer…' };
  }

  return null;
}
