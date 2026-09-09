import { describe, expect, it } from 'vitest';
import { formatMetadataValue, isHiddenGgufField } from './ModelMetadataFieldConfig';

describe('metadata display of decoded JSON', () => {
  it('renders immutable objects and nested arrays without prototype coercion', () => {
    const object: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
    object['nested'] = [true, null, 'exact'];
    Object.freeze(object);
    expect(formatMetadataValue('custom', object)).toBe('{"nested":[true,null,"exact"]}');
    expect(formatMetadataValue('custom', [object, 'tail'])).toBe('{"nested":[true,null,"exact"]}, tail');
    expect(isHiddenGgufField('custom', object)).toBe(false);
  });

  it('retains scalar formatting and uses serialized structured size for collapsed fields', () => {
    expect(formatMetadataValue('match_confidence', 0.75)).toBe('75%');
    expect(formatMetadataValue('custom', true)).toBe('Yes');
    expect(formatMetadataValue('custom', ['a', 'b'])).toBe('a, b');
    expect(isHiddenGgufField('custom', { long: 'x'.repeat(501) })).toBe(true);
  });
});
