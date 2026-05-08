import { Play, Save, Square, Trash2 } from 'lucide-react';
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

const providerDeviceModes: Record<RuntimeProviderId, RuntimeDeviceMode[]> = {
  ollama: ['auto', 'cpu', 'gpu', 'hybrid'],
  llama_cpp: ['auto', 'cpu', 'gpu', 'specific_device'],
};

function deviceModeLabel(mode: RuntimeDeviceMode): string {
  switch (mode) {
    case 'auto':
      return 'Auto';
    case 'cpu':
      return 'CPU';
    case 'gpu':
      return 'GPU';
    case 'hybrid':
      return 'Hybrid';
    case 'specific_device':
      return 'Specific device';
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
  onLaunch: () => void;
  onSave: () => void;
  onStop: () => void;
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
  onLaunch,
  onSave,
  onStop,
  onUpdateDraft,
}: RuntimeProfileEditorProps) {
  const isManagedProfile = draft.management_mode === 'managed';
  const isExistingProfile = selectedProfileId !== null;
  const canStartFromProfile =
    isExistingProfile &&
    isManagedProfile &&
    draft.provider_mode !== 'llama_cpp_dedicated' &&
    selectedStatus?.state !== 'running' &&
    selectedStatus?.state !== 'starting';
  const canStopProfile =
    isExistingProfile &&
    isManagedProfile &&
    (selectedStatus?.state === 'running' || selectedStatus?.state === 'starting');
  const deviceModes = providerDeviceModes[draft.provider].includes(draft.device_mode)
    ? providerDeviceModes[draft.provider]
    : [...providerDeviceModes[draft.provider], draft.device_mode];
  const showsDeviceId =
    draft.device_mode === 'gpu' || draft.device_mode === 'specific_device';
  const showsGpuLayers =
    draft.provider === 'llama_cpp' &&
    (draft.device_mode === 'gpu' || draft.device_mode === 'hybrid' || draft.device_mode === 'specific_device');

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
            disabled={isExistingProfile}
            className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))] disabled:opacity-70"
          />
          {isExistingProfile && (
            <span className="block text-[hsl(var(--launcher-text-muted)/0.75)]">
              Saved profile IDs cannot be renamed. Change the name field or create a new profile.
            </span>
          )}
        </label>
        <div className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
          <span>Runtime</span>
          <div className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary)/0.55)] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]">
            {providerLabel(draft.provider)}
          </div>
        </div>
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
            {deviceModes.map((mode) => (
              <option key={mode} value={mode}>
                {deviceModeLabel(mode)}
              </option>
            ))}
          </select>
        </label>
        <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))] md:col-span-2">
          <span>{isManagedProfile ? 'Managed endpoint override' : 'External endpoint URL'}</span>
          <input
            value={draft.endpoint_url}
            onChange={(event) => onUpdateDraft('endpoint_url', event.target.value)}
            placeholder={isManagedProfile ? 'Auto from process port' : 'http://127.0.0.1:11434'}
            className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
          />
          <span className="block text-[hsl(var(--launcher-text-muted)/0.75)]">
            {isManagedProfile
              ? 'Leave blank unless this managed process must bind a specific host URL.'
              : 'Required for external profiles that Pumas does not launch.'}
          </span>
        </label>
        <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
          <span>{isManagedProfile ? 'Managed process port' : 'Endpoint port'}</span>
          <input
            type="number"
            min={1}
            max={65535}
            value={draft.port}
            onChange={(event) => onUpdateDraft('port', event.target.value)}
            placeholder={isManagedProfile ? 'Auto' : undefined}
            className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
          />
          {isManagedProfile && (
            <span className="block text-[hsl(var(--launcher-text-muted)/0.75)]">
              Leave blank for a unique auto-assigned port. Do not reuse 11434 unless replacing the
              default Ollama profile.
            </span>
          )}
        </label>
        {showsDeviceId && (
          <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
            <span>Device ID</span>
            <input
              value={draft.device_id}
              onChange={(event) => onUpdateDraft('device_id', event.target.value)}
              className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
            />
          </label>
        )}
        {showsGpuLayers && (
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
          {draft.provider_mode === 'llama_cpp_dedicated' && isManagedProfile && (
            <span className="ml-2">Dedicated profiles start from a model&apos;s Serving page.</span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {selectedProfileId && isManagedProfile && draft.provider_mode !== 'llama_cpp_dedicated' && (
            selectedStatus?.state === 'running' || selectedStatus?.state === 'starting' ? (
              <button
                type="button"
                onClick={onStop}
                disabled={!canStopProfile || isSaving}
                className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md border border-[hsl(var(--launcher-border)/0.35)] text-xs text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-primary))] disabled:opacity-50"
              >
                <Square className="w-3.5 h-3.5" />
                Stop
              </button>
            ) : (
              <button
                type="button"
                onClick={onLaunch}
                disabled={!canStartFromProfile || isSaving}
                className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md border border-[hsl(var(--launcher-border)/0.35)] text-xs text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-primary))] disabled:opacity-50"
              >
                <Play className="w-3.5 h-3.5" />
                Start runtime
              </button>
            )
          )}
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
