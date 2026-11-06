import { useEffect, useRef, useState } from 'react';
import { ArrowLeft, Loader2 } from 'lucide-react';
import { api } from '../api/adapter';
import type { TorchRuntimeOptions, TorchRuntimePreviewOutcome, TorchRuntimePreviewRejectReason, TorchVerifiedRuntime } from '../types/torch-install';

interface TorchInstallPreviewProps {
  tag: string;
  onBack: () => void;
  onInstall: (previewId: string) => void;
}

function reasonLabel(reason: TorchRuntimePreviewRejectReason): string {
  switch (reason) {
    case 'unsupported': return 'Unsupported selection';
    case 'validation_failed': return 'Selection failed validation';
    case 'network_inconclusive': return 'Network inconclusive';
    case 'inconclusive': return 'Selection inconclusive';
  }
}

const ADAPTER_LABELS: Record<string, string> = {
  none: 'Core Torch',
  flux2: 'FLUX.2 image generation',
  nunchaku: 'Nunchaku Z-Image generation',
};
const IMAGE_ADAPTER_CHOICES = ['none', 'flux2', 'nunchaku'];

function adapterLabel(adapter: string): string {
  return ADAPTER_LABELS[adapter] ?? adapter;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function TorchInstallPreview({ tag, onBack, onInstall }: TorchInstallPreviewProps) {
  const [runtimeOptions, setRuntimeOptions] = useState<TorchRuntimeOptions | null>(null);
  const [loadingOptions, setLoadingOptions] = useState(true);
  const [buildChoice, setBuildChoice] = useState('auto');
  const [adapterChoice, setAdapterChoice] = useState('none');
  const [pythonChoice, setPythonChoice] = useState('auto');
  const [useVerifiedVersions, setUseVerifiedVersions] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const requestNumber = useRef(0);

  useEffect(() => {
    let active = true;
    requestNumber.current += 1;
    setRuntimeOptions(null);
    setLoadingOptions(true);
    setStarting(false);
    setPythonChoice('auto');
    setBuildChoice('auto');
    setAdapterChoice('none');
    setUseVerifiedVersions(false);
    setError(null);

    void api.get_torch_runtime_options().then((options) => {
      if (active) {
        setRuntimeOptions(options);
        setBuildChoice(options.defaultBuild);
        setAdapterChoice(options.adapters.includes(options.defaultAdapter) ? options.defaultAdapter : 'none');
      }
    }).catch((cause: unknown) => {
      if (active) setError(`Torch runtime choices unavailable: ${errorText(cause)}`);
    }).finally(() => {
      if (active) setLoadingOptions(false);
    });

    return () => {
      active = false;
      requestNumber.current += 1;
    };
  }, [tag, attempt]);

  const requirementOf = (adapter: string) => runtimeOptions?.adapterRequirements.find((requirement) => requirement.adapter === adapter);
  // An adapter whose own wheel allows one selection is offered only for its release.
  const offered = (adapter: string) => runtimeOptions?.adapters.includes(adapter) === true
    && (requirementOf(adapter)?.tag ?? tag) === tag;
  const imageAdapters = IMAGE_ADAPTER_CHOICES.filter(offered);
  const chooseAdapter = (adapter: string) => {
    const requirement = requirementOf(adapter);
    if (requirement) {
      setBuildChoice(requirement.build);
      setPythonChoice(requirement.python);
    }
    setAdapterChoice(adapter);
    setError(null);
  };
  const adapterRequirement = requirementOf(adapterChoice);
  const verifiedChoices: { record: TorchVerifiedRuntime; adapter: string }[] = (runtimeOptions?.verified ?? [])
    .filter((record) => record.tag === tag && runtimeOptions?.builds.includes(record.build))
    .flatMap((record) => record.adapters.filter(offered).map((adapter) => ({ record, adapter })));
  const selectedVerified = verifiedChoices.find(({ record, adapter }) => record.build === buildChoice && record.python === pythonChoice && adapter === adapterChoice);

  const startInstallation = async () => {
    if (!runtimeOptions || starting || !offered(adapterChoice)) return;
    const currentRequest = ++requestNumber.current;
    const build = buildChoice;
    const python = pythonChoice;
    const adapter = adapterChoice;
    // Pinning applies only to an exact verified selection; Rust refuses others.
    const pinVerified = useVerifiedVersions && selectedVerified !== undefined;
    setStarting(true);
    setError(null);

    try {
      const outcome: TorchRuntimePreviewOutcome = await api.preview_torch_runtime({
        tag,
        build,
        python,
        adapter,
        ...(pinVerified ? { verified_versions: true } : {}),
      });
      if (requestNumber.current !== currentRequest) return;
      if (outcome.status === 'rejected') {
        setError(`${reasonLabel(outcome.reason)}: ${outcome.message}`);
        setStarting(false);
        return;
      }
      const selection = outcome.preview;
      if (!selection.previewId || selection.expiresInSeconds <= 0 || selection.tag !== tag
        || selection.build !== build || selection.python !== python || selection.adapter !== adapter) {
        setError('Torch selection could not be confirmed. Try again.');
        setStarting(false);
        return;
      }
      onInstall(selection.previewId);
    } catch (cause) {
      if (requestNumber.current === currentRequest) {
        setError(`Could not start Torch installation: ${errorText(cause)}`);
        setStarting(false);
      }
    }
  };

  return (
    <section className="flex flex-1 min-h-0 max-h-[calc(80vh-5rem)] flex-col overflow-hidden text-[hsl(var(--text-primary))]" aria-label={`Torch installation preview for ${tag}`}>
      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-3">
        <button type="button" onClick={onBack} className="flex items-center gap-2 rounded text-sm text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] active:opacity-75 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[hsl(var(--accent-success))]">
          <ArrowLeft size={16} /> All versions
        </button>
        <div className="space-y-2">
          <h3 className="text-lg font-semibold">Install Torch {tag}</h3>
          <p className="text-sm text-[hsl(var(--text-secondary))]">
            Pumas can choose the build and a private Python version for this computer. Core Torch dependencies are included by default.
          </p>
          <p className="text-sm text-[hsl(var(--text-secondary))]">
            Package and Python resolution, wheel downloads, and installation happen within the install task after you confirm below. Installation does not select or start this version.
          </p>
          <p className="text-xs text-[hsl(var(--text-secondary))]">
            This flow uses official binary wheels only. Device use, image generation, and socket startup need later runtime checks.
          </p>
        </div>
        {runtimeOptions && imageAdapters.length > 1 && (
          <fieldset className="space-y-2" disabled={starting}>
            <legend className="text-sm font-medium">Image adapter</legend>
            {imageAdapters.map((adapter) => (
              <label key={adapter} className="flex items-center gap-2 text-sm">
                <input type="radio" name="torch-image-adapter" value={adapter} checked={adapterChoice === adapter} onChange={() => chooseAdapter(adapter)} />
                {adapterLabel(adapter)}
              </label>
            ))}
            {adapterRequirement && <p className="text-xs text-[hsl(var(--text-secondary))]">
              {adapterLabel(adapterRequirement.adapter)} installs only with the {adapterRequirement.build} build and Python {adapterRequirement.python.replace(/^python/, '')}, which its wheel was built for.
            </p>}
          </fieldset>
        )}
        {loadingOptions && <p role="status" className="flex items-center gap-2 text-sm text-[hsl(var(--text-secondary))]"><Loader2 aria-hidden="true" size={16} className="animate-spin" />Loading Torch choices…</p>}
        {runtimeOptions && <>
          {verifiedChoices.length > 0 && <div role="group" aria-label="Verified combinations" className="space-y-2 rounded border p-3 text-sm">
            <p className="font-medium">Verified combinations</p>
            {verifiedChoices.map(({ record, adapter }) => (
              <button key={`${record.build}-${record.python}-${adapter}`} type="button" disabled={starting} aria-pressed={selectedVerified?.record === record && adapterChoice === adapter} onClick={() => { setBuildChoice(record.build); setPythonChoice(record.python); setAdapterChoice(adapter); setError(null); }} className="mr-2 rounded border px-3 py-2">
                {record.build} · {record.python} · {adapterLabel(adapter)}
              </button>
            ))}
            {selectedVerified && <p className="text-xs text-[hsl(var(--text-secondary))]">
              Verified on {selectedVerified.record.verifiedOn} with {selectedVerified.record.packages.map((item) => `${item.name} ${item.version}`).join(', ')}. {selectedVerified.record.scope} {useVerifiedVersions
                ? 'Installing uses exactly these package versions, so the installed runtime matches the record.'
                : 'Installing resolves current package versions, which may be newer; the installed runtime is marked verified only if they match.'}
            </p>}
            {selectedVerified && <label className="flex items-center gap-2 text-xs">
              <input type="checkbox" disabled={starting} checked={useVerifiedVersions} onChange={(event) => { setUseVerifiedVersions(event.target.checked); setError(null); }} />
              Install the verified package versions
            </label>}
          </div>}
          <details className="rounded border p-3 text-sm">
            <summary className="cursor-pointer">Advanced setup</summary>
            <div className="mt-3 grid gap-3 sm:grid-cols-2">
              <label>Build
                <select aria-label="Torch build" disabled={starting} value={buildChoice} onChange={(event) => { setBuildChoice(event.target.value); setError(null); }} className="mt-1 block w-full rounded border bg-[hsl(var(--surface-control))] p-2">
                  <option value="auto">Select automatically</option>
                  {runtimeOptions.builds.filter((build) => build !== 'auto').map((build) => <option key={build} value={build}>{build}</option>)}
                </select>
              </label>
              {pythonChoice !== 'auto' && <p>Python {pythonChoice.replace(/^python/, '')}{' '}
                <button type="button" disabled={starting} onClick={() => { setPythonChoice('auto'); setError(null); }} className="rounded border px-2 py-1 text-xs">Select automatically</button>
              </p>}
            </div>
            <p className="mt-2 text-xs">These are platform-supported choices. Release and package compatibility are checked during installation. Pumas image dependencies are separate from upstream Torch.</p>
          </details>
        </>}
        {starting && <p role="status" className="flex items-center gap-2 text-sm text-[hsl(var(--text-primary))]"><Loader2 aria-hidden="true" size={16} className="animate-spin" />Starting installation…</p>}
        {error && <p role="alert" className="text-sm text-[hsl(var(--accent-error))]">{error}</p>}
        {error && !runtimeOptions && !loadingOptions && <button type="button" onClick={() => setAttempt((value) => value + 1)} className="rounded border px-3 py-2 text-sm">Retry Torch choices</button>}
      </div>
      <div className="shrink-0 border-t border-[hsl(var(--border-default))] bg-[hsl(var(--surface-low))] px-4 py-3">
        <button type="button" aria-describedby="torch-install-hint" disabled={!runtimeOptions || starting} onClick={() => void startInstallation()} className="rounded border border-[hsl(var(--accent-success))] bg-[hsl(var(--accent-success))] px-4 py-2 text-sm font-semibold text-[hsl(var(--surface-lowest))] transition-colors hover:bg-[hsl(var(--accent-success)/0.85)] active:bg-[hsl(var(--accent-success)/0.7)] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[hsl(var(--accent-success))] disabled:cursor-not-allowed disabled:opacity-50">
          {starting ? 'Starting installation…' : 'Install Torch'}
        </button>
        <p id="torch-install-hint" className="mt-2 text-xs text-[hsl(var(--text-secondary))]">
          {loadingOptions ? 'Loading local Torch choices.' : starting ? 'Starting the install task.' : 'Confirm to start package resolution and installation.'}
        </p>
      </div>
    </section>
  );
}
