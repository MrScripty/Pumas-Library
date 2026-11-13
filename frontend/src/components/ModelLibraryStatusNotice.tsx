import type { ModelLibraryDiagnostics, ModelLibraryLoadStatus } from '../hooks/useModels';
import { UnavailableLibraryRecords } from './UnavailableLibraryRecords';

interface ModelLibraryStatusNoticeProps {
  libraryLoadStatus: ModelLibraryLoadStatus;
  diagnostics: ModelLibraryDiagnostics | undefined;
  hasVisibleModels: boolean;
  /** Why the download list could not be loaded; null when it loaded. */
  downloadListError: string | null;
  /** Repairs a left-out record the backend reports as a retained import. */
  onFinishRetainedImport?: (modelId: string) => Promise<void>;
}

const NOTICE_CLASS = 'rounded border border-[hsl(var(--launcher-border))] px-3 py-2 text-sm text-[hsl(var(--text-secondary))]';
const FAILURE_CLASS = 'rounded border border-[hsl(var(--accent-error)/0.35)] bg-[hsl(var(--accent-error)/0.1)] px-3 py-2 text-sm text-[hsl(var(--accent-error))]';

function formatSnapshotTime(writtenAt: string): string {
  const parsed = new Date(writtenAt);
  return Number.isNaN(parsed.getTime()) ? writtenAt : parsed.toLocaleString();
}

function describeVisibleRows(
  diagnostics: ModelLibraryDiagnostics | undefined,
  hasVisibleModels: boolean
): string {
  if (!hasVisibleModels) return '';
  if (diagnostics?.snapshotWrittenAt) {
    return ` Showing models cached at ${formatSnapshotTime(diagnostics.snapshotWrittenAt)}.`;
  }
  return ' Showing previously loaded models.';
}

/** Load state, failure reasons and cache provenance for the local library. */
export function ModelLibraryStatusNotice({
  libraryLoadStatus,
  diagnostics,
  hasVisibleModels,
  downloadListError,
  onFinishRetainedImport,
}: ModelLibraryStatusNoticeProps) {
  const visibleRows = describeVisibleRows(diagnostics, hasVisibleModels);
  const failures = [
    diagnostics?.scanError ? `Library scan failed: ${diagnostics.scanError}` : null,
    diagnostics?.searchError ? `Library search failed: ${diagnostics.searchError}` : null,
    downloadListError ? `Download list unavailable: ${downloadListError}` : null,
  ].filter((message): message is string => message !== null);
  const unavailableRecords = diagnostics?.unavailableRecords ?? [];

  return (
    <>
      {libraryLoadStatus === 'loading' && (
        <div role="status" className={NOTICE_CLASS}>
          Loading library…{visibleRows}
        </div>
      )}
      {libraryLoadStatus === 'unavailable' && (
        <div role="alert" className={NOTICE_CLASS}>
          Library unavailable. {diagnostics?.loadError ?? 'Could not load models.'}{visibleRows}
        </div>
      )}
      {failures.map((message) => (
        <div key={message} role="alert" className={FAILURE_CLASS}>
          {message}
        </div>
      ))}
      <UnavailableLibraryRecords
        records={unavailableRecords}
        onFinishRetainedImport={onFinishRetainedImport}
      />
    </>
  );
}
