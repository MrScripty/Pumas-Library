import type {
  RuntimeProfileConfig,
  RuntimeProfileStatus,
} from '../../../types/api-runtime-profiles';
import { modeLabel, providerLabel } from './RuntimeProfileSettingsShared';

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
