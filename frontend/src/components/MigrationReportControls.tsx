import { PlayCircle, RefreshCw, Scissors } from 'lucide-react';

interface MigrationReportControlsProps {
  isExecutingMigration: boolean;
  isGeneratingDryRun: boolean;
  isLoadingReports: boolean;
  isPruning: boolean;
  keepLatest: string;
  message: {
    type: 'success' | 'error' | 'info';
    text: string;
  } | null;
  onExecuteMigration: () => void;
  onGenerateDryRun: () => void;
  onKeepLatestChange: (value: string) => void;
  onPruneReports: () => void;
  onRefresh: () => void;
}

export function MigrationReportControls({
  isExecutingMigration,
  isGeneratingDryRun,
  isLoadingReports,
  isPruning,
  keepLatest,
  message,
  onExecuteMigration,
  onGenerateDryRun,
  onKeepLatestChange,
  onPruneReports,
  onRefresh,
}: MigrationReportControlsProps) {
  return (
    <>
      <div className="flex flex-wrap gap-2">
        <button
          className="px-3 py-1.5 text-xs rounded bg-[hsl(var(--accent-info)/0.2)] text-[hsl(var(--launcher-text-primary))] border border-[hsl(var(--accent-info)/0.35)] hover:bg-[hsl(var(--accent-info)/0.3)] disabled:opacity-60 disabled:cursor-not-allowed"
          onClick={onGenerateDryRun}
          disabled={isGeneratingDryRun || isExecutingMigration}
        >
          {isGeneratingDryRun ? 'Generating...' : 'Generate Dry Run'}
        </button>
        <button
          className="px-3 py-1.5 text-xs rounded bg-[hsl(var(--accent-warning)/0.2)] text-[hsl(var(--launcher-text-primary))] border border-[hsl(var(--accent-warning)/0.35)] hover:bg-[hsl(var(--accent-warning)/0.3)] disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-1"
          onClick={onExecuteMigration}
          disabled={isExecutingMigration || isGeneratingDryRun}
        >
          <PlayCircle className="w-3 h-3" />
          {isExecutingMigration ? 'Executing...' : 'Execute Migration'}
        </button>
        <button
          className="px-3 py-1.5 text-xs rounded bg-[hsl(var(--launcher-bg-secondary)/0.8)] text-[hsl(var(--launcher-text-primary))] border border-[hsl(var(--launcher-border)/0.7)] hover:bg-[hsl(var(--launcher-bg-secondary))] disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-1"
          onClick={onRefresh}
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
          onChange={(event) => onKeepLatestChange(event.target.value)}
          className="w-24 px-2 py-1 text-xs rounded bg-[hsl(var(--launcher-bg-secondary)/0.7)] border border-[hsl(var(--launcher-border)/0.7)] text-[hsl(var(--launcher-text-primary))]"
        />
        <button
          className="px-3 py-1.5 text-xs rounded bg-[hsl(var(--launcher-bg-secondary)/0.8)] text-[hsl(var(--launcher-text-primary))] border border-[hsl(var(--launcher-border)/0.7)] hover:bg-[hsl(var(--launcher-bg-secondary))] disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-1"
          onClick={onPruneReports}
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
    </>
  );
}
