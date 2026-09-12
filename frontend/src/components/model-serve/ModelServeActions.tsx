import type { ModelServeError } from '../../types/api-serving';
import type { ServingControlObservation, ServingControlStatus } from '../../hooks/useServingStatus';
import type { ModelServingActionPhase } from './useModelServingActions';
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
  actionPhase: ModelServingActionPhase;
  controlObservation: ServingControlObservation;
  isLoading: boolean;
  isUnavailable: boolean;
  onClose: () => void;
  onServe: () => void;
  onUnload: () => void;
  servedStatus: ServingControlStatus | null;
};

export function ModelServeActions({
  isDialogMode,
  actionPhase,
  controlObservation,
  isLoading,
  isUnavailable,
  onClose,
  onServe,
  onUnload,
  servedStatus,
}: ModelServeActionsProps) {
  const isLoaded = Boolean(servedStatus) && controlObservation.kind === 'known';
  const label = isLoaded
    ? actionPhase === 'stopping' ? 'Stopping...' : 'Stop serving'
    : isLoading
      ? 'Loading'
    : isUnavailable || controlObservation.kind === 'unavailable' || actionPhase === 'uncertain'
      ? 'Serving status unavailable'
      : actionPhase === 'starting' ? 'Starting...' : 'Start serving';
  const disabled = isLoaded
    ? actionPhase === 'stopping'
    : isLoading || isUnavailable || controlObservation.kind === 'unavailable' || actionPhase !== 'idle';

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
        onClick={isLoaded ? onUnload : onServe}
        disabled={disabled}
        className="rounded bg-[hsl(var(--accent-primary))] px-3 py-1.5 text-sm text-[hsl(0_0%_10%)] disabled:opacity-50"
      >
        {label}
      </button>
    </div>
  );
}
