import { useCallback, useEffect, useId, useRef, useState, type RefObject } from 'react';
import type { DownloadLocationReport, DownloadResolutionRequest } from '../generated/desktop-contract';
import { modelsAPI } from '../api/models';
import { ModalDialog } from './ui';

const buttonClass = 'surface-interactive surface-interactive-hover text-[hsl(var(--text-primary))] rounded px-3 py-2 disabled:opacity-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[hsl(var(--launcher-accent-primary))]';

interface Props {
  downloadId: string;
  name: string;
  onClose: () => void;
  onResolved: () => Promise<void>;
  restoreFocusFallbackRef?: RefObject<HTMLElement | null>;
}

export function DownloadLocationResolutionDialog({ downloadId, name, onClose, onResolved, restoreFocusFallbackRef }: Props) {
  const titleId = useId();
  const closeRef = useRef<HTMLButtonElement>(null);
  const [report, setReport] = useState<DownloadLocationReport | null>(null);
  const [selectedId, setSelectedId] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef<DownloadResolutionRequest | null>(null);
  const generationRef = useRef(0);

  const inspect = useCallback(async () => {
    const generation = ++generationRef.current;
    setBusy(true);
    setError(null);
    try {
      const next = await modelsAPI.inspectRetainedDownload(downloadId);
      if (generation !== generationRef.current) return;
      setReport(next);
      setSelectedId(next.candidates.length === 1 ? next.candidates[0]?.modelId ?? '' : '');
      requestRef.current = next.resolutionRequest ?? null;
    } catch {
      if (generation === generationRef.current) setError('Could not inspect this download. Files and download history are retained.');
    } finally {
      if (generation === generationRef.current) setBusy(false);
    }
  }, [downloadId]);

  useEffect(() => {
    void inspect();
    return () => { generationRef.current += 1; };
  }, [inspect]);

  const candidate = report?.candidates.find(item => item.modelId === selectedId);
  const canResolve = Boolean(report && report.attemptId && candidate
    && ['existing_local_candidate', 'ambiguous_candidates'].includes(report.status));
  const canConfirmRetry = report?.status === 'publication_uncertain' && Boolean(requestRef.current);

  async function resolve() {
    if (!report || (!canResolve && !canConfirmRetry) || busy) return;
    const request = requestRef.current ?? (candidate && report.attemptId ? {
      downloadId, attemptId: report.attemptId, candidateModelId: candidate.modelId,
      expectedObservation: candidate.observation, operationId: crypto.randomUUID(),
      acceptance: 'keep_existing_local_files' as const, acceptStorageUpgrade: report.requiresFormatUpgrade,
    } : null);
    if (!request) return;
    requestRef.current = request;
    setBusy(true);
    setError(null);
    try {
      const next = await modelsAPI.resolveRetainedDownload(request);
      setReport(next);
      if (!['resolved', 'publication_uncertain'].includes(next.status)) requestRef.current = null;
      if (next.status === 'resolved') {
        try { await onResolved(); }
        catch { setError('The old failure was resolved, but the library view could not refresh. Refresh the library to see the result.'); }
      }
    } catch {
      setError('The resolution response was unavailable. Retry this same confirmation to check its outcome; existing files are retained.');
    } finally { setBusy(false); }
  }

  return <ModalDialog isOpen onClose={onClose} ariaLabelledBy={titleId}
    initialFocusRef={closeRef} restoreFocusFallbackRef={restoreFocusFallbackRef} dismissDisabled={busy}
    contentClassName="surface-overlay text-[hsl(var(--text-primary))] max-w-xl rounded border border-[hsl(var(--border-default))] p-5">
    <div className="space-y-3 text-sm">
      <h2 id={titleId}>Recover download: {name}</h2>
      {busy && <p role="status">Checking recovery information…</p>}
      {error && <p role="alert">{error}</p>}
      {report && <>
        <p role="status">{report.message}</p>
        {report.originalDestination && <p className="break-all">Original destination: {report.originalDestination}</p>}
        {report.status !== 'resolved' && <>
          <p>Keeping local files resolves the old failed attempt. It does not verify their historical source, file integrity, or ability to run.</p>
          {report.requiresFormatUpgrade && <p role="note">Resolving this download upgrades this library’s HF storage format. Older applications that open the library directly will no longer be able to read it until their Pumas dependency is updated.</p>}
        </>}
        {report.candidates.length > 0 && <fieldset disabled={busy || report.status === 'resolved' || Boolean(requestRef.current)}>
          <legend>Local files to keep</legend>
          {report.candidates.map(item => <label key={item.modelId} className="block rounded border border-[hsl(var(--launcher-border))] p-2">
            <input type="radio" name={titleId} value={item.modelId} checked={item.modelId === selectedId}
              onChange={() => setSelectedId(item.modelId)} />{' '}{item.displayName || item.modelId}
            <span className="block break-all">{item.relativeDestination}</span>
            <span>{item.selectedFileCount} selected files · {(item.sizeBytes / 1e9).toFixed(2)} GB</span>
          </label>)}
        </fieldset>}
      </>}
      <div className="flex gap-2">
        <button className={buttonClass} disabled={busy || (!canResolve && !canConfirmRetry)} onClick={() => { void resolve(); }}>
          {requestRef.current ? 'Retry confirmation' : 'Keep existing files and resolve failure'}
        </button>
        <button className={buttonClass} disabled={busy || Boolean(requestRef.current && report?.status !== 'resolved')}
          onClick={() => { void inspect(); }}>Inspect again</button>
        <button ref={closeRef} className={buttonClass} disabled={busy} onClick={onClose}>{report?.status === 'resolved' ? 'Close' : 'Cancel'}</button>
      </div>
    </div>
  </ModalDialog>;
}
