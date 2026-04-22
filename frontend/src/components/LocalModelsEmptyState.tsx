import { HardDrive } from 'lucide-react';
import { EmptyState } from './ui';

interface LocalModelsEmptyStateProps {
  hasFilters: boolean;
  isChoosingExistingLibrary?: boolean;
  onChooseExistingLibrary?: (() => Promise<void> | void) | undefined;
  onClearFilters?: (() => void) | undefined;
  totalModels: number;
}

export function LocalModelsEmptyState({
  hasFilters,
  isChoosingExistingLibrary = false,
  onChooseExistingLibrary,
  onClearFilters,
  totalModels,
}: LocalModelsEmptyStateProps) {
  if (totalModels === 0 && !hasFilters && onChooseExistingLibrary) {
    return (
      <div className="flex min-h-[22rem] flex-col items-center justify-center gap-4 text-center text-[hsl(var(--text-muted))]">
        <div className="flex h-12 w-12 items-center justify-center rounded-full border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-tertiary)/0.55)]">
          <HardDrive className="h-6 w-6 opacity-70" />
        </div>
        <div className="space-y-2">
          <p className="text-base font-semibold text-[hsl(var(--text-primary))]">
            No library models found
          </p>
          <p className="max-w-md text-sm text-[hsl(var(--text-secondary))]">
            Choose an existing Pumas library root or its <code>shared-resources/models</code> folder to load your saved models into this packaged app.
          </p>
        </div>
        <button
          type="button"
          onClick={() => void onChooseExistingLibrary()}
          disabled={isChoosingExistingLibrary}
          className="rounded-md border border-[hsl(var(--accent-primary)/0.45)] bg-[hsl(var(--accent-primary)/0.14)] px-4 py-2 text-sm font-medium text-[hsl(var(--text-primary))] transition hover:bg-[hsl(var(--accent-primary)/0.22)] disabled:cursor-not-allowed disabled:opacity-60"
        >
          {isChoosingExistingLibrary ? 'Opening Library Picker...' : 'Use Existing Library'}
        </button>
      </div>
    );
  }

  return (
    <EmptyState
      icon={<HardDrive />}
      message={totalModels === 0
        ? 'No models found. Add models to your library to get started.'
        : 'No models match your filters.'}
      action={totalModels > 0 && hasFilters && onClearFilters ? {
        label: 'Clear filters',
        onClick: onClearFilters,
      } : undefined}
    />
  );
}
