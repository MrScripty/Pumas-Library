import { useEffect, useMemo, useState } from 'react';
import { Play, RotateCcw, Save } from 'lucide-react';
import { getElectronAPI } from '../api/adapter';
import { useRuntimeProfiles } from '../hooks/useRuntimeProfiles';
import type { ModelInfo } from '../types/apps';
import { ModelServeDialog } from './ModelServeDialog';

interface ModelRuntimeRouteEditorProps {
  modelId: string;
  modelName: string;
  primaryFile: string | null;
}

function getPrimaryFormat(primaryFile: string | null): ModelInfo['primaryFormat'] {
  if (primaryFile?.toLowerCase().endsWith('.gguf')) {
    return 'gguf';
  }
  if (primaryFile?.toLowerCase().endsWith('.safetensors')) {
    return 'safetensors';
  }
  return undefined;
}

export function ModelRuntimeRouteEditor({
  modelId,
  modelName,
  primaryFile,
}: ModelRuntimeRouteEditorProps) {
  const {
    profiles,
    routes,
    statuses,
    isLoading,
    error,
    refreshRuntimeProfiles,
  } = useRuntimeProfiles();
  const currentRoute = useMemo(
    () => routes.find((route) => route.model_id === modelId) ?? null,
    [modelId, routes]
  );
  const [profileId, setProfileId] = useState<string>('');
  const [autoLoad, setAutoLoad] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [showServeDialog, setShowServeDialog] = useState(false);

  useEffect(() => {
    setProfileId(currentRoute?.profile_id ?? '');
    setAutoLoad(currentRoute?.auto_load ?? true);
  }, [currentRoute]);

  const selectedProfile = profiles.find((profile) => profile.profile_id === profileId) ?? null;
  const selectedStatus = statuses.find((status) => status.profile_id === profileId) ?? null;

  const handleSave = async () => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.set_model_runtime_route) {
      return;
    }

    setIsSaving(true);
    setSaveError(null);
    try {
      const response = await electronAPI.set_model_runtime_route({
        model_id: modelId,
        profile_id: profileId || null,
        auto_load: autoLoad,
      });
      if (!response.success) {
        setSaveError(response.error ?? 'Failed to save runtime route');
        return;
      }
      await refreshRuntimeProfiles();
    } catch (caught) {
      setSaveError(caught instanceof Error ? caught.message : 'Failed to save runtime route');
    } finally {
      setIsSaving(false);
    }
  };

  const handleClear = async () => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.clear_model_runtime_route) {
      return;
    }

    setIsSaving(true);
    setSaveError(null);
    try {
      const response = await electronAPI.clear_model_runtime_route(modelId);
      if (!response.success) {
        setSaveError(response.error ?? 'Failed to clear runtime route');
        return;
      }
      await refreshRuntimeProfiles();
    } catch (caught) {
      setSaveError(caught instanceof Error ? caught.message : 'Failed to clear runtime route');
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        <label className="space-y-1 text-xs text-[hsl(var(--text-muted))]">
          <span>Runtime Profile</span>
          <select
            value={profileId}
            onChange={(event) => setProfileId(event.target.value)}
            className="w-full px-2 py-1.5 rounded bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] text-[hsl(var(--text-primary))]"
          >
            <option value="">Default profile</option>
            {profiles.map((profile) => (
              <option key={profile.profile_id} value={profile.profile_id}>
                {profile.name}
              </option>
            ))}
          </select>
        </label>

        <label className="space-y-1 text-xs text-[hsl(var(--text-muted))]">
          <span>Auto Load</span>
          <div className="flex h-[34px] items-center gap-2">
            <input
              type="checkbox"
              checked={autoLoad}
              onChange={(event) => setAutoLoad(event.target.checked)}
            />
            <span className="text-[hsl(var(--text-secondary))]">
              Register and load when supported
            </span>
          </div>
        </label>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-3 text-xs">
        <div>
          <div className="text-[hsl(var(--text-muted))]">Provider</div>
          <div className="mt-1 text-[hsl(var(--text-primary))]">
            {selectedProfile?.provider ?? 'default'}
          </div>
        </div>
        <div>
          <div className="text-[hsl(var(--text-muted))]">Mode</div>
          <div className="mt-1 text-[hsl(var(--text-primary))]">
            {selectedProfile?.provider_mode ?? 'default'}
          </div>
        </div>
        <div>
          <div className="text-[hsl(var(--text-muted))]">State</div>
          <div className="mt-1 text-[hsl(var(--text-primary))]">
            {selectedStatus?.state ?? (isLoading ? 'loading' : 'unknown')}
          </div>
        </div>
      </div>

      {(error || saveError) && (
        <div className="text-xs text-[hsl(var(--accent-error))]">{saveError ?? error}</div>
      )}

      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={() => setShowServeDialog(true)}
          className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md border border-[hsl(var(--border-default))] text-xs text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))]"
        >
          <Play className="w-3.5 h-3.5" />
          Serve
        </button>
        <button
          type="button"
          onClick={() => void handleClear()}
          disabled={isSaving || !currentRoute}
          className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md border border-[hsl(var(--border-default))] text-xs text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] disabled:opacity-50"
        >
          <RotateCcw className="w-3.5 h-3.5" />
          Clear
        </button>
        <button
          type="button"
          onClick={() => void handleSave()}
          disabled={isSaving}
          className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md bg-[hsl(var(--launcher-accent-primary))] text-xs text-white disabled:opacity-50"
        >
          <Save className="w-3.5 h-3.5" />
          {isSaving ? 'Saving' : 'Save'}
        </button>
      </div>
      {showServeDialog && (
        <ModelServeDialog
          model={{
            id: modelId,
            name: modelName,
            category: 'local',
            primaryFormat: getPrimaryFormat(primaryFile),
          }}
          initialProfileId={profileId || null}
          onClose={() => setShowServeDialog(false)}
        />
      )}
    </div>
  );
}
