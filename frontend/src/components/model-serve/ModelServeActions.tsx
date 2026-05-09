import type { ModelServeError, ServedModelStatus } from '../../types/api-serving';
import { formatServeError } from './modelServeHelpers';

export function ModelServeFeedback({
  message,
  serveError,
}: {
  message: string | null;
  serveError: ModelServeError | null;
}) {
  const statusMessage = formatServeError(serveError) ?? message;

  if (!statusMessage) {
    return null;
  }

  return (
    <div className="mt-3 rounded border border-[hsl(var(--border-default))] px-3 py-2 text-xs text-[hsl(var(--text-secondary))]">
      {statusMessage}
    </div>
  );
}

type ModelServeActionsProps = {
  isDialogMode: boolean;
  isSubmitting: boolean;
  onClose: () => void;
  onServe: () => void;
  onUnload: () => void;
  servedStatus: ServedModelStatus | null;
};

export function ModelServeActions({
  isDialogMode,
  isSubmitting,
  onClose,
  onServe,
  onUnload,
  servedStatus,
}: ModelServeActionsProps) {
  return (
    <div className="mt-4 flex justify-end gap-2">
      {isDialogMode && (
        <button
          type="button"
          onClick={onClose}
          className="rounded px-3 py-1.5 text-sm text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))]"
        >
          Cancel
        </button>
      )}
      <button
        type="button"
        onClick={onServe}
        disabled={isSubmitting}
        className="rounded bg-[hsl(var(--accent-primary))] px-3 py-1.5 text-sm text-[hsl(0_0%_10%)] disabled:opacity-50"
      >
        {isSubmitting ? 'Starting...' : 'Start serving'}
      </button>
      <button
        type="button"
        onClick={onUnload}
        disabled={!servedStatus || isSubmitting}
        className="rounded border border-[hsl(var(--border-default))] px-3 py-1.5 text-sm text-[hsl(var(--text-primary))] disabled:opacity-50"
      >
        Unload
      </button>
    </div>
  );
}
