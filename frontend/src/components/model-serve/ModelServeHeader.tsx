import { ArrowLeft } from 'lucide-react';
import type { ModelInfo } from '../../types/apps';

type ModelServeHeaderProps = {
  isDialogMode: boolean;
  model: ModelInfo;
  onBack?: () => void;
  onClose: () => void;
};

export function ModelServeHeader({
  isDialogMode,
  model,
  onBack,
  onClose,
}: ModelServeHeaderProps) {
  return (
    <div className="mb-4 flex items-start justify-between gap-3">
      <div className="min-w-0">
        {onBack && !isDialogMode && (
          <button
            type="button"
            onClick={onBack}
            className="mb-3 inline-flex items-center gap-1.5 rounded-md border border-[hsl(var(--border-default))] px-2.5 py-1.5 text-xs text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))]"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
            Back
          </button>
        )}
        <h2 id="model-serve-title" className="text-sm font-semibold text-[hsl(var(--text-primary))]">
          Serve {model.name}
        </h2>
        <p className="mt-1 truncate text-xs text-[hsl(var(--text-tertiary))]">{model.id}</p>
      </div>
      {isDialogMode && (
        <button
          type="button"
          onClick={onClose}
          className="rounded px-2 py-1 text-xs text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))]"
        >
          Close
        </button>
      )}
    </div>
  );
}
