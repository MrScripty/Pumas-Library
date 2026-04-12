import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const {
  deleteReportMock,
  executeMigrationMock,
  generateDryRunMock,
  isApiAvailableMock,
  listReportsMock,
  openPathMock,
  pruneReportsMock,
} = vi.hoisted(() => ({
  deleteReportMock: vi.fn(),
  executeMigrationMock: vi.fn(),
  generateDryRunMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  listReportsMock: vi.fn(),
  openPathMock: vi.fn(),
  pruneReportsMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    delete_model_migration_report: deleteReportMock,
    execute_model_migration: executeMigrationMock,
    generate_model_migration_dry_run_report: generateDryRunMock,
    list_model_migration_reports: listReportsMock,
    open_path: openPathMock,
    prune_model_migration_reports: pruneReportsMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

import { MigrationReportsPanel } from './MigrationReportsPanel';

const artifact = {
  generated_at: '2026-04-12T00:00:00Z',
  report_kind: 'dry_run',
  json_report_path: '/tmp/reports/2026/04/report.json',
  markdown_report_path: '/tmp/reports/2026/04/report.md',
};

describe('MigrationReportsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
    listReportsMock.mockResolvedValue({
      success: true,
      reports: [artifact],
    });
    generateDryRunMock.mockResolvedValue({
      success: true,
      report: {
        generated_at: '2026-04-12T00:00:00Z',
        total_models: 10,
        move_candidates: 9,
        keep_candidates: 1,
        collision_count: 0,
        blocked_partial_count: 0,
        error_count: 0,
        models_with_findings: 2,
        items: [],
        machine_readable_report_path: null,
        human_readable_report_path: null,
      },
    });
    executeMigrationMock.mockResolvedValue({
      success: true,
      report: {
        generated_at: '2026-04-12T00:00:00Z',
        completed_at: '2026-04-12T00:05:00Z',
        resumed_from_checkpoint: false,
        checkpoint_path: '/tmp/checkpoint.json',
        planned_move_count: 9,
        completed_move_count: 9,
        skipped_move_count: 0,
        error_count: 0,
        reindexed_model_count: 9,
        metadata_dir_count: 9,
        index_model_count: 9,
        index_metadata_model_count: 9,
        index_partial_download_count: 0,
        index_stale_model_count: 0,
        referential_integrity_ok: true,
        referential_integrity_errors: [],
        machine_readable_report_path: null,
        human_readable_report_path: null,
        results: [],
      },
    });
    deleteReportMock.mockResolvedValue({
      success: true,
      removed: true,
    });
    pruneReportsMock.mockResolvedValue({
      success: true,
      removed: 2,
      kept: 5,
    });
    openPathMock.mockResolvedValue({
      success: false,
      error: 'cannot open path',
    });
    vi.spyOn(window, 'confirm').mockReturnValue(true);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('loads reports, executes migration actions, and shows operator feedback', async () => {
    render(<MigrationReportsPanel />);

    await waitFor(() => {
      expect(listReportsMock).toHaveBeenCalledTimes(1);
    });

    fireEvent.click(screen.getByRole('button', { name: /Migration Reports/i }));

    fireEvent.click(screen.getByRole('button', { name: /Generate Dry Run/i }));

    await waitFor(() => {
      expect(generateDryRunMock).toHaveBeenCalledTimes(1);
      expect(screen.getByText(/Dry-run generated: 9 moves/)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: /Execute Migration/i }));

    await waitFor(() => {
      expect(executeMigrationMock).toHaveBeenCalledTimes(1);
      expect(screen.getByText(/Migration complete: 9\/9 moved or already migrated\./)).toBeInTheDocument();
    });

    fireEvent.change(screen.getByLabelText('Keep latest'), { target: { value: '5' } });
    fireEvent.click(screen.getByRole('button', { name: /^Prune$/i }));

    await waitFor(() => {
      expect(pruneReportsMock).toHaveBeenCalledWith(5);
      expect(screen.getByText('Pruned 2 report(s); keeping latest 5.')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: /Delete/i }));

    await waitFor(() => {
      expect(deleteReportMock).toHaveBeenCalledWith('/tmp/reports/2026/04/report.json');
      expect(screen.getByText('Migration report deleted.')).toBeInTheDocument();
    });
  });

  it('reports validation and open-path failures without crashing', async () => {
    render(<MigrationReportsPanel />);

    await waitFor(() => {
      expect(listReportsMock).toHaveBeenCalledTimes(1);
    });

    fireEvent.click(screen.getByRole('button', { name: /Migration Reports/i }));
    fireEvent.change(screen.getByLabelText('Keep latest'), { target: { value: '-1' } });
    fireEvent.click(screen.getByRole('button', { name: /^Prune$/i }));

    expect(screen.getByText('Keep latest must be a non-negative integer.')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Open JSON/i }));

    await waitFor(() => {
      expect(openPathMock).toHaveBeenCalledWith('/tmp/reports/2026/04/report.json');
      expect(screen.getByText('cannot open path')).toBeInTheDocument();
    });
  });

  it('renders nothing when the backend API is unavailable', () => {
    isApiAvailableMock.mockReturnValue(false);

    const { container } = render(<MigrationReportsPanel />);

    expect(container).toBeEmptyDOMElement();
  });
});
