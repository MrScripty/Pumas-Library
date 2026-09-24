import { useEffect, useRef, useState } from 'react';
import { ArrowLeft, Loader2 } from 'lucide-react';
import { api } from '../api/adapter';
import type { TorchAlternativesOutcome, TorchRuntimeOptions, TorchRuntimePreview, TorchRuntimePreviewOutcome, TorchRuntimePreviewRejectReason } from '../types/torch-install';

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
  const [options, setOptions] = useState<TorchRuntimeOptions | null>(null);
  const [build, setBuild] = useState('');
  const [python, setPython] = useState('');
  const [adapter, setAdapter] = useState('none');
  const [selectionMode, setSelectionMode] = useState<'preset' | 'upstream'>('upstream');
  const [preview, setPreview] = useState<TorchRuntimePreview | null>(null);
  const [loadingOptions, setLoadingOptions] = useState(true);
  const [probing, setProbing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [rejection, setRejection] = useState<Extract<TorchRuntimePreviewOutcome, { status: 'rejected' }> | null>(null);
  const [checks, setChecks] = useState<CheckedCombination[]>([]);
  const [alternatives, setAlternatives] = useState<TorchAlternativesOutcome | null>(null);
  const [alternativesBusy, setAlternativesBusy] = useState(false);
  const [alternativesError, setAlternativesError] = useState<string | null>(null);
  const requestNumber = useRef(0);

  useEffect(() => {
    let active = true;
    setLoadingOptions(true);
    setOptions(null);
    setPreview(null);
    setError(null);
    setRejection(null);
    setChecks([]);
    setAlternatives(null);
    setAlternativesError(null);
    setAlternativesBusy(false);
    void api.get_torch_runtime_options().then((result) => {
      if (!active) return;
      const preset = result.preset.tag === tag ? result.preset : null;
      setOptions(result);
      setSelectionMode(preset ? 'preset' : 'upstream');
      setBuild(preset?.build ?? result.builds[0] ?? '');
      setPython(preset?.python ?? result.pythons[0]?.id ?? '');
      setAdapter(preset?.adapter ?? result.adapters[0] ?? 'none');
    }).catch((cause: unknown) => {
      if (active) setError(errorText(cause));
    }).finally(() => {
      if (active) setLoadingOptions(false);
    });
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

  const isPresetRelease = options?.preset.tag === tag;
  const fixedPreset = isPresetRelease && selectionMode === 'preset';
  const chooseMode = (mode: 'preset' | 'upstream') => {
    if (!options) return;
    invalidatePreview();
    setSelectionMode(mode);
    if (mode === 'preset') {
      setBuild(options.preset.build);
      setPython(options.preset.python);
      setAdapter(options.preset.adapter);
    } else {
      setBuild(options.builds[0] ?? '');
      setPython(options.pythons[0]?.id ?? '');
      setAdapter(options.adapters[0] ?? 'none');
    }
  };
  const availableBuilds = fixedPreset ? [options.preset.build] : options?.builds ?? [];
  const availablePythons = fixedPreset
    ? [{ id: options.preset.python, label: `Python ${options.preset.python.replace('python', '')} (fixed preset)` }]
    : options?.pythons ?? [];
  const availableAdapters = fixedPreset ? [options.preset.adapter] : options?.adapters ?? [];
  const hasSelectedPython = options?.pythons.some((choice) => choice.id === python) ?? false;
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
          Managed Torch requires Linux x86_64. Pumas uses an already installed compatible CPython 3.10–3.13 interpreter; it does not install Python.
        </p>
        <p className="text-xs text-[hsl(var(--text-secondary))]">
          This flow installs official binary wheels only. Source compilation is a separate unsupported path and is never used as a fallback.
        </p>
      </div>
      {loadingOptions ? <Loader2 aria-label="Loading Torch choices" className="animate-spin" /> : options && (
        <>
          {isPresetRelease && <div className="flex flex-wrap gap-2" role="group" aria-label="Torch release choice">
            <button type="button" aria-pressed={fixedPreset} onClick={() => chooseMode('preset')} className="rounded border px-3 py-2 text-sm">Qualified fixed preset</button>
            <button type="button" aria-pressed={!fixedPreset} onClick={() => chooseMode('upstream')} className="rounded border px-3 py-2 text-sm">Other official wheels (unverified)</button>
          </div>}
          {fixedPreset && <p className="text-sm">This release has one qualified Python 3.12 / CUDA 13.0 build.</p>}
          <div className="grid gap-3 sm:grid-cols-3">
            <label className="text-sm">Build
              <select aria-label="Torch build" value={build} disabled={fixedPreset} onChange={(event) => { invalidatePreview(); setBuild(event.target.value); }} className="mt-1 block w-full rounded border bg-[hsl(var(--surface-control))] p-2">
                {availableBuilds.map((choice) => <option key={choice} value={choice}>{choice}</option>)}
              </select>
            </label>
            <label className="text-sm">Installed Python
              <select aria-label="Installed Python" value={python} disabled={fixedPreset} onChange={(event) => { invalidatePreview(); setPython(event.target.value); }} className="mt-1 block w-full rounded border bg-[hsl(var(--surface-control))] p-2">
                {availablePythons.map((choice) => <option key={choice.id} value={choice.id}>{choice.label}</option>)}
              </select>
            </label>
            <label className="text-sm">Image adapter
              <select aria-label="Image adapter" value={adapter} disabled={fixedPreset} onChange={(event) => { invalidatePreview(); setAdapter(event.target.value); }} className="mt-1 block w-full rounded border bg-[hsl(var(--surface-control))] p-2">
                {availableAdapters.map((choice) => <option key={choice} value={choice}>{choice === 'none' ? 'None' : choice === 'bundled' ? 'Bundled image dependencies (fixed)' : choice}</option>)}
              </select>
            </label>
          </div>
          {options.pythons.length === 0 && <p role="status">No supported installed Python interpreter was found. Install CPython 3.10–3.13 for Linux x86_64, then reopen this preview.</p>}
          {fixedPreset && options.pythons.length > 0 && !hasSelectedPython && <p role="status">This fixed preset requires installed CPython 3.12. Install it, then reopen this preview.</p>}
          <button type="button" disabled={!build || !hasSelectedPython || probing} onClick={() => void probe()} className="rounded border px-3 py-2 text-sm disabled:opacity-50">
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
          <p>Checked builds: {alternatives.checkedBuilds.join(', ') || 'none'}</p>
          {alternatives.status === 'none' && <p>No official Torch wheel matches were found for this release and Python.</p>}
          {alternatives.status === 'inconclusive' && <p>Alternative search was inconclusive.</p>}
          {alternatives.issues.map((issue, index) => <p key={`${index}-${issue}`}>{issue}</p>)}
          {alternatives.matches.length > 0 && <ul className="space-y-2">{alternatives.matches.map((match) => (
            <li key={`${match.tag}|${match.build}|${match.python}`} className="rounded bg-[hsl(var(--surface-control))] p-2">
              <button type="button" className="underline" onClick={() => {
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
