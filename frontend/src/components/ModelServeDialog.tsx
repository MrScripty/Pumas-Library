import { useEffect, useMemo, useRef, useState } from 'react';
import { getElectronAPI } from '../api/adapter';
import { useRuntimeProfiles } from '../hooks/useRuntimeProfiles';
import type { ModelInfo } from '../types/apps';
import type { RuntimeDeviceMode } from '../types/api-runtime-profiles';
import type { ModelServeError, ModelServingConfig, ServedModelStatus } from '../types/api-serving';

interface ModelServeDialogProps {
  model: ModelInfo;
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

export function ModelServeDialog({ model, onClose }: ModelServeDialogProps) {
  const { profiles, defaultProfileId, isLoading, error: profileError } = useRuntimeProfiles();
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
  const profileSelectRef = useRef<HTMLSelectElement | null>(null);

  useEffect(() => {
    profileSelectRef.current?.focus();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
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
    const defaultProfile = servingProfiles.find((profile) => profile.profile_id === defaultProfileId);
    if (fallbackProfile) {
      setProfileId((defaultProfile ?? fallbackProfile).profile_id);
    }
  }, [defaultProfileId, profileId, servingProfiles]);

  const selectedProfile = servingProfiles.find((profile) => profile.profile_id === profileId);
  const canServe = Boolean(selectedProfile && isGgufModel(model) && !isSubmitting);

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
        setServeError(validation.errors[0] ?? null);
        return;
      }

      const response = await api.serve_model(request);
      if (response.loaded) {
        setServedStatus(response.status ?? null);
        setMessage('Loaded');
        return;
      }
      setServeError(response.load_error ?? null);
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
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="model-serve-title"
    >
      <div className="w-full max-w-lg rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-panel))] p-4 shadow-xl">
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
            Profile
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

          <div className="grid grid-cols-2 gap-3">
            <label className="grid gap-1 text-xs text-[hsl(var(--text-secondary))]">
              Device
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
              GPU Layers
              <input
                type="number"
                value={gpuLayers}
                onChange={(event) => setGpuLayers(event.target.value)}
                className="rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-base))] px-2 py-1.5 text-sm text-[hsl(var(--text-primary))]"
              />
            </label>
            <label className="grid gap-1 text-xs text-[hsl(var(--text-secondary))]">
              Tensor Split
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

        {(profileError || !isGgufModel(model) || serveError || message) && (
          <div className="mt-3 rounded border border-[hsl(var(--border-default))] px-3 py-2 text-xs text-[hsl(var(--text-secondary))]">
            {profileError ?? (!isGgufModel(model) ? 'Only GGUF models can be served locally in this flow.' : null) ?? formatServeError(serveError) ?? message}
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
