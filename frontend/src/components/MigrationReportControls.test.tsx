import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { MigrationReportControls } from './MigrationReportControls';

describe('MigrationReportControls', () => {
  it('routes control actions, input changes, and renders flash messages', () => {
    const onExecuteMigration = vi.fn();
    const onGenerateDryRun = vi.fn();
    const onKeepLatestChange = vi.fn();
    const onPruneReports = vi.fn();
    const onRefresh = vi.fn();

    render(
      <MigrationReportControls
        isExecutingMigration={false}
        isGeneratingDryRun={false}
        isLoadingReports={false}
        isPruning={false}
        keepLatest="10"
        message={{ type: 'success', text: 'Dry-run generated.' }}
        onExecuteMigration={onExecuteMigration}
        onGenerateDryRun={onGenerateDryRun}
        onKeepLatestChange={onKeepLatestChange}
        onPruneReports={onPruneReports}
        onRefresh={onRefresh}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Generate Dry Run' }));
    fireEvent.click(screen.getByRole('button', { name: /Execute Migration/i }));
    fireEvent.click(screen.getByRole('button', { name: /Refresh/i }));
    fireEvent.change(screen.getByLabelText('Keep latest'), { target: { value: '5' } });
    fireEvent.click(screen.getByRole('button', { name: /Prune/i }));

    expect(onGenerateDryRun).toHaveBeenCalledTimes(1);
    expect(onExecuteMigration).toHaveBeenCalledTimes(1);
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(onKeepLatestChange).toHaveBeenCalledWith('5');
    expect(onPruneReports).toHaveBeenCalledTimes(1);
    expect(screen.getByText('Dry-run generated.')).toBeInTheDocument();
  });

  it('disables actions while the corresponding migration tasks are busy', () => {
    render(
      <MigrationReportControls
        isExecutingMigration={true}
        isGeneratingDryRun={false}
        isLoadingReports={true}
        isPruning={true}
        keepLatest="10"
        message={{ type: 'error', text: 'Failed.' }}
        onExecuteMigration={vi.fn()}
        onGenerateDryRun={vi.fn()}
        onKeepLatestChange={vi.fn()}
        onPruneReports={vi.fn()}
        onRefresh={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: /Generate Dry Run/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /Executing/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /Refresh/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /Pruning/i })).toBeDisabled();
    expect(screen.getByText('Failed.')).toBeInTheDocument();
  });
});
