import { describe, expect, it } from 'vitest';
import { getVersionSelectorDisplayState } from './VersionSelectorState';

const baseInput = {
  activeVersion: 'v1',
  hasNewVersion: false,
  installedVersions: ['v1'],
  installNetworkStatus: 'idle' as const,
  isLoading: false,
  latestVersion: null,
};

describe('version selector disk usage', () => {
  it('reports unknown disk usage instead of presenting it as 0%', () => {
    const state = getVersionSelectorDisplayState({ ...baseInput, diskSpacePercent: null });
    expect(state.diskUsageLabel).toBe('Disk usage unavailable');
    expect(state.folderIconColor).toBe('text-tertiary');
  });

  it('labels and colours a measured reading', () => {
    expect(getVersionSelectorDisplayState({ ...baseInput, diskSpacePercent: 0 }).diskUsageLabel).toBe('Disk 0% used');
    const nearlyFull = getVersionSelectorDisplayState({ ...baseInput, diskSpacePercent: 96.4 });
    expect(nearlyFull.diskUsageLabel).toBe('Disk 96% used');
    expect(nearlyFull.folderIconColor).toBe('text-accent-error');
  });
});
