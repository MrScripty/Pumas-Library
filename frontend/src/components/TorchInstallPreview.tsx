import { useEffect, useRef, useState } from 'react';
import { ArrowLeft, Loader2 } from 'lucide-react';
import { api } from '../api/adapter';
import type { TorchRuntimeOptions, TorchRuntimePreview } from '../types/torch-install';

interface TorchInstallPreviewProps {
  tag: string;
  onBack: () => void;
  onInstall: (previewId: string) => void;
}

interface CheckedCombination {
  key: string;
  label: string;
  status: 'resolved' | 'unsupported' | 'inconclusive';
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function TorchInstallPreview({ tag, onBack, onInstall }: TorchInstallPreviewProps) {
  const [options, setOptions] = useState<TorchRuntimeOptions | null>(null);
  const [build, setBuild] = useState('');
  const [python, setPython] = useState('');
  const [adapter, setAdapter] = useState('none');
  const [preview, setPreview] = useState<TorchRuntimePreview | null>(null);
  const [loadingOptions, setLoadingOptions] = useState(true);
  const [probing, setProbing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [checks, setChecks] = useState<CheckedCombination[]>([]);
  const requestNumber = useRef(0);

  useEffect(() => {
    let active = true;
    setLoadingOptions(true);
    setOptions(null);
    setPreview(null);
    setError(null);
    setChecks([]);
    void api.get_torch_runtime_options().then((result) => {
      if (!active) return;
      const preset = result.preset.tag === tag ? result.preset : null;
      setOptions(result);
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
  };

  const probe = async () => {
    if (!build || !python || probing) return;
    const currentRequest = ++requestNumber.current;
    const key = `${build}|${python}|${adapter}`;
    const label = `${build} · ${python} · ${adapter}`;
    const recordCheck = (status: CheckedCombination['status']) => {
      setChecks((previous) => [...previous.filter((item) => item.key !== key), { key, label, status }]);
    };
    setPreview(null);
    setError(null);
    setProbing(true);
    try {
      const result = await api.preview_torch_runtime({ tag, build, python, adapter });
      if (requestNumber.current === currentRequest) {
        setPreview(result);
        recordCheck('resolved');
      }
    } catch (cause) {
      if (requestNumber.current === currentRequest) {
        const message = errorText(cause);
        setError(message);
        recordCheck(message.includes('Unsupported combination:') ? 'unsupported' : 'inconclusive');
      }
    } finally {
      if (requestNumber.current === currentRequest) setProbing(false);
    }
  };

  const fixedPreset = options?.preset.tag === tag;
  const availableBuilds = fixedPreset ? [options.preset.build] : options?.builds ?? [];
  const availablePythons = fixedPreset
    ? [{ id: options.preset.python, label: `Python ${options.preset.python.replace('python', '')} (fixed preset)` }]
    : options?.pythons ?? [];
  const availableAdapters = fixedPreset ? [options.preset.adapter] : options?.adapters ?? [];
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
      </div>
      {loadingOptions ? <Loader2 aria-label="Loading Torch choices" className="animate-spin" /> : options && (
        <>
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
          {options.pythons.length === 0 && <p role="status">No supported installed Python interpreter was found.</p>}
          <button type="button" disabled={!build || !python || probing} onClick={() => void probe()} className="rounded border px-3 py-2 text-sm disabled:opacity-50">
            {probing ? 'Checking…' : 'Check selected combination'}
          </button>
        </>
      )}
      {checks.length > 0 && <div className="text-xs">
        <strong>Checked combinations</strong>
        <ul>{checks.map((check) => <li key={check.key}>{check.label}: {check.status}</li>)}</ul>
      </div>}
      {error && <p role="alert" className="text-sm text-[hsl(var(--accent-error))]">{error.includes('Unsupported combination:') ? 'Unsupported combination' : 'Probe inconclusive'}: {error}</p>}
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
