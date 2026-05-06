import { describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { Header } from './Header';
import type { SystemResources } from '../types/apps';

/**
 * Compact Header Component Tests
 *
 * Tests the new compact header design with:
 * - Resource monitors (CPU/GPU with load indicators, RAM/VRAM bars)
 * - Status display area
 * - Launcher version and update functionality
 */
describe('Header Component', () => {
  const mockSystemResources: SystemResources = {
    cpu: { usage: 45 },
    gpu: { usage: 60, memory_total: 8, memory: 4.5 },
    ram: { total: 16, usage: 50 },
    disk: { total: 500, free: 100, usage: 80 },
  };

  const defaultProps = {
    systemResources: mockSystemResources,
    launcherUpdateAvailable: false,
    launcherLatestVersion: null,
    isCheckingLauncherUpdates: false,
    onCheckLauncherUpdates: vi.fn(),
    onDownloadLauncherUpdate: vi.fn(),
    onMinimize: vi.fn(),
    onClose: vi.fn(),
    networkAvailable: true,
    modelLibraryLoaded: true,
    installationProgress: null,
    activeModelDownload: null,
    activeModelDownloadCount: 0,
  };

  it('does not render the AI Manager title text', () => {
    render(<Header {...defaultProps} />);
    expect(screen.queryByText('AI Manager')).not.toBeInTheDocument();
  });

  it('keeps the header draggable while leaving buttons non-draggable', () => {
    const { container } = render(<Header {...defaultProps} />);

    const dragRegion = container.querySelector('.app-region-drag');
    const noDragControls = container.querySelectorAll('.app-region-no-drag');

    expect(dragRegion).toBeInTheDocument();
    expect(noDragControls).toHaveLength(3);
  });

  it('displays resource icons with tooltips on hover', () => {
    const { container } = render(<Header {...defaultProps} />);
    // Verify CPU and GPU icons are present
    const cpuIcon = container.querySelector('.lucide-cpu');
    const gpuIcon = container.querySelector('.lucide-gpu');
    const bicepsIcons = container.querySelectorAll('.lucide-biceps-flexed');

    expect(cpuIcon).toBeInTheDocument();
    expect(gpuIcon).toBeInTheDocument();
    expect(bicepsIcons).toHaveLength(2); // One for CPU load, one for GPU load
  });

  it('shows resource bars for RAM and VRAM', () => {
    const { container } = render(<Header {...defaultProps} />);
    // Verify progress bars are present (using background color classes)
    const ramBar = container.querySelector('[class*="launcher-accent-ram"]');
    const vramBar = container.querySelector('[class*="launcher-accent-gpu"]');

    expect(ramBar).toBeInTheDocument();
    expect(vramBar).toBeInTheDocument();
  });

  it('displays network and model library status', () => {
    render(<Header {...defaultProps} />);
    expect(screen.getByText(/Network online · model library ready/)).toBeInTheDocument();
  });

  it('shows update button when update is available', () => {
    const { container } = render(<Header {...defaultProps} launcherUpdateAvailable={true} />);
    const refreshIcon = container.querySelector('.lucide-refresh-cw');
    const updateIcon = container.querySelector('.lucide-download');

    expect(refreshIcon).toBeInTheDocument();
    expect(updateIcon).toBeInTheDocument();
  });

  it('shows check for updates button when no update is available', () => {
    const { container } = render(<Header {...defaultProps} />);
    // Verify refresh icon is displayed
    const checkIcon = container.querySelector('.lucide-refresh-cw');
    expect(checkIcon).toBeInTheDocument();
  });

  it('checks GitHub releases when the refresh button is clicked', () => {
    const onCheckLauncherUpdates = vi.fn();

    render(<Header {...defaultProps} onCheckLauncherUpdates={onCheckLauncherUpdates} />);

    fireEvent.click(screen.getByLabelText('Check GitHub releases for updates'));

    expect(onCheckLauncherUpdates).toHaveBeenCalledTimes(1);
  });

  it('opens the download action when an update is available', () => {
    const onDownloadLauncherUpdate = vi.fn();

    render(
      <Header
        {...defaultProps}
        launcherUpdateAvailable={true}
        launcherLatestVersion="v0.3.1"
        onDownloadLauncherUpdate={onDownloadLauncherUpdate}
      />
    );

    fireEvent.click(screen.getByLabelText('Download v0.3.1 from GitHub'));

    expect(onDownloadLauncherUpdate).toHaveBeenCalledTimes(1);
  });

  it('displays runtime download activity without taking over the header with install percent', () => {
    const installationProgress = {
      tag: 'v0.6.0',
      started_at: new Date().toISOString(),
      stage: 'download' as const,
      stage_progress: 50,
      overall_progress: 25,
      current_item: 'archive.zip',
      download_speed: 1024000, // 1 MB/s
      eta_seconds: 60,
      total_size: 10240000,
      downloaded_bytes: 2560000,
      dependency_count: null,
      completed_dependencies: 0,
      completed_items: [],
      error: null,
    };

    render(<Header {...defaultProps} installationProgress={installationProgress} />);
    expect(screen.getByText(/Downloading 1 runtime/)).toBeInTheDocument();
    expect(screen.getByText(/1000\.0 KB\/s/)).toBeInTheDocument();
    expect(screen.queryByText(/25% complete/)).not.toBeInTheDocument();
  });

  it('combines model and runtime download counts and speed in the header', () => {
    const activeModelDownload = {
      downloadId: 'dl-1',
      repoId: 'meta-llama/Llama-3.2-1B-Instruct',
      status: 'downloading' as const,
      progress: 42,
      downloadedBytes: 2 * 1024 * 1024 * 1024,
      totalBytes: 5 * 1024 * 1024 * 1024,
      speed: 4 * 1024 * 1024,
      etaSeconds: 120,
    };
    const installationProgress = {
      tag: 'v0.22.1',
      started_at: new Date().toISOString(),
      stage: 'download' as const,
      stage_progress: 0,
      overall_progress: 0,
      current_item: 'ollama-linux-amd64.tar.zst',
      download_speed: 1.7 * 1024 * 1024,
      eta_seconds: 60,
      total_size: 10240000,
      downloaded_bytes: 0,
      dependency_count: null,
      completed_dependencies: 0,
      completed_items: [],
      error: null,
    };

    render(
      <Header
        {...defaultProps}
        activeModelDownload={activeModelDownload}
        activeModelDownloadCount={1}
        installationProgress={installationProgress}
      />
    );

    expect(screen.getByText(/Downloading 1 model & 1 runtime/)).toBeInTheDocument();
    expect(screen.getByText(/5\.7 MB\/s/)).toBeInTheDocument();
  });

  it('shows checking state while network and model library status are unresolved', () => {
    render(<Header {...defaultProps} networkAvailable={null} modelLibraryLoaded={null} />);
    expect(screen.getByText(/Checking network and model library/)).toBeInTheDocument();
  });

  it('shows network unavailable when offline', () => {
    render(<Header {...defaultProps} networkAvailable={false} modelLibraryLoaded={true} />);
    expect(screen.getByText(/Network unavailable/)).toBeInTheDocument();
  });

  it('shows model library unavailable when db is not loaded', () => {
    render(<Header {...defaultProps} networkAvailable={true} modelLibraryLoaded={false} />);
    expect(screen.getByText(/Model library database unavailable/)).toBeInTheDocument();
  });

  it('shows singular download count when one model is downloading', () => {
    const activeModelDownload = {
      downloadId: 'dl-1',
      repoId: 'meta-llama/Llama-3.2-1B-Instruct',
      status: 'downloading' as const,
      progress: 42,
      downloadedBytes: 2 * 1024 * 1024 * 1024,
      totalBytes: 5 * 1024 * 1024 * 1024,
      speed: 10 * 1024 * 1024,
      etaSeconds: 120,
    };

    render(
      <Header
        {...defaultProps}
        activeModelDownload={activeModelDownload}
        activeModelDownloadCount={1}
      />
    );
    expect(screen.getByText(/Downloading 1 model/)).toBeInTheDocument();
    expect(screen.getByText(/10\.0 MB\/s/)).toBeInTheDocument();
  });

  it('shows aggregate download count when multiple models are active', () => {
    const activeModelDownload = {
      downloadId: 'dl-1',
      repoId: 'meta-llama/Llama-3.2-1B-Instruct',
      status: 'downloading' as const,
      progress: 42,
      downloadedBytes: 2 * 1024 * 1024 * 1024,
      totalBytes: 5 * 1024 * 1024 * 1024,
      speed: 10 * 1024 * 1024,
      etaSeconds: 120,
    };

    render(
      <Header
        {...defaultProps}
        activeModelDownload={activeModelDownload}
        activeModelDownloadCount={3}
      />
    );
    expect(screen.getByText(/Downloading 3 models/)).toBeInTheDocument();
    expect(screen.getByText(/10\.0 MB\/s/)).toBeInTheDocument();
  });
});
