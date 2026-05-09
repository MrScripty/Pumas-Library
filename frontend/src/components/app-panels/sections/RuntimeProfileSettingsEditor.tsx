import type { RuntimeProfileStatus } from '../../../types/api-runtime-profiles';
import { RuntimeProfileSettingsActions } from './RuntimeProfileSettingsActions';
import { RuntimeProfileSettingsFields } from './RuntimeProfileSettingsFields';
import type {
  RuntimeProfileDraft,
  RuntimeProfileDraftUpdater,
} from './RuntimeProfileSettingsShared';

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
  onUpdateDraft: RuntimeProfileDraftUpdater;
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
  const isExistingProfile = selectedProfileId !== null;
  const isManagedProfile = draft.management_mode === 'managed';

  return (
    <div className="space-y-3 px-3 py-3 rounded-lg bg-[hsl(var(--launcher-bg-secondary)/0.3)] border border-[hsl(var(--launcher-border)/0.3)]">
      <RuntimeProfileSettingsFields
        draft={draft}
        isExistingProfile={isExistingProfile}
        isManagedProfile={isManagedProfile}
        onUpdateDraft={onUpdateDraft}
      />

      {(error || saveError) && (
        <div className="text-xs text-[hsl(var(--accent-error))]">{saveError ?? error}</div>
      )}

      <RuntimeProfileSettingsActions
        draft={draft}
        selectedProfileId={selectedProfileId}
        selectedStatus={selectedStatus}
        isManagedProfile={isManagedProfile}
        isSaving={isSaving}
        onDelete={onDelete}
        onLaunch={onLaunch}
        onSave={onSave}
        onStop={onStop}
      />
    </div>
  );
}

export type { RuntimeProfileDraft } from './RuntimeProfileSettingsShared';
export {
  modeLabel,
  providerLabel,
  providerModes,
} from './RuntimeProfileSettingsShared';
export { RuntimeProfileList } from './RuntimeProfileSettingsList';
