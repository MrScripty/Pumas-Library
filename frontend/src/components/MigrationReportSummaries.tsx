import { AlertTriangle, CheckCircle2 } from 'lucide-react';
import type { MigrationDryRunReport, MigrationExecutionReport } from '../types/api';

interface MigrationReportSummariesProps {
  lastDryRunReport: MigrationDryRunReport | null;
  lastExecutionReport: MigrationExecutionReport | null;
}

export function MigrationReportSummaries({
  lastDryRunReport,
  lastExecutionReport,
}: MigrationReportSummariesProps) {
  return (
    <>
      {lastDryRunReport && (
        <div className="p-3 rounded border border-[hsl(var(--launcher-border)/0.7)] bg-[hsl(var(--launcher-bg-secondary)/0.45)]">
          <div className="text-xs font-medium text-[hsl(var(--launcher-text-primary))]">
            Last Dry Run
          </div>
          <div className="mt-1 text-xs text-[hsl(var(--launcher-text-secondary))]">
            {lastDryRunReport.move_candidates} moves, {lastDryRunReport.keep_candidates} keep,{' '}
            {lastDryRunReport.collision_count} collisions,{' '}
            {lastDryRunReport.blocked_partial_count} blocked partial,{' '}
            {lastDryRunReport.error_count} errors
          </div>
        </div>
      )}

      {lastExecutionReport && (
        <MigrationExecutionSummary report={lastExecutionReport} />
      )}
    </>
  );
}

function MigrationExecutionSummary({
  report,
}: {
  report: MigrationExecutionReport;
}) {
  const metadataDirCount = report.metadata_dir_count;
  const metadataIndexCount = report.index_metadata_model_count;
  const partialIndexCount = report.index_partial_download_count;
  const staleIndexCount = report.index_stale_model_count;

  return (
    <div className="p-3 rounded border border-[hsl(var(--launcher-border)/0.7)] bg-[hsl(var(--launcher-bg-secondary)/0.45)] space-y-2">
      <div className="flex items-center gap-2 text-xs font-medium text-[hsl(var(--launcher-text-primary))]">
        <span>Last Execution</span>
        {report.referential_integrity_ok ? (
          <CheckCircle2 className="w-4 h-4 text-[hsl(var(--accent-success))]" />
        ) : (
          <AlertTriangle className="w-4 h-4 text-[hsl(var(--accent-error))]" />
        )}
      </div>
      <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">
        {report.completed_move_count} completed, {report.skipped_move_count} skipped,{' '}
        {report.error_count} errors
      </div>
      <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">
        Reindexed {report.reindexed_model_count} models, metadata dirs {metadataDirCount},
        {' '}metadata index {metadataIndexCount}
      </div>
      <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">
        Partial index rows {partialIndexCount}, stale index rows {staleIndexCount}
      </div>
      <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">
        Referential integrity: {report.referential_integrity_ok ? 'OK' : 'FAILED'}
      </div>
      {report.referential_integrity_errors.length > 0 && (
        <div className="space-y-1">
          <div className="text-xs text-[hsl(var(--accent-error))]">Integrity Errors</div>
          <div className="max-h-28 overflow-y-auto space-y-1">
            {report.referential_integrity_errors.map((error, index) => (
              <div
                key={`${index}-${error}`}
                className="text-xs font-mono p-2 rounded bg-[hsl(var(--accent-error)/0.1)] border border-[hsl(var(--accent-error)/0.25)] text-[hsl(var(--launcher-text-primary))]"
              >
                {error}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
