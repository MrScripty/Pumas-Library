import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
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
    onMinimize: vi.fn(),
    onClose: vi.fn(),
    networkAvailable: true,
    modelLibraryLoaded: true,
    installationProgress: null,
    activeModelDownload: null,
    activeModelDownloadCount: 0,
  };

  it('renders AI Manager title', () => {
    render(<Header {...defaultProps} />);
    expect(screen.getByText('AI Manager')).toBeInTheDocument();
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
    // Verify green up arrow is displayed
    const updateIcon = container.querySelector('.lucide-arrow-up');
    expect(updateIcon).toBeInTheDocument();
  });

  it('shows check for updates button when no update is available', () => {
    const { container } = render(<Header {...defaultProps} />);
    // Verify refresh icon is displayed
    const checkIcon = container.querySelector('.lucide-refresh-cw');
    expect(checkIcon).toBeInTheDocument();
  });

  it('displays installation progress when installing', () => {
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
    expect(screen.getByText(/Downloading at/)).toBeInTheDocument();
    expect(screen.getByText(/25% complete/)).toBeInTheDocument();
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
