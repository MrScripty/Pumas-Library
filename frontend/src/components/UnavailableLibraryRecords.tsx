import { useState } from 'react';
import type { UnavailableCatalogRecord } from '@pumas/desktop-contract';

const FAILURE_CLASS = 'rounded border border-[hsl(var(--accent-error)/0.35)] bg-[hsl(var(--accent-error)/0.1)] px-3 py-2 text-sm text-[hsl(var(--accent-error))]';
const ACTION_CLASS = 'surface-interactive surface-interactive-hover text-[hsl(var(--text-primary))] rounded px-2 py-0.5 text-xs disabled:opacity-60 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[hsl(var(--launcher-accent-primary))]';

interface UnavailableLibraryRecordsProps {
  records: ReadonlyArray<UnavailableCatalogRecord>;
  /** Runs the backend repair for a record that reports one; rejects with the reason. */
  onFinishRetainedImport?: (modelId: string) => Promise<void>;
}

function describeFailure(error: unknown): string {
  return error instanceof Error ? error.message : 'The backend could not finish the import.';
}

/** Records the backend left out of the library, their reasons, and any repair it offers. */
export function UnavailableLibraryRecords({ records, onFinishRetainedImport }: UnavailableLibraryRecordsProps) {
  const [pending, setPending] = useState<string | null>(null);
  const [failures, setFailures] = useState<Record<string, string>>({});
  if (records.length === 0) return null;

  const finish = async (modelId: string) => {
    if (!onFinishRetainedImport) return;
    setPending(modelId);
    setFailures(({ [modelId]: _previous, ...rest }) => rest);
    try {
      await onFinishRetainedImport(modelId);
    } catch (error) {
      setFailures(current => ({ ...current, [modelId]: describeFailure(error) }));
    } finally {
      setPending(null);
    }
  };

  return (
    <div role="alert" className={FAILURE_CLASS}>
      {records.length === 1
        ? '1 library record could not be shown:'
        : `${records.length} library records could not be shown:`}
      <ul className="mt-1 list-disc space-y-1 pl-5">
        {records.map((record, index) => (
          <li key={`${index}:${record.model_id}`}>
            {record.model_id}: {record.reason}
            {record.recovery === 'finish_retained_import' && onFinishRetainedImport && (
              <>
                {' '}
                <button
                  type="button"
                  className={ACTION_CLASS}
                  disabled={pending !== null}
                  onClick={() => void finish(record.model_id)}
                >
                  {pending === record.model_id ? 'Verifying files…' : 'Verify and finish import'}
                </button>
              </>
            )}
            {failures[record.model_id] && (
              <div className="mt-0.5">Could not finish: {failures[record.model_id]}</div>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
