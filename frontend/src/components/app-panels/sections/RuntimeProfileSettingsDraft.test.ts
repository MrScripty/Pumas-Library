import { describe, expect, it } from 'vitest';
import type { RuntimeProfileConfig } from '../../../types/api-runtime-profiles';
import { draftToProfile, newProfileDraft, profileToDraft } from './RuntimeProfileSettingsDraft';

const pinnedProfile: RuntimeProfileConfig = {
  profile_id: 'llama-pinned',
  provider: 'llama_cpp',
  provider_mode: 'llama_cpp_router',
  management_mode: 'managed',
  name: 'Pinned router',
  enabled: true,
  endpoint_url: null,
  port: null,
  device: { mode: 'cpu' },
  scheduler: { auto_load: true },
  runtime_version: 'b9082+cpu',
};

describe('runtime profile draft runtime_version', () => {
  it('round-trips a pinned runtime version', () => {
    const draft = profileToDraft(pinnedProfile);

    expect(draft.runtime_version).toBe('b9082+cpu');
    expect(draftToProfile(draft).runtime_version).toBe('b9082+cpu');
  });

  it('follows the active version when the pin is cleared or never set', () => {
    const cleared = { ...profileToDraft(pinnedProfile), runtime_version: '' };
    const unpinned = profileToDraft({ ...pinnedProfile, runtime_version: undefined });

    expect(draftToProfile(cleared).runtime_version).toBeNull();
    expect(unpinned.runtime_version).toBe('');
    expect(draftToProfile(unpinned).runtime_version).toBeNull();
    expect(newProfileDraft('torch').runtime_version).toBe('');
  });

  it('never sends a pin for an external profile', () => {
    const external = {
      ...profileToDraft(pinnedProfile),
      management_mode: 'external' as const,
      endpoint_url: 'http://127.0.0.1:18080',
    };

    expect(draftToProfile(external).runtime_version).toBeNull();
  });
});
