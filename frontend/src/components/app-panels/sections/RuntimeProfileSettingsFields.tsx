import type {
  RuntimeDeviceMode,
  RuntimeManagementMode,
  RuntimeProviderMode,
} from '../../../types/api-runtime-profiles';
import {
  deviceModeLabel,
  modeLabel,
  providerDeviceModes,
  providerLabel,
  providerManagementModes,
  providerModes,
  type RuntimeProfileDraft,
  type RuntimeProfileDraftUpdater,
} from './RuntimeProfileSettingsShared';

type RuntimeProfileSettingsFieldsProps = {
  draft: RuntimeProfileDraft;
  isExistingProfile: boolean;
  isManagedProfile: boolean;
  onUpdateDraft: RuntimeProfileDraftUpdater;
};

function getDeviceModes(draft: RuntimeProfileDraft): RuntimeDeviceMode[] {
  const providerModesForDevice = providerDeviceModes[draft.provider];
  return providerModesForDevice.includes(draft.device_mode)
    ? providerModesForDevice
    : [...providerModesForDevice, draft.device_mode];
}

function getManagementModes(draft: RuntimeProfileDraft): RuntimeManagementMode[] {
  const providerModesForManagement = providerManagementModes[draft.provider];
  return providerModesForManagement.includes(draft.management_mode)
    ? providerModesForManagement
    : [...providerModesForManagement, draft.management_mode];
}

export function RuntimeProfileSettingsFields({
  draft,
  isExistingProfile,
  isManagedProfile,
  onUpdateDraft,
}: RuntimeProfileSettingsFieldsProps) {
  const deviceModes = getDeviceModes(draft);
  const managementModes = getManagementModes(draft);
  const showsDeviceId =
    draft.device_mode === 'gpu' || draft.device_mode === 'specific_device';
  const showsGpuLayers =
    draft.provider === 'llama_cpp' &&
    (draft.device_mode === 'gpu' ||
      draft.device_mode === 'hybrid' ||
      draft.device_mode === 'specific_device');

  return (
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
          {managementModes.map((mode) => (
            <option key={mode} value={mode}>
              {mode === 'managed' ? 'Managed' : 'External'}
            </option>
          ))}
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
      <EndpointFields draft={draft} isManagedProfile={isManagedProfile} onUpdateDraft={onUpdateDraft} />
      {showsDeviceId && <DeviceIdField draft={draft} onUpdateDraft={onUpdateDraft} />}
      {showsGpuLayers && <GpuLayersField draft={draft} onUpdateDraft={onUpdateDraft} />}
      <label className="flex items-center gap-2 text-xs text-[hsl(var(--launcher-text-muted))]">
        <input
          type="checkbox"
          checked={draft.enabled}
          onChange={(event) => onUpdateDraft('enabled', event.target.checked)}
        />
        <span>Enabled</span>
      </label>
    </div>
  );
}

type DraftFieldProps = {
  draft: RuntimeProfileDraft;
  onUpdateDraft: RuntimeProfileDraftUpdater;
};

function EndpointFields({
  draft,
  isManagedProfile,
  onUpdateDraft,
}: DraftFieldProps & { isManagedProfile: boolean }) {
  return (
    <>
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
    </>
  );
}

function DeviceIdField({ draft, onUpdateDraft }: DraftFieldProps) {
  return (
    <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
      <span>Device ID</span>
      <input
        value={draft.device_id}
        onChange={(event) => onUpdateDraft('device_id', event.target.value)}
        className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
      />
    </label>
  );
}

function GpuLayersField({ draft, onUpdateDraft }: DraftFieldProps) {
  return (
    <label className="space-y-1 text-xs text-[hsl(var(--launcher-text-muted))]">
      <span>GPU Layers</span>
      <input
        type="number"
        value={draft.gpu_layers}
        onChange={(event) => onUpdateDraft('gpu_layers', event.target.value)}
        className="w-full px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))]"
      />
      <span className="block text-[hsl(var(--launcher-text-muted)/0.75)]">
        Use -1 for all layers.
      </span>
    </label>
  );
}
