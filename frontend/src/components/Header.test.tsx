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

  const mockCacheStatus = {
    has_cache: true,
    is_valid: true,
    is_fetching: false,
    age_seconds: 120,
    releases_count: 102,
  };

  const defaultProps = {
    systemResources: mockSystemResources,
    diskSpacePercent: 80,
    launcherVersion: 'v1.0.0-abc1234',
    launcherUpdateAvailable: false,
    isUpdatingLauncher: false,
    onUpdate: vi.fn(),
    onMinimize: vi.fn(),
    onClose: vi.fn(),
    cacheStatus: mockCacheStatus,
    installationProgress: null,
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

  it('displays status message from cache', () => {
    render(<Header {...defaultProps} />);
    expect(screen.getByText(/Cached data/)).toBeInTheDocument();
    expect(screen.getByText(/102 releases/)).toBeInTheDocument();
  });

  it('shows update button when update is available', () => {
    const { container } = render(<Header {...defaultProps} launcherUpdateAvailable={true} />);
    // Verify green up arrow is displayed
    const updateIcon = container.querySelector('.lucide-arrow-up');
    expect(updateIcon).toBeInTheDocument();
  });

  it('shows check for updates button when no cache', () => {
    const noCacheStatus = {
      has_cache: false,
      is_valid: false,
      is_fetching: false,
    };
    const { container } = render(<Header {...defaultProps} cacheStatus={noCacheStatus} />);
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

  it('shows fetching state when cache is being updated', () => {
    const fetchingCacheStatus = {
      ...mockCacheStatus,
      is_fetching: true,
    };

    render(<Header {...defaultProps} cacheStatus={fetchingCacheStatus} />);
    expect(screen.getByText(/Fetching releases/)).toBeInTheDocument();
  });

  it('shows offline mode when no cache available', () => {
    const noCacheStatus = {
      has_cache: false,
      is_valid: false,
      is_fetching: false,
    };

    render(<Header {...defaultProps} cacheStatus={noCacheStatus} />);
    expect(screen.getByText(/No cache available - offline mode/)).toBeInTheDocument();
  });
});
