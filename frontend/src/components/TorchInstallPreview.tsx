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
  const [build, setBuild] = useState('');
  const [python, setPython] = useState('');
  const [adapter, setAdapter] = useState('');
  const [selectionMode, setSelectionMode] = useState<'preset' | 'upstream'>('upstream');
  const [preview, setPreview] = useState<TorchRuntimePreview | null>(null);
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
    setPreview(null);
    setError(null);
    setRejection(null);
    setChecks([]);
    setAlternatives(null);
    setAlternativesError(null);
    setAlternativesBusy(false);
    selectionModeRef.current = 'upstream';
    setSelectionMode(selectionModeRef.current);
    setBuild('');
    setPython('');
    setAdapter('');
    const releaseRequest = api.get_torch_release_options(tag).then((result) => {
      if (!active) return;
      setReleaseOptions(result);
      if (selectionModeRef.current === 'preset') return;
      const choice = result.combinations.find((item) => item.build === result.recommended?.build && item.python === result.recommended.python);
      setBuild(choice?.build ?? '');
      setPython(choice?.python ?? '');
    }).catch((cause: unknown) => {
      if (active) setError(`Release discovery inconclusive: ${errorText(cause)}`);
    }).finally(() => {
      if (active) setLoadingReleaseOptions(false);
    });
    const runtimeRequest = api.get_torch_runtime_options().then((result) => {
      if (!active) return;
      setRuntimeOptions(result);
      if (tag === result.preset.tag && result.bundledPresetAvailable) {
        selectionModeRef.current = 'preset';
        setSelectionMode('preset');
        setBuild(result.preset.build);
        setPython(result.preset.python);
        setAdapter(result.preset.adapter);
      } else {
        setAdapter(result.defaultAdapter);
      }
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
  }, [tag]);

  const invalidatePreview = () => {
    requestNumber.current += 1;
    setPreview(null);
    setProbing(false);
    setError(null);
    setRejection(null);
    setAlternatives(null);
    setAlternativesError(null);
    setAlternativesBusy(false);
  };

  const probe = async (selection = { build, python, adapter }) => {
    if (!selection.build || !selection.python || probing) return;
    const currentRequest = ++requestNumber.current;
    const key = `${selection.build}|${selection.python}|${selection.adapter}`;
    const label = `${selection.build} · ${selection.python} · ${selection.adapter}`;
    const recordCheck = (status: CheckedCombination['status']) => {
      setChecks((previous) => [...previous.filter((item) => item.key !== key), { key, label, status }]);
    };
    setPreview(null);
    setError(null);
    setRejection(null);
    setProbing(true);
    try {
      const outcome = await api.preview_torch_runtime({ tag, ...selection });
      if (requestNumber.current === currentRequest) {
        if (outcome.status === 'resolved') {
          setPreview(outcome.preview);
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
      const result = await api.find_torch_alternatives(tag, build, python);
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
      setPython(runtimeOptions?.preset.python ?? '');
      setAdapter(runtimeOptions?.preset.adapter ?? '');
    } else {
      const choice = releaseOptions?.combinations.find((item) => item.build === releaseOptions.recommended?.build && item.python === releaseOptions.recommended.python);
      setBuild(choice?.build ?? '');
      setPython(choice?.python ?? '');
      setAdapter(runtimeOptions?.defaultAdapter ?? '');
    }
  };
  const availableBuilds = [...new Set(releaseOptions?.combinations.map((choice) => choice.build) ?? [])];
  const availablePythons = [...new Set(releaseOptions?.combinations.filter((choice) => choice.build === build).map((choice) => choice.python) ?? [])];
  const availableAdapters = runtimeOptions?.adapters.filter((choice) => choice === 'none' || choice === 'flux2') ?? [];
  const isEligibleAlternative = (match: TorchAlternativeMatch) => match.tag === tag
    && releaseOptions?.tag === tag
    && releaseOptions.combinations.some((choice) => choice.build === match.build && choice.python === match.python);
  const eligibleAlternatives = alternatives?.matches.filter(isEligibleAlternative) ?? [];
  const eligibleCheckedBuilds = alternatives?.checkedBuilds.filter((choice) => availableBuilds.includes(choice)) ?? [];
  const hasSelectedPython = fixedPreset
    ? Boolean(runtimeOptions.pythons.some((choice) => choice.id === python))
    : Boolean(releaseOptions?.combinations.some((choice) => choice.build === build && choice.python === python));
  const loadingOptions = loadingRuntimeOptions || (!fixedPreset && loadingReleaseOptions);
  const canInstall = preview !== null && Boolean(preview.previewId)
    && preview.tag === tag && preview.build === build
    && preview.python === python && preview.adapter === adapter;

  return (
    <section className="flex-1 min-h-0 overflow-y-auto px-4 py-3 space-y-4" aria-label={`Torch installation preview for ${tag}`}>
      <button type="button" onClick={onBack} className="flex items-center gap-2 text-sm text-[hsl(var(--text-secondary))]">
        <ArrowLeft size={16} /> All versions
      </button>
      <div>
        <h3 className="text-lg font-semibold">Review Torch {tag}</h3>
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
          Managed Torch requires an already installed CPython 3.10–3.13 interpreter matching this host’s operating system and architecture; Pumas does not install Python.
        </p>
        <p className="text-xs text-[hsl(var(--text-secondary))]">
          This flow installs official binary wheels only. Source compilation is a separate unsupported path and is never used as a fallback.
        </p>
      </div>
      {loadingOptions ? <Loader2 aria-label="Loading Torch choices" className="animate-spin" /> : runtimeOptions && (releaseOptions || fixedPreset) && (
        <>
          {isPresetRelease && <div className="flex flex-wrap gap-2" role="group" aria-label="Torch release choice">
            <button type="button" aria-pressed={fixedPreset} onClick={() => chooseMode('preset')} className="rounded border px-3 py-2 text-sm">Qualified fixed preset</button>
            <button type="button" aria-pressed={!fixedPreset} onClick={() => chooseMode('upstream')} className="rounded border px-3 py-2 text-sm">Other official wheels (unverified)</button>
          </div>}
          {fixedPreset && <p className="text-sm">This release has one qualified Python 3.12 / CUDA 13.0 build.</p>}
          {fixedPreset ? <p className="text-sm">Recommended setup: {build} · {python} · bundled image dependencies (fixed).</p> : <>
            {build && python && <p className="text-sm">{releaseOptions?.recommended?.build === build && releaseOptions.recommended.python === python ? 'Recommended setup' : 'Selected setup'}: {build} · {python} · {adapter === 'flux2' ? 'Pumas image dependencies' : 'Core runtime only'}.</p>}
            {!build && releaseOptions?.combinations.length ? <p role="status" className="text-sm">No setup was recommended. Open Advanced setup and choose a build to continue.</p> : null}
            {releaseOptions?.recommendationNote && <p className="text-xs text-[hsl(var(--text-secondary))]">{releaseOptions.recommendationNote}</p>}
            {adapter === 'flux2' && <p className="text-xs text-[hsl(var(--text-secondary))]">Pumas image dependencies are selected by Pumas for its FLUX.2 path; they are not part of upstream Torch.</p>}
            <details className="rounded border p-3 text-sm">
              <summary className="cursor-pointer">Advanced setup</summary>
              <div className="mt-3 grid gap-3 sm:grid-cols-3">
                <label>Build
                  <select aria-label="Torch build" value={build} onChange={(event) => {
                    invalidatePreview();
                    const nextBuild = event.target.value;
                    setBuild(nextBuild);
                    setPython(releaseOptions?.combinations.find((choice) => choice.build === nextBuild)?.python ?? '');
                  }} className="mt-1 block w-full rounded border bg-[hsl(var(--surface-control))] p-2">
                    <option value="">Choose a build</option>
                    {availableBuilds.map((choice) => <option key={choice} value={choice}>{choice}</option>)}
                  </select>
                </label>
                <label>Installed Python
                  <select aria-label="Installed Python" value={python} disabled={!build} onChange={(event) => { invalidatePreview(); setPython(event.target.value); }} className="mt-1 block w-full rounded border bg-[hsl(var(--surface-control))] p-2">
                    <option value="">Choose installed Python</option>
                    {availablePythons.map((choice) => <option key={choice} value={choice}>{choice}</option>)}
                  </select>
                </label>
                <label>Dependency profile
                  <select aria-label="Dependency profile" value={adapter} onChange={(event) => { invalidatePreview(); setAdapter(event.target.value); }} className="mt-1 block w-full rounded border bg-[hsl(var(--surface-control))] p-2">
                    {availableAdapters.map((choice) => <option key={choice} value={choice}>{choice === 'flux2' ? 'Pumas image dependencies' : 'Core runtime only'}</option>)}
                  </select>
                </label>
              </div>
            </details>
          </>}
          {fixedPreset && runtimeOptions.pythons.length === 0 && <p role="status">No compatible installed CPython interpreter was found. Install host-matched CPython 3.10–3.13, then reopen this preview.</p>}
          {fixedPreset && runtimeOptions.pythons.length > 0 && !hasSelectedPython && <p role="status">This fixed preset requires installed CPython 3.12. Install it, then reopen this preview.</p>}
          {!fixedPreset && releaseOptions?.status === 'inconclusive' && <p role="status">Release discovery was inconclusive. Some official wheels may exist; retry later or review the available combinations.</p>}
          {!fixedPreset && releaseOptions?.status === 'none' && <p role="status">No official wheel matches were found in this scan for an installed compatible Python.</p>}
          {!fixedPreset && releaseOptions && !releaseOptions.completeScan && releaseOptions.status !== 'inconclusive' && <p role="status">This wheel scan was incomplete. Other combinations may still exist.</p>}
          {!fixedPreset && releaseOptions?.issues.map((issue, index) => <p key={`${index}-${issue}`} className="text-xs">{issue}</p>)}
          <button type="button" disabled={!build || !hasSelectedPython || !adapter || probing} onClick={() => void probe()} className="rounded border px-3 py-2 text-sm disabled:opacity-50">
            {probing ? 'Checking…' : 'Check selected combination'}
          </button>
        </>
      )}
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
                setPython(match.python);
                void probe({ build: match.build, python: match.python, adapter });
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
        <div className="space-y-3 rounded border p-3 text-sm" role="status">
          <p className="font-medium">{fixedPreset ? 'Fixed preset highlights' : 'Exact artifacts resolved'}</p>
          {preview.qualification === 'unverified' && <p>Artifact resolution is unverified; trial this installation before selecting it as default.</p>}
          {preview.artifacts.length > 0 && <ul className="space-y-2">{preview.artifacts.map((artifact) => (
            <li key={`${artifact.name}-${artifact.version}`} className="break-all rounded bg-[hsl(var(--surface-control))] p-2">
              <strong>{artifact.name} {artifact.version}</strong><br />
              <span>URL: {artifact.url}</span><br />
              <span>SHA-256: {artifact.sha256}</span>
            </li>
          ))}</ul>}
        </div>
      )}
      <button type="button" disabled={!canInstall} onClick={() => { if (preview?.previewId) onInstall(preview.previewId); }} className="rounded bg-[hsl(var(--accent-success))] px-4 py-2 text-sm font-medium disabled:opacity-50">
        {fixedPreset ? 'Install fixed preset' : 'Install reviewed artifacts'}
      </button>
    </section>
  );
}
