import { useEffect, useState, useSyncExternalStore } from 'react';
import { api } from '../api/adapter';
import type { RuntimeProfileConfig } from '../types/api-runtime-profiles';
import { torchTrialLifecycleStore, type createTorchTrialLifecycleStore } from './TorchTrialLifecycleStore';

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

interface TorchStartupTrialProps {
  tag: string;
  store?: ReturnType<typeof createTorchTrialLifecycleStore>;
}

export function TorchStartupTrial({ tag, store = torchTrialLifecycleStore }: TorchStartupTrialProps) {
  const [profiles, setProfiles] = useState<RuntimeProfileConfig[]>([]);
  const [profileId, setProfileId] = useState('');
  const [profileError, setProfileError] = useState<string | null>(null);
  const { phase, actionTag, result, pendingStop, error, stopResult } = useSyncExternalStore(store.subscribe, store.getSnapshot);

  useEffect(() => {
    let active = true;
    setProfiles([]);
    setProfileId('');
    setProfileError(null);
    void api.get_runtime_profiles_snapshot().then((response) => {
      if (!active) return;
      if (!response.success) {
        setProfileError(response.error ?? 'Could not load runtime profiles');
        return;
      }
      setProfiles(response.snapshot.profiles.filter((profile) =>
        profile.provider === 'torch' && profile.provider_mode === 'torch_serve'
        && profile.management_mode === 'managed' && profile.enabled));
    }).catch((cause: unknown) => {
      if (active) setProfileError(messageOf(cause));
    });
    return () => { active = false; };
  }, [tag]);

  const generationMissing = result !== null && result.startedByTrial
    && (result.success || result.cleanup === 'manual_stop_required')
    && result.cleanup !== 'stopped_owned_generation' && result.generation === null
    && stopResult === null;
  const needsStop = pendingStop !== null || generationMissing;
  const displayedResult = result?.tag === tag ? result : null;
  const displayedStopResult = displayedResult ? stopResult : null;

  return (
    <section className="mt-3 rounded border p-3 text-xs" aria-label={`Torch startup trial for ${tag}`}>
      <strong>Startup trial · {tag}</strong>
      <p className="mt-1">Starts a selected managed Torch profile and checks its health endpoint and protocol. Image generation and GPU model inference are not tested.</p>
      <label className="mt-2 block">Managed Torch profile
        <select aria-label="Managed Torch profile" value={profileId} onChange={(event) => setProfileId(event.target.value)} className="mt-1 block w-full rounded border bg-[hsl(var(--surface-control))] p-2">
          <option value="">Choose a profile</option>
          {profiles.map((profile) => <option key={profile.profile_id} value={profile.profile_id}>{profile.name}</option>)}
        </select>
      </label>
      {profiles.length === 0 && !profileError && <p className="mt-2">Create and enable a managed Torch profile before running a startup trial.</p>}
      {profileError && <p role="alert" className="mt-2">Profiles unavailable: {profileError}</p>}
      <div className="mt-2 flex gap-2">
        <button type="button" onClick={() => void store.start(tag, profileId)} disabled={!profileId || phase !== 'idle' || needsStop} className="rounded border px-3 py-2 disabled:opacity-50">
          {phase === 'starting' ? actionTag === tag ? 'Starting trial…' : `Starting trial for ${actionTag}…` : 'Trial startup'}
        </button>
        {needsStop && <button type="button" onClick={() => void store.stop()} disabled={phase !== 'idle' || pendingStop === null} className="rounded border px-3 py-2 disabled:opacity-50">{phase === 'stopping' && actionTag !== tag ? `Stopping trial for ${actionTag}…` : phase === 'stopping' ? 'Stopping…' : 'Stop trial profile'}</button>}
      </div>
      {error && <p role="alert" className="mt-2">Trial action failed{actionTag && actionTag !== tag ? ` for ${actionTag}` : ''}: {error}</p>}
      {pendingStop && <p className="mt-2" role="status">The trial profile for {pendingStop.tag} is still running. Stop it when finished.</p>}
      {generationMissing && <p role="alert">The trial for {result.tag} has no process generation, so it cannot be stopped safely from here.</p>}
      {displayedStopResult === 'stopped' && <p className="mt-2" role="status">Trial profile stopped.</p>}
      {displayedStopResult === 'not-current' && <p className="mt-2" role="status">The trial process has ended or been replaced. No current process was stopped.</p>}
      {displayedResult && <div className="mt-2" role="status">
        <p>{displayedResult.success ? 'Startup trial passed' : 'Startup trial failed'} · Health: {displayedResult.healthStatus} · Protocol: {displayedResult.protocol ?? 'not checked'}</p>
        {displayedResult.error && <p>{displayedResult.error}</p>}
        {displayedResult.capabilities.length > 0 && <p>Reported capabilities: {displayedResult.capabilities.join(', ')}</p>}
      </div>}
    </section>
  );
}
