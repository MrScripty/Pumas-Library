import { describe, expect, it } from 'vitest';
import {
  constructQuantUrl,
  getSecurityTier,
  getTrustBadge,
  isHiddenGgufField,
  isPriorityGgufField,
} from './metadataUtils';

describe('metadataUtils', () => {
  it('classifies known model file formats by security tier', () => {
    expect(getSecurityTier('model.gguf')).toBe('safe');
    expect(getSecurityTier('weights.safetensors')).toBe('safe');
    expect(getSecurityTier('checkpoint.pt')).toBe('pickle');
    expect(getSecurityTier('archive.zip')).toBe('unknown');
  });

  it('builds trust badges for supported match methods', () => {
    expect(getTrustBadge({
      repo_id: 'user/model',
      official_name: 'Model',
      family: 'test',
      match_method: 'hash',
      match_confidence: 1.0,
    })?.text).toBe('Verified');

    expect(getTrustBadge({
      repo_id: 'user/model',
      official_name: 'Model',
      family: 'test',
      match_method: 'filename_fuzzy',
      match_confidence: 0.61,
    })?.text).toBe('Possible Match');
  });

  it('derives GGUF metadata visibility and link helpers', () => {
    expect(isPriorityGgufField('general.name')).toBe(true);
    expect(isHiddenGgufField('tokenizer.ggml.tokens', 'token')).toBe(true);
    expect(constructQuantUrl({
      'general.quantized_by': 'user',
      'general.name': 'model',
    })).toBe('https://huggingface.co/user/model');
  });
});
