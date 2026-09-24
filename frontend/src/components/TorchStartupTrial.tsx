import { useEffect, useState } from 'react';
import { api } from '../api/adapter';
import type { RuntimeProfileConfig } from '../types/api-runtime-profiles';
import type { TorchStartupTrialOutcome } from '../types/torch-install';

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function TorchStartupTrial({ tag }: { tag: string }) {
  const [profiles, setProfiles] = useState<RuntimeProfileConfig[]>([]);
  const [profileId, setProfileId] = useState('');
  const [profileError, setProfileError] = useState<string | null>(null);
  const [result, setResult] = useState<TorchStartupTrialOutcome | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [stopResult, setStopResult] = useState<'stopped' | 'not-current' | null>(null);

  useEffect(() => {
    let active = true;
    setProfiles([]);
    setProfileId('');
    setResult(null);
    setActionError(null);
    setStopResult(null);
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
    }).catch((error: unknown) => {
      if (active) setProfileError(messageOf(error));
    });
    return () => { active = false; };
  }, [tag]);

  const needsStop = result !== null && result.startedByTrial
    && (result.success || result.cleanup === 'manual_stop_required')
    && result.cleanup !== 'stopped_owned_generation' && stopResult === null;

  const trial = async () => {
    if (!profileId || busy || needsStop) return;
    setBusy(true);
    setResult(null);
    setActionError(null);
    setStopResult(null);
    try {
      setResult(await api.trial_torch_runtime(tag, profileId));
    } catch (error) {
      setActionError(messageOf(error));
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    if (!result || !needsStop || busy || result.generation === null) return;
    setBusy(true);
    setActionError(null);
    try {
      const response = await api.stop_runtime_profile_if_generation(result.profileId, result.generation);
      if (!response.success) {
        setActionError(response.error ?? 'Could not stop trial profile');
        return;
      }
      setStopResult(response.stopped ? 'stopped' : 'not-current');
    } catch (error) {
      setActionError(messageOf(error));
    } finally {
      setBusy(false);
    }
  };

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
        <button type="button" onClick={() => void trial()} disabled={!profileId || busy || Boolean(needsStop)} className="rounded border px-3 py-2 disabled:opacity-50">
          {busy && !needsStop ? 'Starting trial…' : 'Trial startup'}
        </button>
        {needsStop && <button type="button" onClick={() => void stop()} disabled={busy || result.generation === null} className="rounded border px-3 py-2 disabled:opacity-50">{busy ? 'Stopping…' : 'Stop trial profile'}</button>}
      </div>
      {actionError && <p role="alert" className="mt-2">Trial action failed: {actionError}</p>}
      {result && <div className="mt-2" role="status">
        <p>{result.success ? 'Startup trial passed' : 'Startup trial failed'} · Health: {result.healthStatus} · Protocol: {result.protocol ?? 'not checked'}</p>
        {result.error && <p>{result.error}</p>}
        {result.capabilities.length > 0 && <p>Reported capabilities: {result.capabilities.join(', ')}</p>}
        {needsStop && <p>The trial profile is still running. Stop it when finished.</p>}
        {needsStop && result.generation === null && <p role="alert">This trial has no process generation, so it cannot be stopped safely from here.</p>}
        {stopResult === 'stopped' && <p>Trial profile stopped.</p>}
        {stopResult === 'not-current' && <p>The trial process has ended or been replaced. No current process was stopped.</p>}
      </div>}
    </section>
  );
}
