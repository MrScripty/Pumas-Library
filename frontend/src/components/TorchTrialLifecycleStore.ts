import { api } from '../api/adapter';
import type { TorchStartupTrialOutcome } from '../types/torch-install';

export interface TorchTrialStopHandle {
  tag: string;
  profileId: string;
  generation: string;
}

export interface TorchTrialLifecycleState {
  phase: 'idle' | 'starting' | 'stopping';
  actionTag: string | null;
  result: TorchStartupTrialOutcome | null;
  pendingStop: TorchTrialStopHandle | null;
  error: string | null;
  stopResult: 'stopped' | 'not-current' | null;
}

interface TorchTrialLifecycleApi {
  trial_torch_runtime: typeof api.trial_torch_runtime;
  stop_runtime_profile_if_generation: typeof api.stop_runtime_profile_if_generation;
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function requiresStop(result: TorchStartupTrialOutcome | null): boolean {
  return result !== null && result.startedByTrial
    && (result.success || result.cleanup === 'manual_stop_required')
    && result.cleanup !== 'stopped_owned_generation';
}

export function createTorchTrialLifecycleStore(client: TorchTrialLifecycleApi = api) {
  let state: TorchTrialLifecycleState = {
    phase: 'idle', actionTag: null, result: null, pendingStop: null, error: null, stopResult: null,
  };
  let operationId = 0;
  const listeners = new Set<() => void>();
  const getSnapshot = () => state;
  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => { listeners.delete(listener); };
  };
  const publish = (next: TorchTrialLifecycleState) => {
    state = next;
    for (const listener of listeners) listener();
  };

  const start = async (tag: string, profileId: string): Promise<boolean> => {
    if (!profileId || state.phase !== 'idle' || state.pendingStop
      || (state.stopResult === null && requiresStop(state.result))) return false;
    const currentOperation = ++operationId;
    publish({ phase: 'starting', actionTag: tag, result: null, pendingStop: null, error: null, stopResult: null });
    try {
      const outcome = await client.trial_torch_runtime(tag, profileId);
      if (operationId !== currentOperation) return false;
      const pendingStop = requiresStop(outcome) && outcome.generation !== null
        ? { tag: outcome.tag, profileId: outcome.profileId, generation: outcome.generation }
        : null;
      publish({ phase: 'idle', actionTag: tag, result: outcome, pendingStop, error: null, stopResult: null });
      return true;
    } catch (error) {
      if (operationId !== currentOperation) return false;
      publish({ phase: 'idle', actionTag: tag, result: null, pendingStop: null, error: messageOf(error), stopResult: null });
      return false;
    }
  };

  const stop = async (): Promise<boolean> => {
    if (state.phase !== 'idle' || !state.pendingStop) return false;
    const handle = state.pendingStop;
    const currentOperation = ++operationId;
    publish({ ...state, phase: 'stopping', actionTag: handle.tag, error: null });
    try {
      const response = await client.stop_runtime_profile_if_generation(handle.profileId, handle.generation);
      if (operationId !== currentOperation || state.pendingStop !== handle) return false;
      if (!response.success) {
        publish({ ...state, phase: 'idle', error: response.error ?? 'Could not stop trial profile' });
        return false;
      }
      publish({ ...state, phase: 'idle', pendingStop: null, stopResult: response.stopped ? 'stopped' : 'not-current' });
      return true;
    } catch (error) {
      if (operationId !== currentOperation || state.pendingStop !== handle) return false;
      publish({ ...state, phase: 'idle', error: messageOf(error) });
      return false;
    }
  };

  return { getSnapshot, subscribe, start, stop };
}

export const torchTrialLifecycleStore = createTorchTrialLifecycleStore();
