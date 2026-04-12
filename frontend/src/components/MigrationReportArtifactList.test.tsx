import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { MigrationReportArtifactList } from './MigrationReportArtifactList';

describe('MigrationReportArtifactList', () => {
  it('renders the empty state when no report artifacts exist', () => {
    render(
      <MigrationReportArtifactList
        deletingReportPath={null}
        openingPath={null}
        reports={[]}
        onDeleteReport={vi.fn()}
        onOpenPath={vi.fn()}
      />
    );

    expect(screen.getByText('No migration reports yet.')).toBeInTheDocument();
  });

  it('routes open and delete actions for report artifacts', () => {
    const onDeleteReport = vi.fn();
    const onOpenPath = vi.fn();

    render(
      <MigrationReportArtifactList
        deletingReportPath={null}
        openingPath={null}
        reports={[
          {
            generated_at: '2026-04-12 00:00:00',
            report_kind: 'dry_run',
            json_report_path: '/tmp/reports/2026/04/report.json',
            markdown_report_path: '/tmp/reports/2026/04/report.md',
          },
        ]}
        onDeleteReport={onDeleteReport}
        onOpenPath={onOpenPath}
      />
    );

    expect(screen.getByText('Dry Run')).toBeInTheDocument();
    expect(screen.getByText(/JSON: \.\.\.\/2026\/04\/report\.json/)).toBeInTheDocument();
    expect(screen.getByText(/MD: \.\.\.\/2026\/04\/report\.md/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Open JSON/i }));
    fireEvent.click(screen.getByRole('button', { name: /Open MD/i }));
    fireEvent.click(screen.getByRole('button', { name: /Delete/i }));

    expect(onOpenPath).toHaveBeenNthCalledWith(1, '/tmp/reports/2026/04/report.json', 'JSON');
    expect(onOpenPath).toHaveBeenNthCalledWith(2, '/tmp/reports/2026/04/report.md', 'Markdown');
    expect(onDeleteReport).toHaveBeenCalledWith('/tmp/reports/2026/04/report.json');
  });

  it('shows busy labels for the artifact currently being opened or deleted', () => {
    render(
      <MigrationReportArtifactList
        deletingReportPath="/tmp/report.json"
        openingPath="/tmp/report.md"
        reports={[
          {
            generated_at: '2026-04-12 00:00:00',
            report_kind: 'execution',
            json_report_path: '/tmp/report.json',
            markdown_report_path: '/tmp/report.md',
          },
        ]}
        onDeleteReport={vi.fn()}
        onOpenPath={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: /Open JSON/i })).toBeEnabled();
    expect(screen.getByRole('button', { name: /Opening/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /Deleting/i })).toBeDisabled();
  });
});
