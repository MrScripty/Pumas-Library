import { useRef, useState } from 'react';
import type { DownloadStatus } from '../hooks/modelDownloadState';
import { DownloadLocationResolutionDialog } from './DownloadLocationResolutionDialog';

export function RetainedDownloadFailures({ downloads, onResolved }: {
  downloads: Record<string, DownloadStatus>;
  onResolved: () => Promise<void>;
}) {
  const failures = Object.values(downloads).filter(item => item.status === 'error');
  const [selected, setSelected] = useState<{ downloadId: string; name: string } | null>(null);
  const regionRef = useRef<HTMLElement>(null);
  if (failures.length === 0 && !selected) return null;
  return <section aria-label="Failed download recovery" ref={regionRef} tabIndex={-1}
    className="surface-low text-[hsl(var(--text-primary))] space-y-2 rounded border border-[hsl(var(--border-default))] p-3 text-sm">
    {failures.length > 0 && <>
      <p className="text-[hsl(var(--text-secondary))]">Failed downloads may have retained files at a different library location. Inspect them before starting another download.</p>
      {failures.map(item => <div key={item.downloadId} className="flex items-center justify-between gap-3">
        <span>{item.modelName || item.repoId || item.downloadId}</span>
        <button className="surface-interactive surface-interactive-hover text-[hsl(var(--text-primary))] rounded px-3 py-1 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[hsl(var(--launcher-accent-primary))]"
          onClick={() => setSelected({ downloadId: item.downloadId, name: item.modelName || item.repoId || item.downloadId })}>
          Inspect existing files
        </button>
      </div>)}
    </>}
    {selected && <DownloadLocationResolutionDialog key={selected.downloadId} {...selected}
      onClose={() => setSelected(null)} onResolved={onResolved} restoreFocusFallbackRef={regionRef} />}
  </section>;
}
