import { useEffect, useMemo, useState } from 'react';
import { Plus, RefreshCw, Server } from 'lucide-react';
import { getElectronAPI } from '../../../api/adapter';
import { useRuntimeProfiles } from '../../../hooks/useRuntimeProfiles';
import type { RuntimeProviderId } from '../../../types/api-runtime-profiles';
import {
  draftToProfile,
  newProfileDraft,
  profileToDraft,
} from './RuntimeProfileSettingsDraft';
import {
  providerModes,
  RuntimeProfileEditor,
  RuntimeProfileList,
  type RuntimeProfileDraft,
} from './RuntimeProfileSettingsEditor';

type RuntimeProfileSettingsSectionProps = {
  provider: RuntimeProviderId;
};

export function RuntimeProfileSettingsSection({ provider }: RuntimeProfileSettingsSectionProps) {
  const {
    profiles,
    statuses,
    isLoading,
    error,
    refreshRuntimeProfiles,
  } = useRuntimeProfiles();
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [draft, setDraft] = useState<RuntimeProfileDraft>(() => newProfileDraft(provider));
  const [isCreatingProfile, setIsCreatingProfile] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isRuntimeActionRunning, setIsRuntimeActionRunning] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const providerProfiles = useMemo(
    () => profiles.filter((profile) => profile.provider === provider),
    [profiles, provider]
  );
  const providerStatuses = useMemo(
    () =>
      statuses.filter((status) =>
        providerProfiles.some((profile) => profile.profile_id === status.profile_id)
      ),
    [providerProfiles, statuses]
  );
  const selectedProfile = useMemo(
    () => providerProfiles.find((profile) => profile.profile_id === selectedProfileId) ?? null,
    [providerProfiles, selectedProfileId]
  );
  const selectedStatus = useMemo(
    () => providerStatuses.find((status) => status.profile_id === selectedProfileId) ?? null,
    [providerStatuses, selectedProfileId]
  );

  useEffect(() => {
    if (isCreatingProfile) {
      return;
    }
    if (!selectedProfileId && providerProfiles.length > 0) {
      setSelectedProfileId(providerProfiles[0]?.profile_id ?? null);
      return;
    }
    if (selectedProfileId && !providerProfiles.some((profile) => profile.profile_id === selectedProfileId)) {
      setSelectedProfileId(providerProfiles[0]?.profile_id ?? null);
      setDraft(newProfileDraft(provider));
    }
  }, [isCreatingProfile, provider, providerProfiles, selectedProfileId]);

  useEffect(() => {
    if (isCreatingProfile) {
      setDraft((current) =>
        current.provider === provider
          ? current
          : {
              ...newProfileDraft(provider),
              name: current.name,
              profile_id: current.profile_id,
            }
      );
    }
  }, [isCreatingProfile, provider]);

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
      next.provider = provider;
      if (!providerModes[provider].includes(next.provider_mode)) {
        next.provider_mode = providerModes[provider][0] ?? 'ollama_serve';
      }
      if (key === 'device_mode') {
        if (value !== 'gpu' && value !== 'specific_device') {
          next.device_id = '';
        }
        if (provider !== 'llama_cpp' || value === 'auto' || value === 'cpu') {
          next.gpu_layers = '';
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
    const nextDraft = newProfileDraft(provider);
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
      setDraft(newProfileDraft(provider));
      await refreshRuntimeProfiles();
    } catch (caught) {
      setSaveError(caught instanceof Error ? caught.message : 'Failed to delete runtime profile');
    } finally {
      setIsSaving(false);
    }
  };

  const handleLaunchRuntime = async () => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.launch_runtime_profile || !selectedProfileId) {
      return;
    }

    setIsRuntimeActionRunning(true);
    setSaveError(null);
    try {
      const response = await electronAPI.launch_runtime_profile(selectedProfileId);
      if (!response.success) {
        setSaveError(response.error ?? 'Failed to start runtime profile');
        return;
      }
      await refreshRuntimeProfiles();
    } catch (caught) {
      setSaveError(caught instanceof Error ? caught.message : 'Failed to start runtime profile');
    } finally {
      setIsRuntimeActionRunning(false);
    }
  };

  const handleStopRuntime = async () => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.stop_runtime_profile || !selectedProfileId) {
      return;
    }

    setIsRuntimeActionRunning(true);
    setSaveError(null);
    try {
      const response = await electronAPI.stop_runtime_profile(selectedProfileId);
      if (!response.success) {
        setSaveError(response.error ?? 'Failed to stop runtime profile');
        return;
      }
      await refreshRuntimeProfiles();
    } catch (caught) {
      setSaveError(caught instanceof Error ? caught.message : 'Failed to stop runtime profile');
    } finally {
      setIsRuntimeActionRunning(false);
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
          profiles={providerProfiles}
          statuses={providerStatuses}
          selectedProfileId={selectedProfileId}
          isLoading={isLoading}
          onSelectProfile={handleSelectProfile}
        />
        <RuntimeProfileEditor
          draft={draft}
          selectedProfileId={selectedProfileId}
          selectedStatus={selectedStatus}
          isSaving={isSaving || isRuntimeActionRunning}
          error={error}
          saveError={saveError}
          onDelete={() => void handleDelete()}
          onLaunch={() => void handleLaunchRuntime()}
          onSave={() => void handleSave()}
          onStop={() => void handleStopRuntime()}
          onUpdateDraft={updateDraft}
        />
      </div>
    </section>
  );
}
