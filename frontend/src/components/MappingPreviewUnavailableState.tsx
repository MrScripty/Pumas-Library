import { RefreshCw, XCircle } from 'lucide-react';

export function MappingPreviewUnavailableState({
  error,
  onRetry,
}: {
  error: string | null;
  onRetry: () => void;
}) {
  if (!error) {
    return null;
  }

  return (
    <div className="bg-[hsl(var(--launcher-bg-tertiary)/0.3)] rounded-lg border border-[hsl(var(--accent-error)/0.5)] p-4">
      <div className="flex items-start gap-3">
        <XCircle className="w-5 h-5 text-[hsl(var(--accent-error))] flex-shrink-0 mt-0.5" />
        <div className="flex-1">
          <div className="text-sm font-medium text-[hsl(var(--accent-error))]">
            Failed to load mapping preview
          </div>
          <div className="text-xs text-[hsl(var(--launcher-text-secondary))] mt-1">
            {error}
          </div>
          <button
            onClick={onRetry}
            className="mt-3 flex items-center gap-2 px-3 py-1.5 text-xs bg-[hsl(var(--launcher-bg-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] rounded transition-colors"
          >
            <RefreshCw className="w-3 h-3" />
            Retry
          </button>
        </div>
      </div>
    </div>
  );
}
