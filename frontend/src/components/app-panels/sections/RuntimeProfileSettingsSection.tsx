import { useEffect, useMemo, useState } from 'react';
import { Plus, RefreshCw, Save, Server, Trash2 } from 'lucide-react';
import { getElectronAPI } from '../../../api/adapter';
import { useRuntimeProfiles } from '../../../hooks/useRuntimeProfiles';
import type {
  RuntimeDeviceMode,
  RuntimeManagementMode,
  RuntimeProfileConfig,
  RuntimeProviderId,
  RuntimeProviderMode,
} from '../../../types/api-runtime-profiles';

type RuntimeProfileDraft = {
  profile_id: string;
  provider: RuntimeProviderId;
  provider_mode: RuntimeProviderMode;
  management_mode: RuntimeManagementMode;
  name: string;
  enabled: boolean;
  endpoint_url: string;
  port: string;
  device_mode: RuntimeDeviceMode;
  device_id: string;
  gpu_layers: string;
};

const providerModes: Record<RuntimeProviderId, RuntimeProviderMode[]> = {
  ollama: ['ollama_serve'],
  llama_cpp: ['llama_cpp_router', 'llama_cpp_dedicated'],
};

function profileToDraft(profile: RuntimeProfileConfig): RuntimeProfileDraft {
  return {
    profile_id: profile.profile_id,
    provider: profile.provider,
    provider_mode: profile.provider_mode,
    management_mode: profile.management_mode,
    name: profile.name,
    enabled: profile.enabled,
    endpoint_url: profile.endpoint_url ?? '',
    port: profile.port?.toString() ?? '',
    device_mode: profile.device.mode,
    device_id: profile.device.device_id ?? '',
    gpu_layers: profile.device.gpu_layers?.toString() ?? '',
  };
}

function newProfileDraft(): RuntimeProfileDraft {
  return {
    profile_id: `runtime-${Date.now()}`,
    provider: 'ollama',
    provider_mode: 'ollama_serve',
    management_mode: 'managed',
    name: 'New Runtime Profile',
    enabled: true,
    endpoint_url: '',
    port: '',
    device_mode: 'auto',
    device_id: '',
    gpu_layers: '',
  };
}

function draftToProfile(draft: RuntimeProfileDraft): RuntimeProfileConfig {
  return {
    profile_id: draft.profile_id.trim(),
    provider: draft.provider,
    provider_mode: draft.provider_mode,
    management_mode: draft.management_mode,
    name: draft.name.trim(),
    enabled: draft.enabled,
    endpoint_url: draft.endpoint_url.trim() || null,
    port: draft.port.trim() ? Number.parseInt(draft.port, 10) : null,
    device: {
      mode: draft.device_mode,
      device_id: draft.device_id.trim() || null,
      gpu_layers: draft.gpu_layers.trim() ? Number.parseInt(draft.gpu_layers, 10) : null,
      tensor_split: null,
    },
    scheduler: {
      auto_load: true,
      max_concurrent_models: null,
      keep_alive_seconds: null,
    },
  };
}

function providerLabel(provider: RuntimeProviderId): string {
  return provider === 'llama_cpp' ? 'llama.cpp' : 'Ollama';
}

function modeLabel(mode: RuntimeProviderMode): string {
  switch (mode) {
    case 'ollama_serve':
      return 'Serve';
    case 'llama_cpp_router':
      return 'Router';
    case 'llama_cpp_dedicated':
      return 'Dedicated';
  }
}

