import { useEffect, useRef, useState } from 'react';
import { ArrowLeft, Loader2 } from 'lucide-react';
import { api } from '../api/adapter';
import type { TorchRuntimeOptions, TorchRuntimePreviewOutcome, TorchRuntimePreviewRejectReason } from '../types/torch-install';

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

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function TorchInstallPreview({ tag, onBack, onInstall }: TorchInstallPreviewProps) {
  const [runtimeOptions, setRuntimeOptions] = useState<TorchRuntimeOptions | null>(null);
  const [loadingOptions, setLoadingOptions] = useState(true);
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
    setError(null);

    void api.get_torch_runtime_options().then((options) => {
      if (active) setRuntimeOptions(options);
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

  const startInstallation = async () => {
    if (!runtimeOptions || starting) return;
    const currentRequest = ++requestNumber.current;
    const build = runtimeOptions.defaultBuild || 'auto';
    setStarting(true);
    setError(null);

    try {
      const outcome: TorchRuntimePreviewOutcome = await api.preview_torch_runtime({
        tag,
        build,
        python: 'auto',
        adapter: 'none',
      });
      if (requestNumber.current !== currentRequest) return;
      if (outcome.status === 'rejected') {
        setError(`${reasonLabel(outcome.reason)}: ${outcome.message}`);
        setStarting(false);
        return;
      }
      if (outcome.status !== 'ready') {
        setError('Torch selection could not be confirmed. Try again.');
        setStarting(false);
        return;
      }
      const selection = outcome.preview;
      if (!selection.previewId || selection.expiresInSeconds <= 0 || selection.tag !== tag
        || selection.build !== build || selection.adapter !== 'none') {
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
            Pumas will choose the build and a private Python version for this computer. Core Torch dependencies are included.
          </p>
          <p className="text-sm text-[hsl(var(--text-secondary))]">
            Package and Python resolution, wheel downloads, and installation happen within the install task after you confirm below. Installation does not select or start this version.
          </p>
          <p className="text-xs text-[hsl(var(--text-secondary))]">
            This flow uses official binary wheels only. Device use, image generation, and socket startup need later runtime checks.
          </p>
        </div>
        {loadingOptions && <p role="status" className="flex items-center gap-2 text-sm text-[hsl(var(--text-secondary))]"><Loader2 aria-hidden="true" size={16} className="animate-spin" />Loading Torch choices…</p>}
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
