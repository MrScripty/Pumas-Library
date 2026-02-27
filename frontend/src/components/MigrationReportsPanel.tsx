import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type {
  MigrationDryRunReport,
  MigrationExecutionReport,
  MigrationReportArtifact,
} from '../types/api';
import { getLogger } from '../utils/logger';
import {
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  FileText,
  Loader2,
  PlayCircle,
  RefreshCw,
  Scissors,
  Trash2,
} from 'lucide-react';

const logger = getLogger('MigrationReportsPanel');

interface FlashMessage {
  type: 'success' | 'error' | 'info';
  text: string;
}

function formatTimestamp(value: string): string {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleString();
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

export const MigrationReportsPanel: React.FC = () => {
  const [isExpanded, setIsExpanded] = useState(false);
  const [reports, setReports] = useState<MigrationReportArtifact[]>([]);
  const [lastDryRunReport, setLastDryRunReport] = useState<MigrationDryRunReport | null>(null);
  const [lastExecutionReport, setLastExecutionReport] = useState<MigrationExecutionReport | null>(
    null
  );
  const [message, setMessage] = useState<FlashMessage | null>(null);
  const [keepLatest, setKeepLatest] = useState('10');
  const [isLoadingReports, setIsLoadingReports] = useState(false);
  const [isGeneratingDryRun, setIsGeneratingDryRun] = useState(false);
  const [isExecutingMigration, setIsExecutingMigration] = useState(false);
  const [isPruning, setIsPruning] = useState(false);
  const [deletingReportPath, setDeletingReportPath] = useState<string | null>(null);

  const fetchReports = useCallback(async () => {
    if (!isAPIAvailable()) return;

    setIsLoadingReports(true);
    try {
      const result = await api.list_model_migration_reports();
      if (!result.success) {
        setMessage({ type: 'error', text: result.error ?? 'Failed to load migration reports.' });
        return;
      }
      setReports(result.reports ?? []);
    } catch (error) {
      logger.error('Failed to list migration reports', { error });
      setMessage({ type: 'error', text: 'Failed to load migration reports.' });
    } finally {
      setIsLoadingReports(false);
    }
  }, []);

  useEffect(() => {
    if (!isAPIAvailable()) return;
    void fetchReports();
  }, [fetchReports]);

  const handleGenerateDryRun = useCallback(async () => {
    if (!isAPIAvailable()) return;

    setIsGeneratingDryRun(true);
    setMessage(null);
    try {
      const result = await api.generate_model_migration_dry_run_report();
      if (!result.success) {
        setMessage({ type: 'error', text: result.error ?? 'Dry-run generation failed.' });
        return;
      }

      const report = result.report;
      setLastDryRunReport(report);
      setMessage({
        type: 'success',
        text: `Dry-run generated: ${report.move_candidates} moves, ${report.collision_count} collisions, ${report.error_count} errors.`,
      });
      await fetchReports();
    } catch (error) {
      logger.error('Failed to generate migration dry-run report', { error });
      setMessage({ type: 'error', text: 'Dry-run generation failed.' });
    } finally {
      setIsGeneratingDryRun(false);
    }
  }, [fetchReports]);

  const handleExecuteMigration = useCallback(async () => {
    if (!isAPIAvailable()) return;

    const confirmed = window.confirm(
      'Execute metadata v2 migration now? This will move model folders and rewrite metadata.'
    );
    if (!confirmed) return;

    setIsExecutingMigration(true);
    setMessage(null);
    try {
      const result = await api.execute_model_migration();
      if (!result.success) {
        setMessage({ type: 'error', text: result.error ?? 'Migration execution failed.' });
        return;
      }

      const report = result.report;
      setLastExecutionReport(report);
      if (report.referential_integrity_ok) {
        setMessage({
          type: 'success',
          text: `Migration complete: ${report.completed_move_count}/${report.planned_move_count} moved or already migrated.`,
        });
      } else {
        setMessage({
          type: 'error',
          text: `Migration completed with ${report.referential_integrity_errors.length} integrity validation issue(s).`,
        });
      }
      await fetchReports();
    } catch (error) {
      logger.error('Failed to execute migration', { error });
      setMessage({ type: 'error', text: 'Migration execution failed.' });
    } finally {
      setIsExecutingMigration(false);
    }
  }, [fetchReports]);

  const handleDeleteReport = useCallback(
    async (reportPath: string) => {
      if (!isAPIAvailable()) return;

      const confirmed = window.confirm('Delete this migration report artifact entry?');
      if (!confirmed) return;

      setDeletingReportPath(reportPath);
      try {
        const result = await api.delete_model_migration_report(reportPath);
        if (!result.success) {
          setMessage({ type: 'error', text: result.error ?? 'Failed to delete report.' });
          return;
        }
        setMessage({
          type: 'success',
          text: result.removed ? 'Migration report deleted.' : 'Migration report was already removed.',
        });
        await fetchReports();
      } catch (error) {
        logger.error('Failed to delete migration report', { error, reportPath });
        setMessage({ type: 'error', text: 'Failed to delete migration report.' });
      } finally {
        setDeletingReportPath(null);
      }
    },
    [fetchReports]
  );

  const handlePruneReports = useCallback(async () => {
    if (!isAPIAvailable()) return;

    const parsed = Number.parseInt(keepLatest, 10);
    if (Number.isNaN(parsed) || parsed < 0) {
      setMessage({ type: 'error', text: 'Keep latest must be a non-negative integer.' });
      return;
    }

    setIsPruning(true);
    try {
      const result = await api.prune_model_migration_reports(parsed);
      if (!result.success) {
        setMessage({ type: 'error', text: result.error ?? 'Failed to prune migration reports.' });
        return;
      }
      setMessage({
        type: 'success',
        text: `Pruned ${result.removed} report(s); keeping latest ${result.kept}.`,
      });
      await fetchReports();
    } catch (error) {
      logger.error('Failed to prune migration reports', { error });
      setMessage({ type: 'error', text: 'Failed to prune migration reports.' });
    } finally {
      setIsPruning(false);
    }
  }, [fetchReports, keepLatest]);

  const statusIcon = useMemo(() => {
    if (lastExecutionReport === null) return null;
    if (lastExecutionReport.referential_integrity_ok) {
      return <CheckCircle2 className="w-4 h-4 text-[hsl(var(--accent-success))]" />;
    }
    return <AlertTriangle className="w-4 h-4 text-[hsl(var(--accent-error))]" />;
  }, [lastExecutionReport]);

  if (!isAPIAvailable()) {
    return null;
  }

  return (
    <div className="bg-[hsl(var(--launcher-bg-tertiary)/0.3)] rounded-lg border border-[hsl(var(--launcher-border)/0.5)]">
      <button
        onClick={() => setIsExpanded((previous) => !previous)}
        className="w-full px-4 py-3 flex items-center justify-between hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors rounded-lg"
      >
        <div className="flex items-center gap-3">
          <FileText className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
          <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
            Migration Reports
          </span>
          {isLoadingReports && <Loader2 className="w-4 h-4 animate-spin text-[hsl(var(--launcher-text-secondary))]" />}
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs text-[hsl(var(--launcher-text-tertiary))]">
            {reports.length} item{reports.length !== 1 ? 's' : ''}
          </span>
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
          ) : (
            <ChevronDown className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
          )}
        </div>
      </button>

      {isExpanded && (
        <div className="px-4 pb-4 space-y-3">
          <div className="flex flex-wrap gap-2">
            <button
              className="px-3 py-1.5 text-xs rounded bg-[hsl(var(--accent-info)/0.2)] text-[hsl(var(--launcher-text-primary))] border border-[hsl(var(--accent-info)/0.35)] hover:bg-[hsl(var(--accent-info)/0.3)] disabled:opacity-60 disabled:cursor-not-allowed"
              onClick={() => void handleGenerateDryRun()}
              disabled={isGeneratingDryRun || isExecutingMigration}
            >
              {isGeneratingDryRun ? 'Generating...' : 'Generate Dry Run'}
            </button>
            <button
              className="px-3 py-1.5 text-xs rounded bg-[hsl(var(--accent-warning)/0.2)] text-[hsl(var(--launcher-text-primary))] border border-[hsl(var(--accent-warning)/0.35)] hover:bg-[hsl(var(--accent-warning)/0.3)] disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-1"
              onClick={() => void handleExecuteMigration()}
              disabled={isExecutingMigration || isGeneratingDryRun}
            >
              <PlayCircle className="w-3 h-3" />
              {isExecutingMigration ? 'Executing...' : 'Execute Migration'}
            </button>
            <button
              className="px-3 py-1.5 text-xs rounded bg-[hsl(var(--launcher-bg-secondary)/0.8)] text-[hsl(var(--launcher-text-primary))] border border-[hsl(var(--launcher-border)/0.7)] hover:bg-[hsl(var(--launcher-bg-secondary))] disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-1"
              onClick={() => void fetchReports()}
              disabled={isLoadingReports}
            >
              <RefreshCw className="w-3 h-3" />
              Refresh
            </button>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <label
              className="text-xs text-[hsl(var(--launcher-text-secondary))]"
              htmlFor="migration-prune-keep-latest"
            >
              Keep latest
            </label>
            <input
              id="migration-prune-keep-latest"
              type="number"
              min={0}
              step={1}
              value={keepLatest}
              onChange={(event) => setKeepLatest(event.target.value)}
              className="w-24 px-2 py-1 text-xs rounded bg-[hsl(var(--launcher-bg-secondary)/0.7)] border border-[hsl(var(--launcher-border)/0.7)] text-[hsl(var(--launcher-text-primary))]"
            />
            <button
              className="px-3 py-1.5 text-xs rounded bg-[hsl(var(--launcher-bg-secondary)/0.8)] text-[hsl(var(--launcher-text-primary))] border border-[hsl(var(--launcher-border)/0.7)] hover:bg-[hsl(var(--launcher-bg-secondary))] disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-1"
              onClick={() => void handlePruneReports()}
              disabled={isPruning}
            >
              <Scissors className="w-3 h-3" />
              {isPruning ? 'Pruning...' : 'Prune'}
            </button>
          </div>

          {message && (
            <div
              className={`text-xs p-2 rounded border ${
                message.type === 'success'
                  ? 'bg-[hsl(var(--accent-success)/0.12)] border-[hsl(var(--accent-success)/0.35)] text-[hsl(var(--launcher-text-primary))]'
                  : message.type === 'error'
                    ? 'bg-[hsl(var(--accent-error)/0.12)] border-[hsl(var(--accent-error)/0.35)] text-[hsl(var(--launcher-text-primary))]'
                    : 'bg-[hsl(var(--launcher-bg-secondary)/0.6)] border-[hsl(var(--launcher-border)/0.7)] text-[hsl(var(--launcher-text-secondary))]'
              }`}
            >
              {message.text}
            </div>
          )}

          {lastDryRunReport && (
            <div className="p-3 rounded border border-[hsl(var(--launcher-border)/0.7)] bg-[hsl(var(--launcher-bg-secondary)/0.45)]">
              <div className="text-xs font-medium text-[hsl(var(--launcher-text-primary))]">
                Last Dry Run
              </div>
              <div className="mt-1 text-xs text-[hsl(var(--launcher-text-secondary))]">
                {lastDryRunReport.move_candidates} moves, {lastDryRunReport.keep_candidates} keep,{' '}
                {lastDryRunReport.collision_count} collisions, {lastDryRunReport.error_count} errors
              </div>
            </div>
          )}

          {lastExecutionReport && (
            <div className="p-3 rounded border border-[hsl(var(--launcher-border)/0.7)] bg-[hsl(var(--launcher-bg-secondary)/0.45)] space-y-2">
              <div className="flex items-center gap-2 text-xs font-medium text-[hsl(var(--launcher-text-primary))]">
                <span>Last Execution</span>
                {statusIcon}
              </div>
              <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">
                {lastExecutionReport.completed_move_count} completed, {lastExecutionReport.skipped_move_count} skipped,{' '}
                {lastExecutionReport.error_count} errors
              </div>
              <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">
                Reindexed {lastExecutionReport.reindexed_model_count} models, index count {lastExecutionReport.index_model_count}
              </div>
              <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">
                Referential integrity: {lastExecutionReport.referential_integrity_ok ? 'OK' : 'FAILED'}
              </div>
              {lastExecutionReport.referential_integrity_errors.length > 0 && (
                <div className="space-y-1">
                  <div className="text-xs text-[hsl(var(--accent-error))]">Integrity Errors</div>
                  <div className="max-h-28 overflow-y-auto space-y-1">
                    {lastExecutionReport.referential_integrity_errors.map((error, index) => (
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
          )}

          <div className="space-y-2">
            {reports.length === 0 ? (
              <div className="text-xs text-[hsl(var(--launcher-text-tertiary))]">
                No migration reports yet.
              </div>
            ) : (
              reports.map((report) => (
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
                        {formatTimestamp(report.generated_at)}
                      </span>
                    </div>
                    <button
                      className="px-2 py-1 text-xs rounded border border-[hsl(var(--accent-error)/0.35)] text-[hsl(var(--launcher-text-primary))] bg-[hsl(var(--accent-error)/0.12)] hover:bg-[hsl(var(--accent-error)/0.2)] disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-1"
                      onClick={() => void handleDeleteReport(report.json_report_path)}
                      disabled={deletingReportPath === report.json_report_path}
                    >
                      <Trash2 className="w-3 h-3" />
                      {deletingReportPath === report.json_report_path ? 'Deleting...' : 'Delete'}
                    </button>
                  </div>
                  <div className="mt-2 space-y-1 text-[11px] text-[hsl(var(--launcher-text-tertiary))]">
                    <div title={report.json_report_path}>JSON: {shortenPath(report.json_report_path)}</div>
                    <div title={report.markdown_report_path}>MD: {shortenPath(report.markdown_report_path)}</div>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
};
