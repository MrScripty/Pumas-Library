import { S3TransferRetryPanel } from './S3TransferRetryPanel';
import { useEffect, useRef, useState } from 'react';
import { ModalDialog } from './ui/ModalDialog';
import { S3PersistedImportsPanel } from './S3PersistedImportsPanel';
import { useS3PrefixDiscovery } from '../hooks/useS3PrefixDiscovery';
import { useS3ModelImport, type S3ImportDraft } from '../hooks/useS3ModelImport';
import type { S3PinnedFileParams, S3ImportOutcome } from '../generated/desktop-contract';

const fields = [
  ['endpoint', 'HTTPS endpoint origin', 'https://s3.example.com'],
  ['region', 'Region', ''],
  ['bucket', 'Bucket', ''],
  ['key', 'Exact object key', ''],
  ['version_id', 'Immutable VersionId', ''],
  ['sha256', 'Expected SHA-256', '64 hexadecimal digits'],
  ['filename', 'Primary weight logical path', 'model.safetensors or unet/diffusion_pytorch_model.safetensors'],
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
  const nextRow = useRef(0);
  const [auxiliaries, setAuxiliaries] = useState<Array<S3PinnedFileParams & {id: number}>>([]);
  const [authenticated, setAuthenticated] = useState(false);
  const selectionSource = useRef<string | null>(null);
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
  const discovery = useS3PrefixDiscovery();
  const [prefix, setPrefix] = useState('models/');
  const [retryCredentialEpoch, setRetryCredentialEpoch] = useState(0);
  const close = () => { setRetryCredentialEpoch(epoch => epoch + 1); clearCredentials(); discovery.reset(); onClose(); };
  const [draft, setDraft] = useState<S3ImportDraft>({ endpoint: '', region: '', bucket: '', addressing: 'path', key: '', version_id: '', filename: 'weights.gguf', sha256: '', family: '', official_name: '' });
  const { snapshot, bundleProgress, error, commandBusy, start, startBundle, startAuthenticated, cancel, observeAgain, retry } = useS3ModelImport(onImported, true);
  const sourceIdentity = JSON.stringify([draft.endpoint, draft.region, draft.bucket, draft.addressing]);
  useEffect(() => {
    if (selectionSource.current !== null && selectionSource.current !== sourceIdentity) {
      selectionSource.current = null;
      setDraft(previous => ({...previous, key: '', version_id: '', filename: '', sha256: ''}));
      setAuxiliaries([]);
    }
  }, [sourceIdentity]);
  const editable = canStart(snapshot) && !commandBusy && !discovery.busy;
  useEffect(() => { discovery.reset(); }, [draft.endpoint, draft.region, draft.bucket, draft.addressing, prefix, authenticated, discovery.reset]);
  const active = snapshot?.status === 'running';
  const cancellable = active && ['pending', 'selecting', 'acquiring'].includes(snapshot.progress.phase);
  return (
    <ModalDialog isOpen ariaLabelledBy="s3-import-title" ariaDescribedBy="s3-import-description" onClose={close}
      initialFocusRef={closeButton} contentClassName="w-full max-w-2xl max-h-[90vh] overflow-y-auto rounded-xl border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary))] p-6 text-[hsl(var(--launcher-text-primary))]">
      <h2 id="s3-import-title" className="text-lg font-semibold">Import from S3</h2>
      <p id="s3-import-description" className="my-3 text-sm">Import a pinned model file or complete model package over HTTPS. GGUF and safetensors are qualified by the shared importer. Anonymous access is the default. Supply every exact VersionId and expected SHA-256; a key alone is insufficient.</p>
      <S3PersistedImportsPanel />
      <S3TransferRetryPanel outcome={snapshot} busy={commandBusy} onRetry={retry} clearEpoch={retryCredentialEpoch} />
      <form autoComplete="off" onSubmit={event => {
        event.preventDefault();
        if (!editable || (selectionSource.current !== null && selectionSource.current !== sourceIdentity)) return;
        const files = auxiliaries.map(({key,version_id,logical_path,sha256}) => ({key,version_id,logical_path,sha256}));
        if (!authenticated) { if (files.length) void startBundle(draft, files); else void start(draft); return; }
        const credentials = { access_key_id: accessKey.current?.value ?? '',
          secret_access_key: secretKey.current?.value ?? '', session_token: sessionToken.current?.value || null };
        clearCredentials();
        if (files.length) void startBundle(draft, files, credentials);
        else void startAuthenticated(draft, credentials);
      }}>
        <fieldset disabled={!editable} className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          {fields.map(([key, label, placeholder]) => (
            <label key={key} className="block text-sm">{label}
              <input className={inputClass} name={key} required
                type={key === 'endpoint' ? 'url' : 'text'} placeholder={placeholder}
                maxLength={key === 'key' || key === 'filename' ? 1024 : key === 'endpoint' || key === 'version_id' ? 4096 : key === 'sha256' ? 64 : 255}
                value={draft[key]} onChange={event => setDraft(previous => ({ ...previous, [key]: event.target.value }))} />
            </label>
          ))}
          <label className="block text-sm">Addressing style
            <select className={inputClass} value={draft.addressing} onChange={event => setDraft(previous => ({ ...previous, addressing: event.target.value }))}>
              <option value="path">Path</option><option value="virtual_hosted">Virtual hosted</option>
            </select>
          </label>
        </fieldset>
        <section aria-label="S3 prefix discovery" className="my-4 space-y-2">
          <label className="block text-sm">Object prefix<input className={inputClass} maxLength={1024} value={prefix}
            disabled={!editable} onChange={event => setPrefix(event.target.value)} /></label>
          <p className="text-sm">Discover up to 32 objects within 15 seconds. Results pin observed versions; ETags are not SHA-256. Choose files and provide output paths and trusted SHA-256 values before importing.</p>
          <button type="button" className={buttonClass} disabled={!editable} onClick={() => {
            const credentials = authenticated ? {access_key_id: accessKey.current?.value ?? '',
              secret_access_key: secretKey.current?.value ?? '', session_token: sessionToken.current?.value || null} : undefined;
            clearCredentials(); void discovery.start(draft, prefix, credentials);
          }}>Discover prefix</button>
          {discovery.busy && <button type="button" className={buttonClass} onClick={() => { void discovery.cancel(); }}>Cancel discovery</button>}
          {discovery.error && <><p role="alert">{discovery.error}</p><button type="button" className={buttonClass} onClick={discovery.observeAgain}>Observe discovery again</button></>}
          {discovery.snapshot?.status === 'running' && <p role="status">Discovering the complete prefix and pinning versions…</p>}
          {discovery.snapshot?.status === 'incomplete' && <p role="status">Discovery exceeded its capacity. Narrow the prefix; no partial selection is available.</p>}
          {discovery.snapshot?.status === 'deadline' && <p role="status">Discovery reached its deadline. No selection is available.</p>}
          {discovery.snapshot?.status === 'cancelled' && <p role="status">Discovery cancelled. No files were imported.</p>}
          {discovery.snapshot?.status === 'unavailable' && <p role="status">Discovery is unavailable for this source or backend. No selection is available.</p>}
          {discovery.snapshot?.status === 'rejected' && <p role="status">Discovery was refused. Check the source and observe any existing S3 operation.</p>}
          {discovery.snapshot?.status === 'complete' && <>
            <p role="status">Complete discovery: {discovery.snapshot.objects.length} objects across {discovery.snapshot.pages} pages. Discovery does not import files.</p>
            <ul>{discovery.snapshot.objects.map(object => <li key={object.key} className="my-2 break-all">
              <span>{object.key} — version {object.version_id}, {object.size_bytes} bytes</span>
              <button type="button" className={buttonClass} disabled={!editable} onClick={() => {
                selectionSource.current = sourceIdentity;
                setDraft(previous => ({...previous, key: object.key, version_id: object.version_id, filename: '', sha256: ''}));
              }}>Use {object.key} as primary</button>
              <button type="button" className={buttonClass} disabled={!editable || auxiliaries.length >= 31} onClick={() => {
                selectionSource.current = sourceIdentity;
                setAuxiliaries(rows => [...rows, {id: nextRow.current++, key: object.key, version_id: object.version_id, logical_path: '', sha256: ''}]);
              }}>Add {object.key} as package file</button>
            </li>)}</ul>
          </>}
        </section>
        <fieldset disabled={!editable} className="mt-4 space-y-3">
          <legend>Additional selected package files</legend>
          <p className="text-sm">Keep the original package paths. Select the weight index and every shard, model config, tokenizer assets and required processor. Standard SD/SDXL packages also need model_index.json and all required components. Each file needs its exact key, immutable VersionId, output path and SHA-256. Unsupported formats or incomplete packages are refused before model registration.</p>
          {auxiliaries.map((file, index) => <fieldset key={file.id} className="grid grid-cols-1 sm:grid-cols-2 gap-3 border p-3">
            <legend>Package file {index + 1}</legend>
            {([['key','exact object key',1024],['version_id','immutable VersionId',4096],['logical_path','logical output path',1024],['sha256','expected SHA-256',64]] as const).map(([field,label,max]) =>
              <label key={field} className="block text-sm">Package file {index + 1} {label}<input className={inputClass} required maxLength={max} value={file[field]}
                onChange={event => setAuxiliaries(rows => rows.map(row => row.id === file.id ? {...row,[field]:event.target.value} : row))} /></label>)}
            <button type="button" className={buttonClass} onClick={() => setAuxiliaries(rows => rows.filter(row => row.id !== file.id))}>Remove package file {index + 1}</button>
          </fieldset>)}
          <button type="button" className={buttonClass} disabled={auxiliaries.length >= 31}
            onClick={() => setAuxiliaries(rows => [...rows,{id:nextRow.current++,key:'',version_id:'',logical_path:'',sha256:''}])}>Add package file</button>
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
          {bundleProgress && bundleProgress.files_total > 0 && <>
            <p>{bundleProgress.total_bytes_observed} of {bundleProgress.total_expected_bytes ?? 'unknown'} total bytes observed in staging. Counts can reset on retry and do not prove publication.</p>
            <p>{bundleProgress.files_acquired} of {bundleProgress.files_total} selected files acquired in staging; complete verification and registration follow separately.</p>
            {bundleProgress.file_index !== null && <p>Acquiring file {bundleProgress.file_index + 1} in logical-path order.</p>}
          </>}
          <p className="text-xs">Operation: {snapshot.operation_id}</p>
        </div>}
        {snapshot?.status === 'not_found' && <p role="alert" className="mt-4">The operation result is unavailable. Check the library and reconcile retained work before importing this source again. Operation: {snapshot.operation_id}</p>}
        {snapshot?.status === 'finished' && <div role={snapshot.result.status === 'completed' ? 'status' : 'alert'} className="mt-4">
          {snapshot.result.status === 'completed' ? <><p>Model registered: {snapshot.result.model_id}</p><p>Registration does not establish backend compatibility or inference readiness.</p></>
            : snapshot.result.status === 'cancelled' ? <p>Import cancelled.</p>
              : <><p>{snapshot.result.error.message}</p>{snapshot.result.published_model_id && <p>Publication may exist for {snapshot.result.published_model_id}; verify the library before retrying.</p>}</>}
          {snapshot.result.status !== 'completed' && snapshot.result.retained_work && <p>Input or publication custody may remain. Only an eligible transfer held by this process can be retried explicitly; other retained work requires reconciliation.</p>}
        </div>}
        {error && <p role="alert" className="mt-4">{error}</p>}
        <p className="my-3 text-sm">Verified bytes must pass model qualification before registration. ONNX/external tensors, executable custom code and unsupported package classes are not admitted by this import path.</p>
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
