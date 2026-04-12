import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { MigrationReportSummaries } from './MigrationReportSummaries';

describe('MigrationReportSummaries', () => {
  it('renders dry-run and execution summaries including integrity errors', () => {
    render(
      <MigrationReportSummaries
        lastDryRunReport={{
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
        }}
        lastExecutionReport={{
          generated_at: '2026-04-12T00:00:00Z',
          completed_at: '2026-04-12T00:05:00Z',
          resumed_from_checkpoint: false,
          checkpoint_path: '/tmp/checkpoint.json',
          planned_move_count: 9,
          completed_move_count: 8,
          skipped_move_count: 1,
          error_count: 1,
          reindexed_model_count: 8,
          metadata_dir_count: 8,
          index_model_count: 8,
          index_metadata_model_count: 8,
          index_partial_download_count: 1,
          index_stale_model_count: 0,
          referential_integrity_ok: false,
          referential_integrity_errors: ['missing metadata row for qwen/test'],
          machine_readable_report_path: null,
          human_readable_report_path: null,
          results: [],
        }}
      />
    );

    expect(screen.getByText('Last Dry Run')).toBeInTheDocument();
    expect(screen.getByText(/9 moves, 1 keep, 0 collisions, 0 blocked partial, 0 errors/)).toBeInTheDocument();
    expect(screen.getByText('Last Execution')).toBeInTheDocument();
    expect(screen.getByText(/8 completed, 1 skipped, 1 errors/)).toBeInTheDocument();
    expect(screen.getByText(/Partial index rows 1, stale index rows 0/)).toBeInTheDocument();
    expect(screen.getByText('Referential integrity: FAILED')).toBeInTheDocument();
    expect(screen.getByText('Integrity Errors')).toBeInTheDocument();
    expect(screen.getByText('missing metadata row for qwen/test')).toBeInTheDocument();
  });

  it('renders nothing when no reports are available', () => {
    const { container } = render(
      <MigrationReportSummaries
        lastDryRunReport={null}
        lastExecutionReport={null}
      />
    );

    expect(container).toBeEmptyDOMElement();
  });
});