export function RuntimeProfileSettingsSection() {
  const {
    profiles,
    statuses,
    isLoading,
    error,
    refreshRuntimeProfiles,
  } = useRuntimeProfiles();
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [draft, setDraft] = useState<RuntimeProfileDraft>(() => newProfileDraft());
  const [isSaving, setIsSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const selectedProfile = useMemo(
    () => profiles.find((profile) => profile.profile_id === selectedProfileId) ?? null,
    [profiles, selectedProfileId]
  );
  const selectedStatus = useMemo(
    () => statuses.find((status) => status.profile_id === selectedProfileId) ?? null,
    [statuses, selectedProfileId]
  );

  useEffect(() => {
    if (!selectedProfileId && profiles.length > 0) {
      setSelectedProfileId(profiles[0]?.profile_id ?? null);
    }
  }, [profiles, selectedProfileId]);

  useEffect(() => {
    if (selectedProfile) {
      setDraft(profileToDraft(selectedProfile));
    }
  }, [selectedProfile]);

  const updateDraft = <Key extends keyof RuntimeProfileDraft>(
    key: Key,
    value: RuntimeProfileDraft[Key]
  ) => {
    setDraft((current) => {
      const next = { ...current, [key]: value };
      if (key === 'provider') {
        next.provider_mode = providerModes[value as RuntimeProviderId][0] ?? 'ollama_serve';
      }
      return next;
    });
  };

  const handleNewProfile = () => {
    const nextDraft = newProfileDraft();
    setSelectedProfileId(null);
    setDraft(nextDraft);
    setSaveError(null);
  };

  const handleSave = async () => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.upsert_runtime_profile) {
      return;
    }

    setIsSaving(true);
    setSaveError(null);
    try {
      const profile = draftToProfile(draft);
      const response = await electronAPI.upsert_runtime_profile(profile);
      if (!response.success) {
        setSaveError(response.error ?? 'Failed to save runtime profile');
        return;
      }
      setSelectedProfileId(profile.profile_id);
      await refreshRuntimeProfiles();
    } catch (caught) {
      setSaveError(caught instanceof Error ? caught.message : 'Failed to save runtime profile');
    } finally {
      setIsSaving(false);
    }
  };

  const handleDelete = async () => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.delete_runtime_profile || !selectedProfileId) {
      return;
    }

    setIsSaving(true);
    setSaveError(null);
    try {
      const response = await electronAPI.delete_runtime_profile(selectedProfileId);
      if (!response.success) {
        setSaveError(response.error ?? 'Failed to delete runtime profile');
        return;
      }
      setSelectedProfileId(null);
      setDraft(newProfileDraft());
      await refreshRuntimeProfiles();
    } catch (caught) {
      setSaveError(caught instanceof Error ? caught.message : 'Failed to delete runtime profile');
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <section className="w-full space-y-3">
      <div className="flex items-center justify-between gap-3">
        <div className="text-xs uppercase tracking-wider text-[hsl(var(--launcher-text-muted))] flex items-center gap-2">
          <Server className="w-3.5 h-3.5" />
          <span>Runtime Profiles</span>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => void refreshRuntimeProfiles()}
            className="p-1.5 rounded-md border border-[hsl(var(--launcher-border)/0.35)] text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-primary))]"
            aria-label="Refresh runtime profiles"
          >
            <RefreshCw className="w-3.5 h-3.5" />
          </button>
          <button
            type="button"
            onClick={handleNewProfile}
            className="p-1.5 rounded-md border border-[hsl(var(--launcher-border)/0.35)] text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-primary))]"
            aria-label="New runtime profile"
          >
            <Plus className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-[minmax(180px,240px)_1fr] gap-3">
        <div className="space-y-2">
          {profiles.map((profile) => {
            const status = statuses.find((item) => item.profile_id === profile.profile_id);
            const selected = profile.profile_id === selectedProfileId;
            return (
              <button
                type="button"
                key={profile.profile_id}
                onClick={() => setSelectedProfileId(profile.profile_id)}
                className={`w-full text-left px-3 py-2 rounded-md border text-xs transition-colors ${
                  selected
                    ? 'border-[hsl(var(--accent-primary)/0.6)] bg-[hsl(var(--accent-primary)/0.08)]'
                    : 'border-[hsl(var(--launcher-border)/0.25)] bg-[hsl(var(--launcher-bg-secondary)/0.25)] hover:border-[hsl(var(--launcher-border)/0.5)]'
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                    {profile.name}
                  </span>
                  <span className="text-[hsl(var(--launcher-text-muted))]">{status?.state ?? 'unknown'}</span>
                </div>
                <div className="mt-1 text-[hsl(var(--launcher-text-muted))] truncate">
                  {providerLabel(profile.provider)} / {modeLabel(profile.provider_mode)}
                </div>
              </button>
            );
          })}
          {profiles.length === 0 && (
            <div className="px-3 py-2 rounded-md border border-[hsl(var(--launcher-border)/0.25)] text-xs text-[hsl(var(--launcher-text-muted))]">
              {isLoading ? 'Loading runtime profiles' : 'No runtime profiles'}
            </div>
          )}
        </div>

        <div className="space-y-3 px-3 py-3 rounded-lg bg-[hsl(var(--launcher-bg-secondary)/0.3)] border border-[hsl(var(--launcher-border)/0.3)]">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
              <span>Name</span>
              <input
                value={draft.name}
                onChange={(event) => updateDraft('name', event.target.value)}
                className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
              />
            </label>
            <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
              <span>Profile ID</span>
              <input
                value={draft.profile_id}
                onChange={(event) => updateDraft('profile_id', event.target.value)}
                className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
              />
            </label>
            <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
              <span>Provider</span>
              <select
                value={draft.provider}
                onChange={(event) => updateDraft('provider', event.target.value as RuntimeProviderId)}
                className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
              >
                <option value="ollama">Ollama</option>
                <option value="llama_cpp">llama.cpp</option>
              </select>
            </label>
            <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
              <span>Mode</span>
              <select
                value={draft.provider_mode}
                onChange={(event) => updateDraft('provider_mode', event.target.value as RuntimeProviderMode)}
                className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
              >
                {providerModes[draft.provider].map((mode) => (
                  <option key={mode} value={mode}>
                    {modeLabel(mode)}
                  </option>
                ))}
              </select>
            </label>
            <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
              <span>Management</span>
              <select
                value={draft.management_mode}
                onChange={(event) => updateDraft('management_mode', event.target.value as RuntimeManagementMode)}
                className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
              >
                <option value="managed">Managed</option>
                <option value="external">External</option>
              </select>
            </label>
            <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
              <span>Device</span>
              <select
                value={draft.device_mode}
                onChange={(event) => updateDraft('device_mode', event.target.value as RuntimeDeviceMode)}
                className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
              >
                <option value="auto">Auto</option>
                <option value="cpu">CPU</option>
                <option value="gpu">GPU</option>
                <option value="hybrid">Hybrid</option>
                <option value="specific_device">Specific Device</option>
              </select>
            </label>
            <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))] md:col-span-2">
              <span>Endpoint URL</span>
              <input
                value={draft.endpoint_url}
                onChange={(event) => updateDraft('endpoint_url', event.target.value)}
                placeholder="http://127.0.0.1:11434"
                className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
              />
            </label>
            <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
              <span>Port</span>
              <input
                type="number"
                min={1}
                max={65535}
                value={draft.port}
                onChange={(event) => updateDraft('port', event.target.value)}
                className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
              />
            </label>
            <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
              <span>Device ID</span>
              <input
                value={draft.device_id}
                onChange={(event) => updateDraft('device_id', event.target.value)}
                className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
              />
            </label>
            {draft.provider === 'llama_cpp' && (
              <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
                <span>GPU Layers</span>
                <input
                  type="number"
                  value={draft.gpu_layers}
                  onChange={(event) => updateDraft('gpu_layers', event.target.value)}
                  className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
                />
              </label>
            )}
            <label className="flex items-center gap-2 text-xs text-[hsl(var(--launcher-text-muted))]">
              <input
                type="checkbox"
                checked={draft.enabled}
                onChange={(event) => updateDraft('enabled', event.target.checked)}
              />
              <span>Enabled</span>
            </label>
          </div>

          {(error || saveError) && (
            <div className="text-xs text-[hsl(var(--accent-error))]">{saveError ?? error}</div>
          )}

          <div className="flex items-center justify-between gap-3">
            <div className="text-xs text-[hsl(var(--launcher-text-muted))]">
              {selectedStatus ? `State: ${selectedStatus.state}` : 'State: unknown'}
            </div>
            <div className="flex items-center gap-2">
              {selectedProfileId && (
                <button
                  type="button"
                  onClick={() => void handleDelete()}
                  disabled={isSaving}
                  className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md border border-[hsl(var(--launcher-border)/0.35)] text-xs text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-primary))] disabled:opacity-50"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                  Delete
                </button>
              )}
              <button
                type="button"
                onClick={() => void handleSave()}
                disabled={isSaving}
                className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md bg-[hsl(var(--accent-primary))] text-xs text-white disabled:opacity-50"
              >
                <Save className="w-3.5 h-3.5" />
                {isSaving ? 'Saving' : 'Save'}
              </button>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
