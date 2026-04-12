import { ExternalLink, Trash2 } from 'lucide-react';
import type { MigrationReportArtifact } from '../types/api';

interface MigrationReportArtifactListProps {
  deletingReportPath: string | null;
  openingPath: string | null;
  reports: MigrationReportArtifact[];
  onDeleteReport: (reportPath: string) => void;
  onOpenPath: (path: string, label: string) => void;
}

function shortenPath(value: string): string {
  const normalized = value.replaceAll('\\', '/');
  const parts = normalized.split('/');
  if (parts.length <= 3) {
    return value;
  }
  return `.../${parts.slice(-3).join('/')}`;
}

function reportKindLabel(kind: string): string {
  if (kind === 'dry_run') return 'Dry Run';
  if (kind === 'execution') return 'Execution';
  return kind;
}

export function MigrationReportArtifactList({
  deletingReportPath,
  openingPath,
  reports,
  onDeleteReport,
  onOpenPath,
}: MigrationReportArtifactListProps) {
  if (reports.length === 0) {
    return (
      <div className="text-xs text-[hsl(var(--launcher-text-tertiary))]">
        No migration reports yet.
      </div>
    );
  }

  return (
    <>
      {reports.map((report) => (
        <div
          key={`${report.generated_at}-${report.json_report_path}`}
          className="p-2 rounded border border-[hsl(var(--launcher-border)/0.65)] bg-[hsl(var(--launcher-bg-secondary)/0.35)]"
        >
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-2 min-w-0">
              <span className="text-[10px] px-2 py-0.5 rounded bg-[hsl(var(--launcher-bg-tertiary)/0.85)] text-[hsl(var(--launcher-text-secondary))] uppercase tracking-wide">
                {reportKindLabel(report.report_kind)}
              </span>
              <span className="text-xs text-[hsl(var(--launcher-text-primary))] truncate">
                {report.generated_at}
              </span>
            </div>
            <div className="flex items-center gap-1">
              <button
                className="px-2 py-1 text-xs rounded border border-[hsl(var(--launcher-border)/0.8)] text-[hsl(var(--launcher-text-primary))] bg-[hsl(var(--launcher-bg-secondary)/0.8)] hover:bg-[hsl(var(--launcher-bg-secondary))] disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-1"
                onClick={() => onOpenPath(report.json_report_path, 'JSON')}
                disabled={openingPath === report.json_report_path}
              >
                <ExternalLink className="w-3 h-3" />
                {openingPath === report.json_report_path ? 'Opening...' : 'Open JSON'}
              </button>
              <button
                className="px-2 py-1 text-xs rounded border border-[hsl(var(--launcher-border)/0.8)] text-[hsl(var(--launcher-text-primary))] bg-[hsl(var(--launcher-bg-secondary)/0.8)] hover:bg-[hsl(var(--launcher-bg-secondary))] disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-1"
                onClick={() => onOpenPath(report.markdown_report_path, 'Markdown')}
                disabled={openingPath === report.markdown_report_path}
              >
                <ExternalLink className="w-3 h-3" />
                {openingPath === report.markdown_report_path ? 'Opening...' : 'Open MD'}
              </button>
              <button
                className="px-2 py-1 text-xs rounded border border-[hsl(var(--accent-error)/0.35)] text-[hsl(var(--launcher-text-primary))] bg-[hsl(var(--accent-error)/0.12)] hover:bg-[hsl(var(--accent-error)/0.2)] disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-1"
                onClick={() => onDeleteReport(report.json_report_path)}
                disabled={deletingReportPath === report.json_report_path}
              >
                <Trash2 className="w-3 h-3" />
                {deletingReportPath === report.json_report_path ? 'Deleting...' : 'Delete'}
              </button>
            </div>
          </div>
          <div className="mt-2 space-y-1 text-[11px] text-[hsl(var(--launcher-text-tertiary))]">
            <div title={report.json_report_path}>JSON: {shortenPath(report.json_report_path)}</div>
            <div title={report.markdown_report_path}>MD: {shortenPath(report.markdown_report_path)}</div>
          </div>
        </div>
      ))}
    </>
  );
}
