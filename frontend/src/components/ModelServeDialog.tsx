import { useEffect, useMemo, useRef, useState } from 'react';
import { getElectronAPI } from '../api/adapter';
import { useRuntimeProfiles } from '../hooks/useRuntimeProfiles';
import type { ModelInfo } from '../types/apps';
import type { RuntimeDeviceMode } from '../types/api-runtime-profiles';
import type { ModelServeError, ModelServingConfig, ServedModelStatus } from '../types/api-serving';

interface ModelServeDialogProps {
  model: ModelInfo;
  initialProfileId?: string | null;
  onClose: () => void;
}

const DEVICE_OPTIONS: Array<{ value: RuntimeDeviceMode; label: string }> = [
  { value: 'auto', label: 'Auto' },
  { value: 'cpu', label: 'CPU' },
  { value: 'gpu', label: 'GPU' },
  { value: 'hybrid', label: 'Hybrid' },
  { value: 'specific_device', label: 'Device' },
];

function formatServeError(error: ModelServeError | null): string | null {
  if (!error) {
    return null;
  }
  return error.message || error.code.replace(/_/g, ' ');
}

function isGgufModel(model: ModelInfo): boolean {
  return model.primaryFormat === 'gguf' || model.format?.toLowerCase() === 'gguf';
}

