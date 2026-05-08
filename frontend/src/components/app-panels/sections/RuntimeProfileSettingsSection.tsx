import { useEffect, useMemo, useState } from 'react';
import { Plus, RefreshCw, Server } from 'lucide-react';
import { getElectronAPI } from '../../../api/adapter';
import { useRuntimeProfiles } from '../../../hooks/useRuntimeProfiles';
import type {
  RuntimeProfileConfig,
  RuntimeProviderId,
} from '../../../types/api-runtime-profiles';
import {
  providerModes,
  RuntimeProfileEditor,
  RuntimeProfileList,
  type RuntimeProfileDraft,
} from './RuntimeProfileSettingsEditor';

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
  const [isCreatingProfile, setIsCreatingProfile] = useState(false);
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
    if (!isCreatingProfile && !selectedProfileId && profiles.length > 0) {
      setSelectedProfileId(profiles[0]?.profile_id ?? null);
    }
  }, [isCreatingProfile, profiles, selectedProfileId]);

  useEffect(() => {
    if (!isCreatingProfile && selectedProfile) {
      setDraft(profileToDraft(selectedProfile));
    }
  }, [isCreatingProfile, selectedProfile]);

  const updateDraft = <Key extends keyof RuntimeProfileDraft>(
    key: Key,
    value: RuntimeProfileDraft[Key]
  ) => {
    setDraft((current) => {
      const next = { ...current, [key]: value };
      if (key === 'provider') {
        next.provider_mode = providerModes[value as RuntimeProviderId][0] ?? 'ollama_serve';
        if (next.management_mode === 'managed') {
          next.endpoint_url = '';
          next.port = '';
        }
      }
      if (key === 'management_mode' && value === 'managed') {
        next.endpoint_url = '';
        next.port = '';
      }
      return next;
    });
  };

  const handleNewProfile = () => {
    const nextDraft = newProfileDraft();
    setIsCreatingProfile(true);
    setSelectedProfileId(null);
    setDraft(nextDraft);
    setSaveError(null);
  };

  const handleSelectProfile = (profileId: string) => {
    setIsCreatingProfile(false);
    setSelectedProfileId(profileId);
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
      setIsCreatingProfile(false);
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
      setIsCreatingProfile(false);
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
        <RuntimeProfileList
          profiles={profiles}
          statuses={statuses}
          selectedProfileId={selectedProfileId}
          isLoading={isLoading}
          onSelectProfile={handleSelectProfile}
        />
        <RuntimeProfileEditor
          draft={draft}
          selectedProfileId={selectedProfileId}
          selectedStatus={selectedStatus}
          isSaving={isSaving}
          error={error}
          saveError={saveError}
          onDelete={() => void handleDelete()}
          onSave={() => void handleSave()}
          onUpdateDraft={updateDraft}
        />
      </div>
    </section>
  );
}
