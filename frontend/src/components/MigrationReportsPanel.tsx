import React, { useCallback, useEffect, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type {
  MigrationDryRunReport,
  MigrationExecutionReport,
  MigrationReportArtifact,
} from '../types/api';
import { getLogger } from '../utils/logger';
import {
  ChevronDown,
  ChevronUp,
  FileText,
  Loader2,
} from 'lucide-react';
import { MigrationReportArtifactList } from './MigrationReportArtifactList';
import { MigrationReportControls } from './MigrationReportControls';
import { MigrationReportSummaries } from './MigrationReportSummaries';
import { ConfirmationDialog } from './ConfirmationDialog';

const logger = getLogger('MigrationReportsPanel');

interface FlashMessage {
  type: 'success' | 'error' | 'info';
  text: string;
}

type PendingConfirmation =
  | { kind: 'executeMigration' }
  | { kind: 'deleteReport'; reportPath: string };

function formatTimestamp(value: string): string {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleString();
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
  const [openingPath, setOpeningPath] = useState<string | null>(null);
  const [pendingConfirmation, setPendingConfirmation] = useState<PendingConfirmation | null>(null);

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
        text: `Dry-run generated: ${report.move_candidates} moves (${report.blocked_partial_count} blocked partial), ${report.collision_count} collisions, ${report.error_count} errors.`,
      });
      await fetchReports();
    } catch (error) {
      logger.error('Failed to generate migration dry-run report', { error });
      setMessage({ type: 'error', text: 'Dry-run generation failed.' });
    } finally {
      setIsGeneratingDryRun(false);
    }
  }, [fetchReports]);

  const executeMigration = useCallback(async () => {
    if (!isAPIAvailable()) return;

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

  const handleExecuteMigration = useCallback(() => {
    setPendingConfirmation({ kind: 'executeMigration' });
  }, []);

  const deleteReport = useCallback(
    async (reportPath: string) => {
      if (!isAPIAvailable()) return;

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

  const handleDeleteReport = useCallback((reportPath: string) => {
    setPendingConfirmation({ kind: 'deleteReport', reportPath });
  }, []);

  const handleConfirmPendingAction = useCallback(() => {
    const pending = pendingConfirmation;
    setPendingConfirmation(null);

    if (!pending) {
      return;
    }

    if (pending.kind === 'executeMigration') {
      void executeMigration();
      return;
    }

    void deleteReport(pending.reportPath);
  }, [deleteReport, executeMigration, pendingConfirmation]);

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

  const handleOpenPath = useCallback(async (path: string, label: string) => {
    if (!isAPIAvailable()) return;

    setOpeningPath(path);
    try {
      const result = await api.open_path(path);
      if (!result.success) {
        setMessage({
          type: 'error',
          text: result.error ?? `Failed to open ${label} report.`,
        });
      }
    } catch (error) {
      logger.error('Failed to open migration report path', { error, path });
      setMessage({ type: 'error', text: `Failed to open ${label} report.` });
    } finally {
      setOpeningPath(null);
    }
  }, []);

  if (!isAPIAvailable()) {
    return null;
  }

  const pendingConfirmationCopy = pendingConfirmation;
  const confirmationTitle = pendingConfirmationCopy?.kind === 'executeMigration'
    ? 'Execute metadata migration'
    : 'Delete migration report';
  const confirmationMessage = pendingConfirmationCopy?.kind === 'executeMigration'
    ? 'This will move model folders and rewrite metadata.'
    : 'This removes the migration report artifact entry from the report list.';
  const confirmationLabel = pendingConfirmationCopy?.kind === 'executeMigration'
    ? 'Execute migration'
    : 'Delete report';

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
          <MigrationReportControls
            isExecutingMigration={isExecutingMigration}
            isGeneratingDryRun={isGeneratingDryRun}
            isLoadingReports={isLoadingReports}
            isPruning={isPruning}
            keepLatest={keepLatest}
            message={message}
            onExecuteMigration={handleExecuteMigration}
            onGenerateDryRun={() => void handleGenerateDryRun()}
            onKeepLatestChange={setKeepLatest}
            onPruneReports={() => void handlePruneReports()}
            onRefresh={() => void fetchReports()}
          />

          <MigrationReportSummaries
            lastDryRunReport={lastDryRunReport}
            lastExecutionReport={lastExecutionReport}
          />

          <div className="space-y-2">
            <MigrationReportArtifactList
              deletingReportPath={deletingReportPath}
              openingPath={openingPath}
              reports={reports.map((report) => ({
                ...report,
                generated_at: formatTimestamp(report.generated_at),
              }))}
              onDeleteReport={handleDeleteReport}
              onOpenPath={(path, label) => void handleOpenPath(path, label)}
            />
          </div>
        </div>
      )}

      <ConfirmationDialog
        isOpen={pendingConfirmation !== null}
        title={confirmationTitle}
        message={confirmationMessage}
        confirmLabel={confirmationLabel}
        onCancel={() => setPendingConfirmation(null)}
        onConfirm={handleConfirmPendingAction}
      />
    </div>
  );
};
