import { useEffect, useRef, useState } from 'react';
import { ArrowLeft, Loader2 } from 'lucide-react';
import { api } from '../api/adapter';
import type { TorchAlternativeMatch, TorchAlternativesOutcome, TorchReleaseOptionsOutcome, TorchRuntimeOptions, TorchRuntimePreview, TorchRuntimePreviewOutcome, TorchRuntimePreviewRejectReason } from '../types/torch-install';

interface TorchInstallPreviewProps {
  tag: string;
  onBack: () => void;
  onInstall: (previewId: string) => void;
}

interface CheckedCombination {
  key: string;
  label: string;
  status: 'resolved' | TorchRuntimePreviewRejectReason;
}

function reasonLabel(reason: TorchRuntimePreviewRejectReason): string {
  switch (reason) {
    case 'unsupported': return 'Unsupported combination';
    case 'validation_failed': return 'Resolved wheel report failed validation';
    case 'network_inconclusive': return 'Network inconclusive';
    case 'inconclusive': return 'Probe inconclusive';
  }
}

function checkStatusLabel(status: CheckedCombination['status']): string {
  if (status === 'resolved') return 'resolved';
  if (status === 'validation_failed') return 'failed validation';
  return reasonLabel(status).toLowerCase();
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function TorchInstallPreview({ tag, onBack, onInstall }: TorchInstallPreviewProps) {
  const [runtimeOptions, setRuntimeOptions] = useState<TorchRuntimeOptions | null>(null);
  const [releaseOptions, setReleaseOptions] = useState<TorchReleaseOptionsOutcome | null>(null);
  const [releaseError, setReleaseError] = useState<string | null>(null);
  const [releaseAttempt, setReleaseAttempt] = useState(0);
  const [build, setBuild] = useState('');
  const [adapter, setAdapter] = useState('');
  const [selectionMode, setSelectionMode] = useState<'preset' | 'upstream'>('upstream');
  const [preview, setPreview] = useState<TorchRuntimePreview | null>(null);
  const [previewExpiresAt, setPreviewExpiresAt] = useState<number | null>(null);
  const [previewExpired, setPreviewExpired] = useState(false);
  const [loadingRuntimeOptions, setLoadingRuntimeOptions] = useState(true);
  const [loadingReleaseOptions, setLoadingReleaseOptions] = useState(true);
  const [probing, setProbing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [rejection, setRejection] = useState<Extract<TorchRuntimePreviewOutcome, { status: 'rejected' }> | null>(null);
  const [checks, setChecks] = useState<CheckedCombination[]>([]);
  const [alternatives, setAlternatives] = useState<TorchAlternativesOutcome | null>(null);
  const [alternativesBusy, setAlternativesBusy] = useState(false);
  const [alternativesError, setAlternativesError] = useState<string | null>(null);
  const requestNumber = useRef(0);
  const selectionModeRef = useRef<'preset' | 'upstream'>('upstream');

  useEffect(() => {
    let active = true;
    setLoadingRuntimeOptions(true);
    setLoadingReleaseOptions(true);
    setRuntimeOptions(null);
    setReleaseOptions(null);
    setReleaseError(null);
    setPreview(null);
    setPreviewExpiresAt(null);
    setPreviewExpired(false);
    setError(null);
    setRejection(null);
    setChecks([]);
    setAlternatives(null);
    setAlternativesError(null);
    setAlternativesBusy(false);
    selectionModeRef.current = 'upstream';
    setSelectionMode(selectionModeRef.current);
    setBuild('');
    setAdapter('');
    const releaseRequest = api.get_torch_release_options(tag).then((result) => {
      if (!active) return;
      setReleaseOptions(result);
      if (selectionModeRef.current === 'preset') return;
      const choice = result.combinations.find((item) => item.build === result.recommended?.build && item.python === result.recommended.python);
      setBuild(choice?.build ?? '');
    }).catch((cause: unknown) => {
      if (active) setReleaseError(`Release discovery inconclusive: ${errorText(cause)}`);
    }).finally(() => {
      if (active) setLoadingReleaseOptions(false);
    });
    const runtimeRequest = api.get_torch_runtime_options().then((result) => {
      if (!active) return;
      setRuntimeOptions(result);
      setAdapter(result.defaultAdapter);
    }).catch((cause: unknown) => {
      if (active) setError(`Torch runtime choices unavailable: ${errorText(cause)}`);
    }).finally(() => {
      if (active) setLoadingRuntimeOptions(false);
    });
    void releaseRequest;
    void runtimeRequest;
    return () => {
      active = false;
      requestNumber.current += 1;
    };
  }, [tag, releaseAttempt]);

  useEffect(() => {
    if (previewExpiresAt === null) return;
    let timeout: number;
    const expireWhenDue = () => {
      const remaining = previewExpiresAt - Date.now();
      if (remaining <= 0) {
        setPreviewExpired(true);
      } else {
        timeout = window.setTimeout(expireWhenDue, Math.min(remaining, 2_147_483_647));
      }
    };
    expireWhenDue();
    return () => window.clearTimeout(timeout);
  }, [previewExpiresAt]);

  const invalidatePreview = () => {
    requestNumber.current += 1;
    setPreview(null);
    setPreviewExpiresAt(null);
    setPreviewExpired(false);
    setProbing(false);
    setError(null);
    setRejection(null);
    setAlternatives(null);
    setAlternativesError(null);
    setAlternativesBusy(false);
  };

  const selectedPython = selectionMode === 'preset' ? runtimeOptions?.preset.python ?? '' : 'auto';
  const probe = async (selection = { build, python: selectedPython, adapter }) => {
    if (!selection.build || !selection.python || probing) return;
    const currentRequest = ++requestNumber.current;
    const key = `${selection.build}|${selection.python}|${selection.adapter}`;
    const label = `${selection.build} · ${selection.python === 'auto' ? 'Python selected automatically' : selection.python} · ${selection.adapter}`;
    const recordCheck = (status: CheckedCombination['status']) => {
      setChecks((previous) => [...previous.filter((item) => item.key !== key), { key, label, status }]);
    };
    setPreview(null);
    setPreviewExpiresAt(null);
    setPreviewExpired(false);
    setError(null);
    setRejection(null);
    setProbing(true);
    try {
      const outcome = await api.preview_torch_runtime({ tag, ...selection });
      if (requestNumber.current === currentRequest) {
        if (outcome.status === 'resolved') {
          setPreview(outcome.preview);
          setPreviewExpiresAt(Date.now() + outcome.preview.expiresInSeconds * 1000);
          setPreviewExpired(false);
          recordCheck('resolved');
        } else {
          setRejection(outcome);
          recordCheck(outcome.reason);
        }
      }
    } catch (cause) {
      if (requestNumber.current === currentRequest) {
        const message = errorText(cause);
        setError(message);
        recordCheck('inconclusive');
      }
    } finally {
      if (requestNumber.current === currentRequest) setProbing(false);
    }
  };

  const findAlternatives = async () => {
    if (rejection?.reason !== 'unsupported' || alternativesBusy) return;
    const currentRequest = requestNumber.current;
    setAlternativesBusy(true);
    setAlternatives(null);
    setAlternativesError(null);
    try {
      const discoveryPython = releaseOptions?.combinations.find((choice) => choice.build === build)?.python;
      if (!discoveryPython) return;
      const result = await api.find_torch_alternatives(tag, build, discoveryPython);
      if (requestNumber.current === currentRequest) setAlternatives(result);
    } catch (cause) {
      if (requestNumber.current === currentRequest) setAlternativesError(errorText(cause));
    } finally {
      if (requestNumber.current === currentRequest) setAlternativesBusy(false);
    }
  };

  const isPresetRelease = tag === runtimeOptions?.preset.tag && runtimeOptions.bundledPresetAvailable;
  const fixedPreset = isPresetRelease && selectionMode === 'preset';
  const chooseMode = (mode: 'preset' | 'upstream') => {
    invalidatePreview();
    selectionModeRef.current = mode;
    setSelectionMode(mode);
    if (mode === 'preset') {
      setBuild(runtimeOptions?.preset.build ?? '');
      setAdapter(runtimeOptions?.preset.adapter ?? '');
    } else {
      const choice = releaseOptions?.combinations.find((item) => item.build === releaseOptions.recommended?.build && item.python === releaseOptions.recommended.python);
      setBuild(choice?.build ?? '');
      setAdapter(runtimeOptions?.defaultAdapter ?? '');
    }
  };
  const availableBuilds = [...new Set(releaseOptions?.combinations.map((choice) => choice.build) ?? [])];
  const availableAdapters = runtimeOptions?.adapters.filter((choice) => choice === 'none' || choice === 'flux2') ?? [];
  const isEligibleAlternative = (match: TorchAlternativeMatch) => match.tag === tag
    && releaseOptions?.tag === tag
    && releaseOptions.combinations.some((choice) => choice.build === match.build && choice.python === match.python);
  const eligibleAlternatives = alternatives?.matches.filter(isEligibleAlternative) ?? [];
  const eligibleCheckedBuilds = alternatives?.checkedBuilds.filter((choice) => availableBuilds.includes(choice)) ?? [];
  const loadingOptions = loadingRuntimeOptions || (!isPresetRelease && loadingReleaseOptions);
  const canInstall = preview !== null && Boolean(preview.previewId)
    && previewExpiresAt !== null && !previewExpired && Date.now() < previewExpiresAt
    && preview.tag === tag && preview.build === build
    && Boolean(preview.python) && preview.python !== 'auto'
    && (!fixedPreset || preview.python === runtimeOptions.preset.python)
    && preview.adapter === adapter;
  const installHint = previewExpired
    ? 'This review expired. Check the selected combination again to install.'
    : loadingOptions
      ? 'Loading Torch choices before artifact review.'
      : probing
        ? 'Checking the selected combination before installation.'
        : !build || !adapter
          ? 'Choose a build and dependency profile, then check the combination.'
          : !canInstall
            ? 'Check the selected combination to review its artifacts and enable installation.'
            : 'Artifact check complete. Confirm to install this Torch version.';

  return (
    <section className="flex flex-1 min-h-0 max-h-[calc(80vh-5rem)] flex-col overflow-hidden text-[hsl(var(--text-primary))]" aria-label={`Torch installation preview for ${tag}`}>
      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-3">
      <button type="button" onClick={onBack} className="flex items-center gap-2 rounded text-sm text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] active:opacity-75 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[hsl(var(--accent-success))]">
        <ArrowLeft size={16} /> All versions
      </button>
      <div>
        <h3 className="text-lg font-semibold">Review Torch {tag}</h3>
        <p className="text-sm text-[hsl(var(--text-secondary))]">
          The first Install click opens this review. Checking may fetch package files into Pumas’ private cache. Installing into the managed Torch environment starts only after you confirm Install below.
        </p>
        <p className="text-sm text-[hsl(var(--text-secondary))]">
          {fixedPreset
            ? 'Review fixed preset wheel highlights. The complete bundled lock is checked during installation; saved runtime checks can be inspected afterward.'
            : 'Check the exact official artifacts before installing.'}
          {' '}Installation does not select or start this version.
        </p>
        <p className="text-xs text-[hsl(var(--text-secondary))]">
          This check resolves package files and hashes. Device use, image generation, and socket startup need later runtime checks.
        </p>
        <p className="text-xs text-[hsl(var(--text-secondary))]">
          This flow installs official binary wheels only. Source compilation is a separate unsupported path and is never used as a fallback.
        </p>
        <p className="text-xs text-[hsl(var(--text-secondary))]">
          Pumas installs a private Python version that matches the selected Torch release and its dependencies. Python does not need to be installed on this computer.
        </p>
      </div>
      {loadingOptions ? <p role="status" className="flex items-center gap-2 text-sm text-[hsl(var(--text-secondary))]"><Loader2 aria-hidden="true" size={16} className="animate-spin" />Loading Torch choices…</p> : runtimeOptions && (releaseOptions || isPresetRelease) && (
        <>
          {isPresetRelease && <div className="flex flex-wrap gap-2" role="group" aria-label="Torch release choice">
            <button type="button" aria-pressed={fixedPreset} onClick={() => chooseMode('preset')} className="rounded border px-3 py-2 text-sm">Qualified fixed preset</button>
            <button type="button" aria-pressed={!fixedPreset} onClick={() => chooseMode('upstream')} className="rounded border px-3 py-2 text-sm">Other official wheels (unverified)</button>
          </div>}
          {fixedPreset && <p className="text-sm">This release has one qualified Python 3.12 / CUDA 13.0 build.</p>}
          {fixedPreset ? <p className="text-sm">Recommended setup: {build} · {runtimeOptions.preset.python} · bundled image dependencies (fixed).</p> : <>
            {build && <p className="text-sm">{releaseOptions?.recommended?.build === build ? 'Recommended setup' : 'Selected setup'}: {build} · Python selected automatically · {adapter === 'flux2' ? 'Pumas image dependencies' : 'Core runtime only'}.</p>}
            {!build && releaseOptions?.combinations.length ? <p role="status" className="text-sm">No setup was recommended. Open Advanced setup and choose a build to continue.</p> : null}
            {releaseOptions?.recommendationNote && <p className="text-xs text-[hsl(var(--text-secondary))]">{releaseOptions.recommendationNote}</p>}
            {adapter === 'flux2' && <p className="text-xs text-[hsl(var(--text-secondary))]">Pumas image dependencies are selected by Pumas for its FLUX.2 path; they are not part of upstream Torch.</p>}
            <details className="rounded border p-3 text-sm">
              <summary className="cursor-pointer">Advanced setup</summary>
              <div className="mt-3 grid gap-3 sm:grid-cols-2">
                <label>Build
                  <select aria-label="Torch build" value={build} onChange={(event) => {
                    invalidatePreview();
                    const nextBuild = event.target.value;
                    setBuild(nextBuild);
                  }} className="mt-1 block w-full rounded border bg-[hsl(var(--surface-control))] p-2 text-[hsl(var(--text-primary))]">
                    <option value="">Choose a build</option>
                    {availableBuilds.map((choice) => <option key={choice} value={choice}>{choice}</option>)}
                  </select>
                </label>
                <label>Dependency profile
                  <select aria-label="Dependency profile" value={adapter} onChange={(event) => { invalidatePreview(); setAdapter(event.target.value); }} className="mt-1 block w-full rounded border bg-[hsl(var(--surface-control))] p-2 text-[hsl(var(--text-primary))]">
                    {availableAdapters.map((choice) => <option key={choice} value={choice}>{choice === 'flux2' ? 'Pumas image dependencies' : 'Core runtime only'}</option>)}
                  </select>
                </label>
              </div>
            </details>
          </>}
          {!fixedPreset && loadingReleaseOptions && <p role="status">Discovering official wheels…</p>}
          {!fixedPreset && releaseOptions?.status === 'inconclusive' && <p role="status">Release discovery was inconclusive. Some official wheels may exist; retry later or review the available combinations.</p>}
          {!fixedPreset && releaseOptions?.status === 'none' && <p role="status">No official wheel matches were found in this scan.</p>}
          {!fixedPreset && releaseOptions && !releaseOptions.completeScan && releaseOptions.status !== 'inconclusive' && <p role="status">This wheel scan was incomplete. Other combinations may still exist.</p>}
          {!fixedPreset && releaseOptions?.issues.map((issue, index) => <p key={`${index}-${issue}`} className="text-xs">{issue}</p>)}
          {probing && <p role="status" className="flex items-center gap-2 rounded border border-[hsl(var(--accent-success))]/40 bg-[hsl(var(--accent-success)/0.08)] px-3 py-2 text-sm text-[hsl(var(--text-primary))]">
            <Loader2 aria-hidden="true" size={16} className="shrink-0 animate-spin text-[hsl(var(--accent-success))]" />
            <span>Resolving official Torch packages… Large wheels may download to Pumas’ shared cache so installation can reuse completed downloads.</span>
          </p>}
          <button type="button" disabled={!build || !adapter || probing} onClick={() => void probe()} className="inline-flex items-center gap-2 rounded border border-[hsl(var(--border-control))] bg-[hsl(var(--surface-control))] px-3 py-2 text-sm text-[hsl(var(--text-primary))] transition-colors hover:border-[hsl(var(--accent-success))] hover:bg-[hsl(var(--accent-success)/0.12)] active:scale-[0.98] active:bg-[hsl(var(--accent-success)/0.2)] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[hsl(var(--accent-success))] disabled:cursor-not-allowed disabled:opacity-60">
            {probing ? <><Loader2 aria-hidden="true" size={16} className="animate-spin" />Checking…</> : 'Check selected combination'}
          </button>
        </>
      )}
      {!fixedPreset && releaseError && <div className="space-y-2 text-sm">
        <p role="alert" className="text-[hsl(var(--accent-error))]">{releaseError}</p>
        <button type="button" onClick={() => setReleaseAttempt((attempt) => attempt + 1)} className="rounded border px-3 py-2">Retry release discovery</button>
      </div>}
      {checks.length > 0 && <div className="text-xs">
        <strong>Checked combinations</strong>
        <ul>{checks.map((check) => <li key={check.key}>{check.label}: {checkStatusLabel(check.status)}</li>)}</ul>
      </div>}
      {error && <p role="alert" className="text-sm text-[hsl(var(--accent-error))]">Probe inconclusive: {error}</p>}
      {rejection && <p role="alert" className="text-sm text-[hsl(var(--accent-error))]">{reasonLabel(rejection.reason)}: {rejection.message}</p>}
      {!fixedPreset && rejection?.reason === 'unsupported' && <div className="space-y-2 text-sm">
        <button type="button" disabled={alternativesBusy} onClick={() => void findAlternatives()} className="rounded border px-3 py-2 disabled:opacity-50">
          {alternativesBusy ? 'Checking official wheels…' : 'Find compatible alternatives'}
        </button>
        {alternativesError && <p role="alert">Alternative search inconclusive: {alternativesError}</p>}
        {alternatives && <div className="space-y-2 rounded border p-3">
          <strong>Official Torch wheel matches only</strong>
          <p>Other dependencies have not been checked. Choose a match to run the normal exact preview for the full environment.</p>
          <p>Checked builds: {eligibleCheckedBuilds.join(', ') || 'none'}</p>
          {alternatives.status === 'none' && <p>No official Torch wheel matches were found for this release and Python.</p>}
          {alternatives.status === 'inconclusive' && <p>Alternative search was inconclusive.</p>}
          {alternatives.issues.map((issue, index) => <p key={`${index}-${issue}`}>{issue}</p>)}
          {eligibleAlternatives.length > 0 && <ul className="space-y-2">{eligibleAlternatives.map((match) => (
            <li key={`${match.tag}|${match.build}|${match.python}`} className="rounded bg-[hsl(var(--surface-control))] p-2">
              <button type="button" className="underline" onClick={() => {
                if (!isEligibleAlternative(match)) return;
                invalidatePreview();
                setBuild(match.build);
                void probe({ build: match.build, python: 'auto', adapter });
              }}>
                Preview {match.tag} · {match.build} · {match.python}
              </button>
              {match.wheelUrl && <p className="break-all">Torch wheel: {match.wheelUrl}</p>}
              {match.sha256 && <p className="break-all">SHA-256: {match.sha256}</p>}
            </li>
          ))}</ul>}
        </div>}
      </div>}
      {preview && (
        <div className="space-y-3 rounded border p-3 text-sm">
          <div role="status">
            <p className="font-medium">{fixedPreset ? 'Fixed preset highlights' : 'Exact artifacts resolved'}</p>
            <p>{fixedPreset
              ? `${preview.artifacts.length} fixed preset wheel highlight${preview.artifacts.length === 1 ? '' : 's'} · ${preview.build} · ${preview.python}`
              : `${preview.artifacts.length} exact artifacts resolved · ${preview.build} · ${preview.python} · ${preview.adapter === 'none' ? 'Core runtime only' : preview.adapter}`}</p>
          </div>
          <p>Selected Python: {preview.python}</p>
          {fixedPreset && <p>Additional bundled dependencies and their URLs are selected during installation.</p>}
          {preview.qualification === 'unverified' && <p>Artifact resolution is unverified; trial this installation before selecting it as default.</p>}
          {previewExpired && <p role="alert" className="text-[hsl(var(--accent-error))]">Preview expired. Check selected combination again before installing.</p>}
          {preview.artifacts.length > 0 && <div role="region" aria-label="Artifact URL and hash review" className="rounded border p-3">
            <p className="font-medium">{fixedPreset ? 'Preset wheel highlight URLs and SHA-256 hashes' : 'Exact artifact URLs and SHA-256 hashes'}</p>
            <ul aria-label={fixedPreset ? 'Preset wheel highlight URLs and SHA-256 hashes' : 'Exact artifact URLs and SHA-256 hashes'} className="mt-3 max-h-72 space-y-2 overflow-y-auto">{preview.artifacts.map((artifact) => (
              <li key={`${artifact.name}-${artifact.version}`} className="break-all rounded bg-[hsl(var(--surface-control))] p-2">
                <strong>{artifact.name} {artifact.version}</strong><br />
                <span>URL: {artifact.url}</span><br />
                <span>SHA-256: {artifact.sha256}</span>
              </li>
            ))}</ul>
          </div>}
        </div>
      )}
      </div>
      <div className="shrink-0 border-t border-[hsl(var(--border-default))] bg-[hsl(var(--surface-low))] px-4 py-3">
        <button type="button" aria-describedby="torch-install-hint" disabled={!canInstall} onClick={() => { if (canInstall && Date.now() < previewExpiresAt) onInstall(preview.previewId); }} className="rounded border border-[hsl(var(--accent-success))] bg-[hsl(var(--accent-success)/0.14)] px-4 py-2 text-sm font-semibold text-[hsl(var(--text-primary))] transition-colors hover:bg-[hsl(var(--accent-success)/0.22)] active:scale-[0.98] active:bg-[hsl(var(--accent-success)/0.3)] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[hsl(var(--accent-success))] disabled:cursor-not-allowed disabled:opacity-50">
          {previewExpired ? 'Preview expired — check again' : fixedPreset ? 'Install fixed preset' : 'Install reviewed artifacts'}
        </button>
        <p id="torch-install-hint" className="mt-2 text-xs text-[hsl(var(--text-secondary))]">{installHint}</p>
      </div>
    </section>
  );
}
