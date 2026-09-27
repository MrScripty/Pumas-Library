import { describe, expect, it } from 'vitest';
import { getInstallationDownloadPresentation } from './installationDownloadPresentation';

describe('getInstallationDownloadPresentation', () => {
  it('shows a measured rate while the Torch install has an active transfer', () => {
    expect(getInstallationDownloadPresentation('torch', {
      stage: 'dependencies', download_active: true, download_speed: 12 * 1024 * 1024,
    })).toEqual({ kind: 'measured', label: '12.0 MB/s' });
  });

  it('says it is measuring while an active transfer awaits its first sample', () => {
    expect(getInstallationDownloadPresentation('torch', {
      stage: 'dependencies', download_active: true, download_speed: null,
    })).toEqual({ kind: 'measuring', label: 'Measuring speed…' });
  });

  it('distinguishes a capable monitor with no current package transfer', () => {
    expect(getInstallationDownloadPresentation('torch', {
      stage: 'dependencies', download_active: false, download_speed: null,
      download_measurement_available: true,
    })).toEqual({ kind: 'waiting', label: 'No package transfer active' });
  });

  it('reports unsupported telemetry instead of silently omitting the status', () => {
    expect(getInstallationDownloadPresentation('torch', {
      stage: 'dependencies', download_active: false, download_speed: null,
      download_measurement_available: false,
    })).toEqual({ kind: 'unavailable', label: 'Speed monitoring unavailable' });
  });

  it('does not label an inactive transfer with its last sampled speed', () => {
    expect(getInstallationDownloadPresentation('torch', {
      stage: 'dependencies', download_active: false, download_speed: 12 * 1024 * 1024,
      download_measurement_available: true,
    })).toEqual({ kind: 'waiting', label: 'No package transfer active' });
  });

  it('prioritizes unavailable monitoring over stale speed or active state', () => {
    expect(getInstallationDownloadPresentation('torch', {
      stage: 'dependencies', download_active: true, download_speed: 12 * 1024 * 1024,
      download_measurement_available: false,
    })).toEqual({ kind: 'unavailable', label: 'Speed monitoring unavailable' });
  });

  it('does not show completed or unrelated installation stages as downloads', () => {
    expect(getInstallationDownloadPresentation('torch', {
      stage: 'dependencies', download_speed: 12 * 1024 * 1024, completed_at: 'done',
    })).toBeNull();
    expect(getInstallationDownloadPresentation('ollama', {
      stage: 'dependencies', download_speed: null, download_active: false,
    })).toBeNull();
  });
});
