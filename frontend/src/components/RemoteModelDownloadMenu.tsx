import { Loader2 } from 'lucide-react';
import type { RemoteModelInfo } from '../types/apps';
import { formatDownloadSize } from '../utils/modelFormatters';

type DownloadOption = NonNullable<RemoteModelInfo['downloadOptions']>[number];

interface RemoteModelDownloadMenuProps {
  downloadOptions: DownloadOption[];
  hasExactDetails: boolean;
  hasFileGroups: boolean;
  isHydratingDetails: boolean;
  hydrationError?: string;
  onLoadDetails?: () => void;
  model: RemoteModelInfo;
  selectedGroups: Set<string>;
  selectedTotalBytes: number;
  onClearSelection: () => void;
  onCloseMenu: () => void;
  onStartDownload: (
    model: RemoteModelInfo,
    quant?: string | null,
    filenames?: string[] | null
  ) => Promise<void>;
  onToggleGroup: (label: string) => void;
  collectSelectedFilenames: () => string[];
}

export function RemoteModelDownloadMenu({
  downloadOptions,
  hasExactDetails,
  hasFileGroups,
  isHydratingDetails,
  hydrationError,
  onLoadDetails,
  model,
  selectedGroups,
  selectedTotalBytes,
  onClearSelection,
  onCloseMenu,
  onStartDownload,
  onToggleGroup,
  collectSelectedFilenames,
}: RemoteModelDownloadMenuProps) {
  return (
    <>
      {isHydratingDetails && !hasExactDetails ? (
        <div className="flex items-center gap-2 px-3 py-3 text-xs text-[hsl(var(--text-muted))]">
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
          Loading exact download details...
        </div>
      ) : hydrationError || (!hasExactDetails && onLoadDetails) ? (
        <div className="space-y-2 px-3 py-3 text-xs text-[hsl(var(--text-secondary))]">
          {hydrationError ? <p role="alert">{hydrationError}</p> : <p>Load download details to choose files.</p>}
          {onLoadDetails && <button type="button" onClick={onLoadDetails}
            className="text-[hsl(var(--launcher-accent-primary))] hover:underline">
            {hydrationError ? 'Retry download details' : 'Load download details'}
          </button>}
        </div>
      ) : downloadOptions.length === 0 ? (
        <p className="px-3 py-3 text-xs text-[hsl(var(--text-muted))]">No downloadable files were found.</p>
      ) : hasFileGroups ? (
        <>
          {downloadOptions.map((option) => {
            const label = option.fileGroup?.label ?? option.quant;
            const shardCount = option.fileGroup?.shardCount ?? 1;
            const checked = selectedGroups.has(label);
            return (
              <label
                key={label}
                className="flex w-full cursor-pointer items-center gap-2 px-3 py-1.5 text-xs text-[hsl(var(--text-secondary))] transition-colors hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
              >
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={() => onToggleGroup(label)}
                  className="accent-[hsl(var(--launcher-accent-primary))]"
                />
                <span className="min-w-0 flex-1 truncate" title={label}>
                  {label}
                  {shardCount > 1 ? ` (${shardCount} shards)` : ''}
                </span>
                <span className="flex-shrink-0 text-[hsl(var(--text-muted))]">
                  {typeof option.sizeBytes === 'number' && option.sizeBytes > 0
                    ? formatDownloadSize(option.sizeBytes)
                    : ''}
                </span>
              </label>
            );
          })}
          <div className="mt-1 flex flex-col gap-1.5 border-t border-[hsl(var(--launcher-border))] px-3 pb-2 pt-1">
            <button
              type="button"
              disabled={selectedGroups.size === 0}
              onClick={() => {
                onCloseMenu();
                const filenames = collectSelectedFilenames();
                if (filenames.length > 0) {
                  void onStartDownload(model, null, filenames);
                }
                onClearSelection();
              }}
              className="w-full rounded bg-[hsl(var(--launcher-accent-primary)/0.15)] py-1.5 text-xs font-medium text-[hsl(var(--launcher-accent-primary))] transition-colors hover:bg-[hsl(var(--launcher-accent-primary)/0.25)] disabled:cursor-not-allowed disabled:opacity-40"
            >
              Download selected
              {selectedTotalBytes > 0 ? ` (${formatDownloadSize(selectedTotalBytes)})` : ''}
            </button>
            <button
              type="button"
              onClick={() => {
                onCloseMenu();
                void onStartDownload(model, null, null);
                onClearSelection();
              }}
              className="w-full py-1 text-[10px] text-[hsl(var(--text-muted))] transition-colors hover:text-[hsl(var(--text-secondary))]"
            >
              All files
              {model.totalSizeBytes ? ` (${formatDownloadSize(model.totalSizeBytes)})` : ''}
            </button>
          </div>
        </>
      ) : (
        <>
          {downloadOptions.map((option) => (
            <button
              key={option.quant}
              type="button"
              onClick={() => {
                onCloseMenu();
                void onStartDownload(model, option.quant);
              }}
              className="w-full px-3 py-2 text-left text-xs text-[hsl(var(--text-secondary))] transition-colors hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
            >
              {option.quant}
              {typeof option.sizeBytes === 'number' && option.sizeBytes > 0
                ? ` (${formatDownloadSize(option.sizeBytes)})`
                : ' (Unknown)'}
            </button>
          ))}
          <button
            type="button"
            onClick={() => {
              onCloseMenu();
              void onStartDownload(model, null);
            }}
            className="w-full px-3 py-2 text-left text-xs text-[hsl(var(--text-secondary))] transition-colors hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
          >
            All files
            {model.totalSizeBytes ? ` (${formatDownloadSize(model.totalSizeBytes)})` : ''}
          </button>
        </>
      )}
    </>
  );
}
