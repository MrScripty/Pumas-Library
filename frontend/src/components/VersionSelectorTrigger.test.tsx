import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { VersionSelectorTrigger } from './VersionSelectorTrigger';

function renderTrigger(overrides: Partial<React.ComponentProps<typeof VersionSelectorTrigger>> = {}) {
  const props: React.ComponentProps<typeof VersionSelectorTrigger> = {
    activeVersion: 'v1.0.0',
    canMakeDefault: true,
    defaultVersion: 'v1.0.0',
    displayVersion: 'v1.0.0',
    emphasizeInstall: false,
    folderIconColor: 'text-[hsl(var(--text-tertiary))]',
    hasInstallActivity: false,
    hasInstalledVersions: true,
    hasNewVersion: false,
    hasVersionsToShow: true,
    installNetworkStatus: 'idle',
    installingVersion: null,
    isInstallFailed: false,
    isInstallPending: false,
    isLoading: false,
    isOpeningPath: false,
    isSwitching: false,
    latestVersion: null,
    onOpenActiveInstall: vi.fn(),
    onOpenVersionManager: vi.fn(),
    onToggleDefault: vi.fn(),
    popoverTriggerProps: {
      'aria-controls': 'version-actions',
      'aria-expanded': false,
      'aria-haspopup': 'dialog',
      onClick: vi.fn(),
      ref: vi.fn(),
    },
    ringDegrees: 0,
    showOpenedIndicator: false,
    ...overrides,
  };

  render(<VersionSelectorTrigger {...props} />);
  return props;
}

describe('VersionSelectorTrigger', () => {
  it('keeps default and action buttons separate from the version trigger', () => {
    const props = renderTrigger();

    fireEvent.click(screen.getByRole('button', { name: 'v1.0.0' }));
    expect(props.popoverTriggerProps.onClick).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: 'v1.0.0' })).toHaveAttribute(
      'aria-controls',
      'version-actions'
    );

    fireEvent.click(screen.getByTitle('Click to unset as default'));
    expect(props.onToggleDefault).toHaveBeenCalledTimes(1);
    expect(props.popoverTriggerProps.onClick).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByTitle('Open active version in file manager'));
    expect(props.onOpenActiveInstall).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByTitle('Install new version'));
    expect(props.onOpenVersionManager).toHaveBeenCalledTimes(1);
  });

  it('opens the version manager when no versions are installed', () => {
    const props = renderTrigger({
      activeVersion: null,
      canMakeDefault: false,
      defaultVersion: null,
      displayVersion: 'No version',
      hasInstalledVersions: false,
      hasVersionsToShow: false,
    });

    fireEvent.click(screen.getByTitle('Install your first version'));
    expect(props.onOpenVersionManager).toHaveBeenCalledTimes(1);
  });

  it('opens the version manager while versions are loading', () => {
    const props = renderTrigger({
      isLoading: true,
    });

    fireEvent.click(screen.getByTitle('Install new version'));
    expect(props.onOpenVersionManager).toHaveBeenCalledTimes(1);
  });

  it('opens the first-install version manager while versions are loading', () => {
    const props = renderTrigger({
      activeVersion: null,
      canMakeDefault: false,
      defaultVersion: null,
      displayVersion: 'No version',
      hasInstalledVersions: false,
      hasVersionsToShow: false,
      isLoading: true,
    });

    fireEvent.click(screen.getByTitle('Install your first version'));
    expect(props.onOpenVersionManager).toHaveBeenCalledTimes(1);
  });

  it('describes install ring progress and disk usage in text', () => {
    renderTrigger({ hasInstallActivity: true, ringDegrees: 180, diskUsageLabel: 'Disk usage unavailable' });

    expect(screen.getByRole('button', { name: 'Install new version' }))
      .toHaveAccessibleDescription('Installation 50% complete');
    expect(screen.getByTitle('Open active version in file manager · Disk usage unavailable')).toBeInTheDocument();
  });
});
