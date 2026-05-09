import { Play, Save, Square, Trash2 } from 'lucide-react';
import type { RuntimeProfileStatus } from '../../../types/api-runtime-profiles';
import type { RuntimeProfileDraft } from './RuntimeProfileSettingsShared';

type RuntimeProfileSettingsActionsProps = {
  draft: RuntimeProfileDraft;
  selectedProfileId: string | null;
  selectedStatus: RuntimeProfileStatus | null;
  isManagedProfile: boolean;
  isSaving: boolean;
  onDelete: () => void;
  onLaunch: () => void;
  onSave: () => void;
  onStop: () => void;
};

export function RuntimeProfileSettingsActions({
  draft,
  selectedProfileId,
  selectedStatus,
  isManagedProfile,
  isSaving,
  onDelete,
  onLaunch,
  onSave,
  onStop,
}: RuntimeProfileSettingsActionsProps) {
  const isExistingProfile = selectedProfileId !== null;
  const isRunning = selectedStatus?.state === 'running' || selectedStatus?.state === 'starting';
  const canStartFromProfile =
    isExistingProfile &&
    isManagedProfile &&
    draft.provider_mode !== 'llama_cpp_dedicated' &&
    !isRunning;
  const canStopProfile = isExistingProfile && isManagedProfile && isRunning;
  const showsRuntimeAction =
    selectedProfileId && isManagedProfile && draft.provider_mode !== 'llama_cpp_dedicated';

  return (
    <div className="flex items-center justify-between gap-3">
      <div className="text-xs text-[hsl(var(--launcher-text-muted))]">
        {selectedStatus ? `State: ${selectedStatus.state}` : 'State: unknown'}
        {draft.provider_mode === 'llama_cpp_dedicated' && isManagedProfile && (
          <span className="ml-2">Dedicated profiles start from a model&apos;s Serving page.</span>
        )}
      </div>
      <div className="flex items-center gap-2">
        {showsRuntimeAction && (
          isRunning ? (
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
  );
}
