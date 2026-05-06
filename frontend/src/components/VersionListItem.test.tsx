import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { VersionListItem } from './VersionListItem';
import type { InstallationProgress, VersionRelease } from '../hooks/useVersions';

const release: VersionRelease = {
  tagName: 'v1.2.3',
  name: 'Version 1.2.3',
  publishedAt: '2026-04-12T00:00:00Z',
  prerelease: true,
  htmlUrl: 'https://github.com/example/app/releases/tag/v1.2.3',
  totalSize: 1024 ** 3,
};

const dependencyProgress: InstallationProgress = {
  tag: 'v1.2.3',
  started_at: '2026-04-12T00:00:00Z',
  stage: 'dependencies',
  stage_progress: 60,
  overall_progress: 75,
  current_item: 'torch',
  download_speed: 1024,
  eta_seconds: 30,
  total_size: 4096,
  downloaded_bytes: 2048,
  dependency_count: 4,
  completed_dependencies: 2,
  completed_items: [],
  error: null,
};

const downloadProgress: InstallationProgress = {
  ...dependencyProgress,
  stage: 'download',
  stage_progress: 50,
  overall_progress: 8,
  current_item: 'ollama-linux-amd64.tar.zst',
  downloaded_bytes: 512,
  total_size: 1024,
  dependency_count: null,
  completed_dependencies: 0,
};

function getClosestButton(label: string): HTMLButtonElement {
  const button = screen.getByText(label).closest('button');
  if (!(button instanceof HTMLButtonElement)) {
    throw new TypeError(`Expected ${label} to have a button ancestor`);
  }

  return button;
}

function renderVersionListItem(overrides: Partial<React.ComponentProps<typeof VersionListItem>> = {}) {
  const props: React.ComponentProps<typeof VersionListItem> = {
    release,
    isInstalled: false,
    isInstalling: false,
    progress: null,
    hasError: false,
    errorMessage: null,
    isHovered: false,
    isCancelHovered: false,
    installNetworkStatus: 'idle',
    failedLogPath: null,
    onInstall: vi.fn(),
    onRemove: vi.fn(),
    onCancel: vi.fn(),
    onOpenUrl: vi.fn(),
    onOpenLogPath: vi.fn(),
    onHoverStart: vi.fn(),
    onHoverEnd: vi.fn(),
    onCancelMouseEnter: vi.fn(),
    onCancelMouseLeave: vi.fn(),
    ...overrides,
  };

  return {
    ...render(<VersionListItem {...props} />),
    props,
  };
}

describe('VersionListItem', () => {
  it('renders installable releases and routes release-link, hover, and install actions', () => {
    const { props, container } = renderVersionListItem();

    expect(screen.getByText('1.2.3')).toBeInTheDocument();
    expect(screen.getByText('Pre')).toBeInTheDocument();
    expect(screen.getByText('1.00 GB')).toBeInTheDocument();

    fireEvent.pointerEnter(container.firstChild as Element);
    fireEvent.pointerLeave(container.firstChild as Element);
    fireEvent.click(screen.getByRole('button', { name: 'Release notes' }));
    fireEvent.click(getClosestButton('1.00 GB'));

    expect(props.onHoverStart).toHaveBeenCalledTimes(1);
    expect(props.onHoverEnd).toHaveBeenCalledTimes(1);
    expect(props.onOpenUrl).toHaveBeenCalledWith(
      'https://github.com/example/app/releases/tag/v1.2.3'
    );
    expect(props.onInstall).toHaveBeenCalledTimes(1);
  });

  it('renders uninstall and error states for hovered installed releases', () => {
    const { props } = renderVersionListItem({
      isInstalled: true,
      isHovered: true,
      hasError: true,
      errorMessage: 'Install failed',
      failedLogPath: '/tmp/install.log',
    });

    expect(screen.getByText('Uninstall')).toBeInTheDocument();
    expect(screen.getByText('Install failed')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'View log' }));
    fireEvent.click(getClosestButton('Uninstall'));

    expect(props.onOpenLogPath).toHaveBeenCalledWith('/tmp/install.log');
    expect(props.onRemove).toHaveBeenCalledTimes(1);
  });

  it('renders installing dependency progress and cancel affordances', () => {
    const { props, rerender } = renderVersionListItem({
      isInstalling: true,
      progress: dependencyProgress,
      installNetworkStatus: 'downloading',
    });

    expect(screen.getByText('2/4')).toBeInTheDocument();

    const progressButton = getClosestButton('2/4');
    fireEvent.pointerEnter(progressButton);
    fireEvent.pointerLeave(progressButton);
    fireEvent.click(progressButton);

    expect(props.onCancelMouseEnter).toHaveBeenCalledTimes(1);
    expect(props.onCancelMouseLeave).toHaveBeenCalledTimes(1);
    expect(props.onCancel).toHaveBeenCalledTimes(1);

    rerender(
      <VersionListItem
        {...props}
        isInstalling={true}
        progress={dependencyProgress}
        isCancelHovered={true}
      />
    );

    expect(screen.getByText('Cancel')).toBeInTheDocument();
  });

  it('shows download percent while an install archive is downloading', () => {
    renderVersionListItem({
      isInstalling: true,
      progress: downloadProgress,
      installNetworkStatus: 'downloading',
    });

    expect(screen.getByText('50%')).toBeInTheDocument();
  });
});
