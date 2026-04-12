import { Loader2 } from 'lucide-react';
import type { RemoteModelInfo } from '../types/apps';
import { formatDownloadSize } from '../utils/modelFormatters';

type DownloadOption = NonNullable<RemoteModelInfo['downloadOptions']>[number];

interface RemoteModelDownloadMenuProps {
  downloadOptions: DownloadOption[];
  hasExactDetails: boolean;
  hasFileGroups: boolean;
  isHydratingDetails: boolean;
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
    <div className="absolute right-0 top-full z-10 mt-2 min-w-[200px] rounded border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-overlay))] shadow-[0_12px_24px_hsl(var(--launcher-bg-primary)/0.6)]">
      {isHydratingDetails && !hasExactDetails ? (
        <div className="flex items-center gap-2 px-3 py-3 text-xs text-[hsl(var(--text-muted))]">
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
          Loading exact download details...
        </div>
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
    </div>
  );
}
