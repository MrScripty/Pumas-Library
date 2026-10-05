import { useEffect, useRef, useState } from 'react';
import { ModalDialog } from './ui/ModalDialog';
import { useS3ModelImport, type S3ImportDraft } from '../hooks/useS3ModelImport';
import type { S3ImportOutcome } from '../generated/desktop-contract';

const fields = [
  ['endpoint', 'HTTPS endpoint origin', 'https://s3.example.com'],
  ['region', 'Region', ''],
  ['bucket', 'Bucket', ''],
  ['key', 'Exact object key', ''],
  ['version_id', 'Immutable VersionId', ''],
  ['sha256', 'Expected SHA-256', '64 hexadecimal digits'],
  ['filename', 'Library GGUF filename', 'weights.gguf'],
  ['family', 'Model family', ''],
  ['official_name', 'Model name', ''],
] as const;
const inputClass = 'w-full rounded border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-tertiary))] px-3 py-2 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[hsl(var(--launcher-accent-primary))]';
const buttonClass = 'rounded border border-[hsl(var(--launcher-border))] px-3 py-2 disabled:opacity-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[hsl(var(--launcher-accent-primary))]';
function canStart(snapshot: S3ImportOutcome | null): boolean {
  if (snapshot?.status === 'idle') return true;
  if (snapshot?.status === 'rejected') return snapshot.error.class !== 'conflict';
  if (snapshot?.status !== 'finished') return false;
  return snapshot.result.status === 'completed' || !snapshot.result.retained_work;
}
const phaseLabels = {
  pending: 'Waiting for source selection', selecting: 'Selecting the pinned object',
  acquiring: 'Acquiring and verifying bytes', cancelling: 'Waiting for cancellation to settle',
  finalizing: 'Registering the verified model', completed: 'Observing the final result',
  cancelled: 'Observing cancellation', failed: 'Observing the failure result',
  interrupted: 'Operation interrupted; retained work may require reconciliation',
};
export function S3ModelImportDialog({ onClose, onImported }: { onClose: () => void; onImported?: () => void }) {
  const closeButton = useRef<HTMLButtonElement>(null);
  const accessKey = useRef<HTMLInputElement>(null);
  const secretKey = useRef<HTMLInputElement>(null);
  const sessionToken = useRef<HTMLInputElement>(null);
  const [authenticated, setAuthenticated] = useState(false);
  const clearCredentials = () => {
    for (const input of [accessKey.current, secretKey.current, sessionToken.current]) {
      if (input) input.value = '';
    }
  };
  useEffect(() => {
    // Capture owned nodes: React may clear refs before passive unmount cleanup.
    const inputs = [accessKey.current, secretKey.current, sessionToken.current];
    return () => { for (const input of inputs) if (input) input.value = ''; };
  }, [authenticated]);
  const close = () => { clearCredentials(); onClose(); };
  const [draft, setDraft] = useState<S3ImportDraft>({ endpoint: '', region: '', bucket: '', addressing: 'path', key: '', version_id: '', filename: 'weights.gguf', sha256: '', family: '', official_name: '' });
  const { snapshot, error, commandBusy, start, startAuthenticated, cancel, observeAgain } = useS3ModelImport(onImported);
  const editable = canStart(snapshot) && !commandBusy;
  const active = snapshot?.status === 'running';
  const cancellable = active && ['pending', 'selecting', 'acquiring'].includes(snapshot.progress.phase);
  return (
    <ModalDialog isOpen ariaLabelledBy="s3-import-title" ariaDescribedBy="s3-import-description" onClose={close}
      initialFocusRef={closeButton} contentClassName="w-full max-w-2xl max-h-[90vh] overflow-y-auto rounded-xl border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary))] p-6 text-[hsl(var(--launcher-text-primary))]">
      <h2 id="s3-import-title" className="text-lg font-semibold">Import from S3</h2>
      <p id="s3-import-description" className="my-3 text-sm">Import one pinned GGUF object over HTTPS. Anonymous access is the default. Supply the exact VersionId and expected SHA-256; a key alone is insufficient.</p>
      <form autoComplete="off" onSubmit={event => {
        event.preventDefault();
        if (!editable) return;
        if (!authenticated) { void start(draft); return; }
        const credentials = { access_key_id: accessKey.current?.value ?? '',
          secret_access_key: secretKey.current?.value ?? '', session_token: sessionToken.current?.value || null };
        clearCredentials();
        void startAuthenticated(draft, credentials);
      }}>
        <fieldset disabled={!editable} className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          {fields.map(([key, label, placeholder]) => (
            <label key={key} className="block text-sm">{label}
              <input className={inputClass} name={key} required
                type={key === 'endpoint' ? 'url' : 'text'} placeholder={placeholder}
                maxLength={key === 'key' ? 1024 : key === 'endpoint' || key === 'version_id' ? 4096 : key === 'sha256' ? 64 : 255}
                value={draft[key]} onChange={event => setDraft(previous => ({ ...previous, [key]: event.target.value }))} />
            </label>
          ))}
          <label className="block text-sm">Addressing style
            <select className={inputClass} value={draft.addressing} onChange={event => setDraft(previous => ({ ...previous, addressing: event.target.value }))}>
              <option value="path">Path</option><option value="virtual_hosted">Virtual hosted</option>
            </select>
          </label>
        </fieldset>
        <fieldset disabled={!editable} className="mt-4">
          <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={authenticated}
            onChange={event => { clearCredentials(); setAuthenticated(event.target.checked); }} />Use one-use credentials</label>
          {authenticated && <div className="mt-3 grid grid-cols-1 gap-3">
            <p className="text-sm">Credentials are used for this import only. Inputs clear on submit or close; credentials are not saved or refreshed.</p>
            <label className="block text-sm">Access key ID<input ref={accessKey} type="password" autoComplete="off" maxLength={4096} required className={inputClass} /></label>
            <label className="block text-sm">Secret access key<input ref={secretKey} type="password" autoComplete="off" maxLength={4096} required className={inputClass} /></label>
            <label className="block text-sm">Session token (optional)<input ref={sessionToken} type="password" autoComplete="off" maxLength={4096} className={inputClass} /></label>
          </div>}
        </fieldset>
        {commandBusy && <p role="status" className="mt-4">Waiting for command acknowledgement…</p>}
        {snapshot === null && !error && <p role="status" className="mt-4">Checking source import availability…</p>}
        {snapshot?.status === 'unavailable' && <p role="status" className="mt-4">S3 import is unavailable in this backend build or connection. Use a backend built with the S3 feature.</p>}
        {snapshot?.status === 'running' && <div role="status" className="mt-4">
          <p>{phaseLabels[snapshot.progress.phase]}</p>
          <p>{snapshot.progress.downloaded_for_current_file} bytes observed for the current file. This count can reset and does not prove verification.</p>
          <p className="text-xs">Operation: {snapshot.operation_id}</p>
        </div>}
        {snapshot?.status === 'not_found' && <p role="alert" className="mt-4">The operation result is unavailable. Check the library and reconcile retained work before importing this source again. Operation: {snapshot.operation_id}</p>}
        {snapshot?.status === 'finished' && <div role={snapshot.result.status === 'completed' ? 'status' : 'alert'} className="mt-4">
          {snapshot.result.status === 'completed' ? <p>Model registered: {snapshot.result.model_id}</p>
            : snapshot.result.status === 'cancelled' ? <p>Import cancelled.</p>
              : <><p>{snapshot.result.error.message}</p>{snapshot.result.published_model_id && <p>Publication may exist for {snapshot.result.published_model_id}; verify the library before retrying.</p>}</>}
          {snapshot.result.status !== 'completed' && snapshot.result.retained_work && <p>Input or publication custody may remain. Reconcile retained work before retrying; this dialog will not resubmit it.</p>}
        </div>}
        {error && <p role="alert" className="mt-4">{error}</p>}
        <p className="my-4 text-sm">Closing this dialog leaves admitted work running. Reopen it to observe the result. Cancellation is available until registration starts.</p>
        <div className="flex flex-wrap gap-2">
          <button className={buttonClass} type="submit" disabled={!editable}>Import pinned object</button>
          <button className={buttonClass} type="button" disabled={!cancellable || commandBusy} onClick={() => { void cancel(); }}>Cancel import</button>
          <button className={buttonClass} type="button" disabled={commandBusy} onClick={observeAgain}>Observe again</button>
          <button ref={closeButton} className={buttonClass} type="button" onClick={close}>Close</button>
        </div>
      </form>
    </ModalDialog>
  );
}
