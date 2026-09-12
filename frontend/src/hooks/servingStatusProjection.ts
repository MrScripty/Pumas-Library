import type { RouterProfileSyncStatus } from '../types/api-serving';
import type { ServingControlObservation } from './useServingStatus';

export function profileControlObservation(
  observation: ServingControlObservation,
  routerProfiles: readonly RouterProfileSyncStatus[],
  profileId: string
): ServingControlObservation {
  if (observation.kind === 'unavailable') return observation;
  const sync = routerProfiles.find((profile) => profile.profile_id === profileId);
  if (sync && sync.observation_state !== 'current') {
    return {
      kind: 'unavailable',
      message: sync.observation_state === 'connecting'
        ? 'Router status is connecting for this profile'
        : (sync.last_error ?? 'Router status is unavailable for this profile'),
    };
  }
  return observation;
}
