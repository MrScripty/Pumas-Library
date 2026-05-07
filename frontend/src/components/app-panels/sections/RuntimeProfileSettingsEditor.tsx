import { Save, Trash2 } from 'lucide-react';
import type {
  RuntimeDeviceMode,
  RuntimeManagementMode,
  RuntimeProfileConfig,
  RuntimeProfileStatus,
  RuntimeProviderId,
  RuntimeProviderMode,
} from '../../../types/api-runtime-profiles';

export type RuntimeProfileDraft = {
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

export const providerModes: Record<RuntimeProviderId, RuntimeProviderMode[]> = {
  ollama: ['ollama_serve'],
  llama_cpp: ['llama_cpp_router', 'llama_cpp_dedicated'],
};

export function providerLabel(provider: RuntimeProviderId): string {
  return provider === 'llama_cpp' ? 'llama.cpp' : 'Ollama';
}

export function modeLabel(mode: RuntimeProviderMode): string {
  switch (mode) {
    case 'ollama_serve':
      return 'Serve';
    case 'llama_cpp_router':
      return 'Router';
    case 'llama_cpp_dedicated':
      return 'Dedicated';
  }
}

type RuntimeProfileListProps = {
  profiles: RuntimeProfileConfig[];
  statuses: RuntimeProfileStatus[];
  selectedProfileId: string | null;
  isLoading: boolean;
  onSelectProfile: (profileId: string) => void;
};

export function RuntimeProfileList({
  profiles,
  statuses,
  selectedProfileId,
  isLoading,
  onSelectProfile,
}: RuntimeProfileListProps) {
  return (
    <div className="space-y-2">
      {profiles.map((profile) => {
        const status = statuses.find((item) => item.profile_id === profile.profile_id);
        const selected = profile.profile_id === selectedProfileId;
        return (
          <button
            type="button"
            key={profile.profile_id}
            onClick={() => onSelectProfile(profile.profile_id)}
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
              <span className="text-[hsl(var(--launcher-text-muted))]">
                {status?.state ?? 'unknown'}
              </span>
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
  );
}

type RuntimeProfileEditorProps = {
  draft: RuntimeProfileDraft;
  selectedProfileId: string | null;
  selectedStatus: RuntimeProfileStatus | null;
  isSaving: boolean;
  error: string | null;
  saveError: string | null;
  onDelete: () => void;
  onSave: () => void;
  onUpdateDraft: <Key extends keyof RuntimeProfileDraft>(
    key: Key,
    value: RuntimeProfileDraft[Key]
  ) => void;
};

export function RuntimeProfileEditor({
  draft,
  selectedProfileId,
  selectedStatus,
  isSaving,
  error,
  saveError,
  onDelete,
  onSave,
  onUpdateDraft,
}: RuntimeProfileEditorProps) {
  return (
    <div className="space-y-3 px-3 py-3 rounded-lg bg-[hsl(var(--launcher-bg-secondary)/0.3)] border border-[hsl(var(--launcher-border)/0.3)]">
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
          <span>Name</span>
          <input
            value={draft.name}
            onChange={(event) => onUpdateDraft('name', event.target.value)}
            className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
          />
        </label>
        <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
          <span>Profile ID</span>
          <input
            value={draft.profile_id}
            onChange={(event) => onUpdateDraft('profile_id', event.target.value)}
            className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
          />
        </label>
        <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
          <span>Provider</span>
          <select
            value={draft.provider}
            onChange={(event) => onUpdateDraft('provider', event.target.value as RuntimeProviderId)}
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
            onChange={(event) =>
              onUpdateDraft('provider_mode', event.target.value as RuntimeProviderMode)
            }
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
            onChange={(event) =>
              onUpdateDraft('management_mode', event.target.value as RuntimeManagementMode)
            }
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
            onChange={(event) =>
              onUpdateDraft('device_mode', event.target.value as RuntimeDeviceMode)
            }
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
            onChange={(event) => onUpdateDraft('endpoint_url', event.target.value)}
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
            onChange={(event) => onUpdateDraft('port', event.target.value)}
            className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
          />
        </label>
        <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
          <span>Device ID</span>
          <input
            value={draft.device_id}
            onChange={(event) => onUpdateDraft('device_id', event.target.value)}
            className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
          />
        </label>
        {draft.provider === 'llama_cpp' && (
          <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
            <span>GPU Layers</span>
            <input
              type="number"
              value={draft.gpu_layers}
              onChange={(event) => onUpdateDraft('gpu_layers', event.target.value)}
              className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
            />
          </label>
        )}
        <label className="flex items-center gap-2 text-xs text-[hsl(var(--launcher-text-muted))]">
          <input
            type="checkbox"
            checked={draft.enabled}
            onChange={(event) => onUpdateDraft('enabled', event.target.checked)}
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
              onClick={onDelete}
              disabled={isSaving}
              className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md border border-[hsl(var(--launcher-border)/0.35)] text-xs text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-primary))] disabled:opacity-50"
            >
              <Trash2 className="w-3.5 h-3.5" />
              Delete
            </button>
          )}
          <button
            type="button"
            onClick={onSave}
            disabled={isSaving}
            className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md bg-[hsl(var(--accent-primary))] text-xs text-white disabled:opacity-50"
          >
            <Save className="w-3.5 h-3.5" />
            {isSaving ? 'Saving' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  );
}
