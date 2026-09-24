import { describe, expect, it, vi } from 'vitest';
import type { TorchStartupTrialOutcome } from '../types/torch-install';
import { createTorchTrialLifecycleStore } from './TorchTrialLifecycleStore';

function passedTrial(tag: string, generation: string): TorchStartupTrialOutcome {
  return {
    success: true, tag, profileId: 'managed-torch', startupStatus: 'passed',
    healthStatus: 'passed', protocol: 3, capabilities: [], generation,
    startedByTrial: true, cleanup: 'not_needed',
  };
}

describe('Torch trial lifecycle store', () => {
  it('serializes stop and startup re-entry so an old stop cannot clear a newer generation', async () => {
    let finishStop!: (value: { success: boolean; stopped: boolean }) => void;
    const trial = vi.fn()
      .mockResolvedValueOnce(passedTrial('v2.10.0', 'generation-1'))
      .mockResolvedValueOnce(passedTrial('v2.11.0', 'generation-2'));
    const stop = vi.fn(() => new Promise<{ success: boolean; stopped: boolean }>((resolve) => { finishStop = resolve; }));
    const store = createTorchTrialLifecycleStore({ trial_torch_runtime: trial, stop_runtime_profile_if_generation: stop });

    expect(await store.start('v2.10.0', 'managed-torch')).toBe(true);
    expect(store.getSnapshot().pendingStop?.generation).toBe('generation-1');
    const firstStop = store.stop();
    expect(store.getSnapshot().phase).toBe('stopping');
    expect(await store.stop()).toBe(false);
    expect(await store.start('v2.11.0', 'managed-torch')).toBe(false);
    expect(trial).toHaveBeenCalledTimes(1);

    finishStop({ success: true, stopped: true });
    expect(await firstStop).toBe(true);
    expect(stop).toHaveBeenCalledWith('managed-torch', 'generation-1');
    expect(await store.start('v2.11.0', 'managed-torch')).toBe(true);
    expect(store.getSnapshot().pendingStop).toEqual({
      tag: 'v2.11.0', profileId: 'managed-torch', generation: 'generation-2',
    });
    expect(stop).toHaveBeenCalledTimes(1);
  });
});