export function ModelServeDialog({ model, initialProfileId, onClose }: ModelServeDialogProps) {
  const { profiles, routes, defaultProfileId, isLoading, error: profileError } = useRuntimeProfiles();
  const servingProfiles = useMemo(
    () => profiles.filter((profile) => profile.provider === 'ollama' || profile.provider === 'llama_cpp'),
    [profiles]
  );
  const [profileId, setProfileId] = useState('');
  const [deviceMode, setDeviceMode] = useState<RuntimeDeviceMode>('auto');
  const [deviceId, setDeviceId] = useState('');
  const [gpuLayers, setGpuLayers] = useState('');
  const [tensorSplit, setTensorSplit] = useState('');
  const [contextSize, setContextSize] = useState('');
  const [keepLoaded, setKeepLoaded] = useState(true);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [serveError, setServeError] = useState<ModelServeError | null>(null);
  const [servedStatus, setServedStatus] = useState<ServedModelStatus | null>(null);
  const dialogRef = useRef<HTMLDivElement | null>(null);
  const profileSelectRef = useRef<HTMLSelectElement | null>(null);

  useEffect(() => {
    profileSelectRef.current?.focus();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
        return;
      }
      if (event.key !== 'Tab' || !dialogRef.current) {
        return;
      }

      const focusableElements = Array.from(
        dialogRef.current.querySelectorAll<HTMLElement>(
          'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
        )
      );
      if (focusableElements.length === 0) {
        return;
      }

      const firstElement = focusableElements[0];
      const lastElement = focusableElements[focusableElements.length - 1];
      if (!firstElement || !lastElement) {
        return;
      }

      if (event.shiftKey && document.activeElement === firstElement) {
        event.preventDefault();
        lastElement.focus();
      } else if (!event.shiftKey && document.activeElement === lastElement) {
        event.preventDefault();
        firstElement.focus();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  useEffect(() => {
    if (profileId || servingProfiles.length === 0) {
      return;
    }
    const fallbackProfile = servingProfiles[0];
    const routedProfileId =
      initialProfileId || routes.find((route) => route.model_id === model.id)?.profile_id;
    const routedProfile = servingProfiles.find((profile) => profile.profile_id === routedProfileId);
    const defaultProfile = servingProfiles.find((profile) => profile.profile_id === defaultProfileId);
    if (fallbackProfile) {
      setProfileId((routedProfile ?? defaultProfile ?? fallbackProfile).profile_id);
    }
  }, [defaultProfileId, initialProfileId, model.id, profileId, routes, servingProfiles]);

  useEffect(() => {
    const api = getElectronAPI();
    if (!api?.get_serving_status) {
      return;
    }

    let isActive = true;
    void api.get_serving_status().then((response) => {
      if (!isActive || !response.success) {
        return;
      }
      const status = response.snapshot.served_models.find(
        (servedModel) => servedModel.model_id === model.id
      );
      setServedStatus(status ?? null);
      if (status) {
        setMessage(`Loaded on ${status.profile_id}`);
      }
    });

    return () => {
      isActive = false;
    };
  }, [model.id]);

  const selectedProfile = servingProfiles.find((profile) => profile.profile_id === profileId);
  const serveBlockReason =
    profileError ??
    (isLoading ? 'Loading runtime profiles.' : null) ??
    (servingProfiles.length === 0 ? 'Create a runtime profile before serving a model.' : null) ??
    (!selectedProfile ? 'Select a runtime target before serving.' : null) ??
    (!isGgufModel(model) ? 'Only GGUF models can be served locally in this flow.' : null);
  const canServe = Boolean(!serveBlockReason && !isSubmitting);

  useEffect(() => {
    if (!selectedProfile) {
      return;
    }
    setDeviceMode(selectedProfile.device.mode);
    setDeviceId(selectedProfile.device.device_id ?? '');
    setGpuLayers(selectedProfile.device.gpu_layers?.toString() ?? '');
    setTensorSplit(selectedProfile.device.tensor_split?.join(',') ?? '');
  }, [selectedProfile?.profile_id]);

  const buildConfig = (): ModelServingConfig | null => {
    if (!selectedProfile) {
      return null;
    }

    return {
      provider: selectedProfile.provider,
      profile_id: selectedProfile.profile_id,
      device_mode: deviceMode,
      device_id: deviceId.trim() ? deviceId.trim() : null,
      gpu_layers: gpuLayers.trim() ? Number(gpuLayers) : null,
      tensor_split: tensorSplit.trim()
        ? tensorSplit.split(',').map((value) => Number(value.trim()))
        : null,
      context_size: contextSize.trim() ? Number(contextSize) : null,
      keep_loaded: keepLoaded,
      model_alias: null,
    };
  };

  const handleServe = async () => {
    const api = getElectronAPI();
    const config = buildConfig();
    if (!api?.validate_model_serving_config || !api.serve_model || !config) {
      return;
    }

    setIsSubmitting(true);
    setMessage(null);
    setServeError(null);

    try {
      const request = { model_id: model.id, config };
      const validation = await api.validate_model_serving_config(request);
      if (!validation.valid) {
        setServeError(
          validation.errors[0] ?? {
            code: 'invalid_request',
            severity: 'non_critical',
            message: 'The selected runtime target cannot serve this model configuration.',
            model_id: model.id,
            profile_id: config.profile_id,
          }
        );
        return;
      }

      const response = await api.serve_model(request);
      if (response.loaded) {
        setServedStatus(response.status ?? null);
        setMessage('Loaded');
        return;
      }
      setServeError(
        response.load_error ?? {
          code: 'provider_load_failed',
          severity: 'non_critical',
          message: 'The runtime did not report the model as loaded.',
          model_id: model.id,
          profile_id: config.profile_id,
        }
      );
    } catch (caught) {
      setMessage(caught instanceof Error ? caught.message : 'Serving request failed');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleUnload = async () => {
    const api = getElectronAPI();
    if (!api?.unserve_model || !servedStatus) {
      return;
    }

    setIsSubmitting(true);
    setMessage(null);
    setServeError(null);

    try {
      const response = await api.unserve_model({
        model_id: servedStatus.model_id,
        profile_id: servedStatus.profile_id,
        model_alias: servedStatus.model_alias ?? null,
      });
      if (response.unloaded) {
        setServedStatus(null);
        setMessage('Unloaded');
      } else {
        setMessage(response.error ?? 'Model was not loaded');
      }
    } catch (caught) {
      setMessage(caught instanceof Error ? caught.message : 'Unload request failed');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-[hsl(0_0%_0%/0.78)] px-4 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-labelledby="model-serve-title"
    >
      <div
        ref={dialogRef}
        className="w-full max-w-xl rounded-lg border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-primary))] p-4 shadow-2xl"
      >
        <div className="mb-4 flex items-start justify-between gap-3">
          <div className="min-w-0">
            <h2 id="model-serve-title" className="text-sm font-semibold text-[hsl(var(--text-primary))]">
              Serve {model.name}
            </h2>
            <p className="mt-1 truncate text-xs text-[hsl(var(--text-tertiary))]">{model.id}</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded px-2 py-1 text-xs text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))]"
          >
            Close
          </button>
        </div>

        <div className="grid gap-3">
          <label className="grid gap-1 text-xs text-[hsl(var(--text-secondary))]">
            Runtime target
            <select
              ref={profileSelectRef}
              value={profileId}
              onChange={(event) => setProfileId(event.target.value)}
              className="rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-base))] px-2 py-1.5 text-sm text-[hsl(var(--text-primary))]"
            >
              {servingProfiles.map((profile) => (
                <option key={profile.profile_id} value={profile.profile_id}>
                  {profile.name}
                </option>
              ))}
            </select>
          </label>

          <div className="rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--launcher-bg-secondary))] px-3 py-2 text-xs text-[hsl(var(--text-secondary))]">
            {serveBlockReason
              ? `Cannot serve yet: ${serveBlockReason}`
              : `Ready to serve ${model.name} with ${selectedProfile?.name ?? 'the selected runtime target'}.`}
          </div>

          <div className="grid grid-cols-2 gap-3 text-xs">
            <div>
              <div className="text-[hsl(var(--text-tertiary))]">Provider</div>
              <div className="mt-1 text-[hsl(var(--text-primary))]">
                {selectedProfile?.provider ?? 'none'}
              </div>
            </div>
            <div>
              <div className="text-[hsl(var(--text-tertiary))]">Serving mode</div>
              <div className="mt-1 text-[hsl(var(--text-primary))]">
                {selectedProfile?.provider_mode ?? 'none'}
              </div>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <label className="grid gap-1 text-xs text-[hsl(var(--text-secondary))]">
              Model device
              <select
                value={deviceMode}
                onChange={(event) => setDeviceMode(event.target.value as RuntimeDeviceMode)}
                className="rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-base))] px-2 py-1.5 text-sm text-[hsl(var(--text-primary))]"
              >
                {DEVICE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="grid gap-1 text-xs text-[hsl(var(--text-secondary))]">
              Device ID
              <input
                value={deviceId}
                onChange={(event) => setDeviceId(event.target.value)}
                className="rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-base))] px-2 py-1.5 text-sm text-[hsl(var(--text-primary))]"
              />
            </label>
          </div>

          <div className="grid grid-cols-3 gap-3">
            <label className="grid gap-1 text-xs text-[hsl(var(--text-secondary))]">
              Model GPU layers
              <input
                type="number"
                value={gpuLayers}
                onChange={(event) => setGpuLayers(event.target.value)}
                className="rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-base))] px-2 py-1.5 text-sm text-[hsl(var(--text-primary))]"
              />
            </label>
            <label className="grid gap-1 text-xs text-[hsl(var(--text-secondary))]">
              Model tensor split
              <input
                value={tensorSplit}
                onChange={(event) => setTensorSplit(event.target.value)}
                className="rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-base))] px-2 py-1.5 text-sm text-[hsl(var(--text-primary))]"
              />
            </label>
            <label className="grid gap-1 text-xs text-[hsl(var(--text-secondary))]">
              Context
              <input
                type="number"
                value={contextSize}
                onChange={(event) => setContextSize(event.target.value)}
                className="rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-base))] px-2 py-1.5 text-sm text-[hsl(var(--text-primary))]"
              />
            </label>
          </div>

          <label className="flex items-center gap-2 text-xs text-[hsl(var(--text-secondary))]">
            <input
              type="checkbox"
              checked={keepLoaded}
              onChange={(event) => setKeepLoaded(event.target.checked)}
            />
            Keep loaded
          </label>
        </div>

        {(serveError || message) && (
          <div className="mt-3 rounded border border-[hsl(var(--border-default))] px-3 py-2 text-xs text-[hsl(var(--text-secondary))]">
            {formatServeError(serveError) ?? message}
          </div>
        )}

        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded px-3 py-1.5 text-sm text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))]"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => void handleServe()}
            disabled={!canServe || isLoading}
            className="rounded bg-[hsl(var(--accent-primary))] px-3 py-1.5 text-sm text-[hsl(0_0%_10%)] disabled:opacity-50"
          >
            {isSubmitting ? 'Serving...' : 'Serve'}
          </button>
          <button
            type="button"
            onClick={() => void handleUnload()}
            disabled={!servedStatus || isSubmitting}
            className="rounded border border-[hsl(var(--border-default))] px-3 py-1.5 text-sm text-[hsl(var(--text-primary))] disabled:opacity-50"
          >
            Unload
          </button>
        </div>
      </div>
    </div>
  );
}
