import { describe, expect, it } from 'vitest';
import { torchExtrasLabel, torchInstalledSummary } from './torch-install';
import type { TorchInstalledConfig } from './torch-install';

const installed: TorchInstalledConfig = {
  tag: 'v2.15.0', build: 'cu130', python: 'python3.12', adapter: 'none', extras: [],
  qualification: 'unverified', capabilities: [],
};

describe('torchInstalledSummary', () => {
  it('names what the runtime serves', () => {
    expect(torchInstalledSummary({
      ...installed,
      capabilities: [
        { name: 'text', status: 'available', reason: null },
        { name: 'cohere_asr', status: 'available', reason: null },
        { name: 'image', status: 'unavailable', reason: 'No compatible binary wheel for torchvision' },
      ],
    })).toBe('cu130 · python3.12 · general · unverified · serves text, cohere asr');
  });

  it('distinguishes failed checks from checks that never ran', () => {
    expect(torchInstalledSummary({
      ...installed,
      capabilities: [
        { name: 'text', status: 'unavailable', reason: 'No module named transformers' },
        { name: 'audio', status: 'unknown', reason: 'Probed before modality probing' },
      ],
    })).toBe('cu130 · python3.12 · general · unverified · no capability imports (text unavailable)');
    expect(torchInstalledSummary({
      ...installed,
      capabilities: [{ name: 'text', status: 'unknown', reason: 'Probed before modality probing' }],
    })).toBe('cu130 · python3.12 · general · unverified · capabilities unchecked');
  });

  it('labels extras and runtimes that record none', () => {
    expect(torchExtrasLabel([])).toBe('general');
    expect(torchExtrasLabel(['nunchaku'])).toBe('nunchaku');
    expect(torchExtrasLabel(null)).toBe('unknown extras');
  });
});
