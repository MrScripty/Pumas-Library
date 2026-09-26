import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ProgressDetailsView } from './ProgressDetailsView';
import type { InstallationProgress } from '../hooks/useVersions';

const dependencyProgress: InstallationProgress = {
  tag: 'v1.2.3',
  started_at: '2026-04-12T00:00:00Z',
  stage: 'dependencies',
  stage_progress: 45,
  overall_progress: 70,
  current_item: 'torch',
  download_speed: 1024 * 1024,
  eta_seconds: 45,
  total_size: 10 * 1024 * 1024,
  downloaded_bytes: 5 * 1024 * 1024,
  dependency_count: 4,
  completed_dependencies: 2,
  completed_items: [
    {
      name: 'wheel.whl',
      type: 'dependency',
      size: 2048,
      completed_at: '2026-04-12T00:02:00Z',
    },
  ],
  error: 'Dependency install failed',
  log_path: '/tmp/install.log',
};

describe('ProgressDetailsView', () => {
  it('treats Torch setup as indeterminate and shows the backend phase', () => {
    render(<ProgressDetailsView
      appId="torch"
      progress={{ ...dependencyProgress, tag: 'v2.14.0', stage: 'setup', stage_progress: 0, overall_progress: 95, current_item: 'Creating managed Python environment', error: null }}
      installingVersion="v2.14.0" showCompletedItems={false}
      onToggleCompletedItems={vi.fn()} onBackToList={vi.fn()} onOpenLogPath={vi.fn()}
    />);
    expect(screen.getByText('Creating managed Python environment')).toBeInTheDocument();
    expect(screen.queryByText('95%')).not.toBeInTheDocument();
    expect(screen.queryByText('0%')).not.toBeInTheDocument();
    expect(screen.getByRole('progressbar', { name: 'Overall installation progress' })).not.toHaveAttribute('aria-valuenow');
    expect(screen.getByRole('progressbar', { name: 'Final Setup progress' })).not.toHaveAttribute('aria-valuenow');
  });

  it('shows measured Torch setup progress when the stage reports real movement', () => {
    render(<ProgressDetailsView
      appId="torch"
      progress={{ ...dependencyProgress, tag: 'v2.14.0', stage: 'setup', stage_progress: 40, overall_progress: 95, current_item: 'Qualifying runtime', error: null }}
      installingVersion="v2.14.0" showCompletedItems={false}
      onToggleCompletedItems={vi.fn()} onBackToList={vi.fn()} onOpenLogPath={vi.fn()}
    />);
    expect(screen.getByRole('progressbar', { name: 'Overall installation progress' })).toHaveAttribute('aria-valuenow', '95');
    expect(screen.getByRole('progressbar', { name: 'Final Setup progress' })).toHaveAttribute('aria-valuenow', '40');
  });

  it('renders dependency progress details and routes detail actions', () => {
    const onBackToList = vi.fn();
    const onToggleCompletedItems = vi.fn();
    const onOpenLogPath = vi.fn();

    render(
      <ProgressDetailsView
        progress={dependencyProgress}
        installingVersion="v1.2.3"
        showCompletedItems={true}
        onToggleCompletedItems={onToggleCompletedItems}
        onBackToList={onBackToList}
        onOpenLogPath={onOpenLogPath}
      />
    );

    expect(screen.getByText('Installing v1.2.3')).toBeInTheDocument();
    expect(screen.getByText('Overall Progress')).toBeInTheDocument();
    expect(screen.getByText('Installing Dependencies')).toBeInTheDocument();
    expect(screen.getByText('2 / 4')).toBeInTheDocument();
    expect(screen.getByText('wheel.whl')).toBeInTheDocument();
    expect(screen.getByText('Dependency install failed')).toBeInTheDocument();
    expect(screen.getByRole('progressbar', { name: 'Overall installation progress' })).toHaveAttribute(
      'aria-valuenow',
      '70'
    );
    expect(screen.getByRole('progressbar', { name: 'Installing Dependencies progress' })).toHaveAttribute(
      'aria-valuenow',
      '45'
    );
    expect(screen.getByRole('alert')).toHaveTextContent('Installation Failed');

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    fireEvent.click(screen.getByText('Completed Items (1)'));
    fireEvent.click(screen.getByRole('button', { name: /open log/i }));

    expect(onBackToList).toHaveBeenCalledTimes(1);
    expect(onToggleCompletedItems).toHaveBeenCalledTimes(1);
    expect(onOpenLogPath).toHaveBeenCalledWith('/tmp/install.log');
  });

  it('renders cancellation and success summaries for completed installs', () => {
    const { rerender } = render(
      <ProgressDetailsView
        progress={{
          ...dependencyProgress,
          error: 'User cancelled installation',
          log_path: null,
        }}
        installingVersion="v1.2.3"
        showCompletedItems={false}
        onToggleCompletedItems={vi.fn()}
        onBackToList={vi.fn()}
        onOpenLogPath={vi.fn()}
      />
    );

    expect(screen.getByText('Installation Cancelled')).toBeInTheDocument();
    expect(
      screen.getByText('The installation was stopped and incomplete files have been removed')
    ).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveAttribute('aria-live', 'polite');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();

    rerender(
      <ProgressDetailsView
        progress={{
          ...dependencyProgress,
          error: null,
          completed_at: '2026-04-12T00:05:00Z',
          success: true,
        }}
        installingVersion="v1.2.3"
        showCompletedItems={false}
        onToggleCompletedItems={vi.fn()}
        onBackToList={vi.fn()}
        onOpenLogPath={vi.fn()}
      />
    );

    expect(screen.getByText('Installation Complete!')).toBeInTheDocument();
    expect(screen.getByText('v1.2.3 has been successfully installed')).toBeInTheDocument();
    const successStatus = screen.getByRole('status');
    expect(successStatus).toHaveAttribute('aria-live', 'polite');
    expect(screen.getAllByRole('status')).toHaveLength(1);

    rerender(
      <ProgressDetailsView
        progress={{
          ...dependencyProgress,
          error: null,
          completed_at: '2026-04-12T00:05:00Z',
          success: true,
        }}
        installingVersion="v1.2.3"
        showCompletedItems={false}
        onToggleCompletedItems={vi.fn()}
        onBackToList={vi.fn()}
        onOpenLogPath={vi.fn()}
      />
    );

    expect(screen.getByRole('status')).toBe(successStatus);
    expect(screen.getAllByRole('status')).toHaveLength(1);
  });
});
