import { describe, expect, it } from 'vitest';
import type { VersionRelease } from '../types/versions';
import { installableReleaseListing, latestStableRelease } from './releaseListing';

function release(tagName: string, prerelease = false): VersionRelease {
  return { tagName, name: tagName, publishedAt: '', prerelease };
}

const tags = (releases: VersionRelease[]) => releases.map((item) => item.tagName);

describe('installable release listing', () => {
  it('offers only the newest patch of each major.minor and every major', () => {
    const available = ['v3.0.0', 'v2.0.1', 'v2.0.0', 'v1.2.1', 'v1.2.0', 'v1.1.3', 'v1.1.2', 'v1.0.0'].map((tag) => release(tag));
    expect(tags(installableReleaseListing(available, []))).toEqual(['v3.0.0', 'v2.0.1', 'v1.2.1', 'v1.1.3', 'v1.0.0']);
  });

  it('never offers pre-releases, flagged or newer than a stable patch', () => {
    const available = [release('b12129', true), release('v0.22.0'), release('v0.21.3-rc0', true), release('v0.21.2'), release('v0.21.1')];
    expect(tags(installableReleaseListing(available, []))).toEqual(['v0.22.0', 'v0.21.2']);
  });

  it('keeps installed versions that a newer patch or a pre-release flag would hide', () => {
    const available = [release('v0.22.0'), release('v0.21.3-rc0', true), release('v0.21.2'), release('v0.21.1')];
    expect(tags(installableReleaseListing(available, ['v0.21.1', 'v0.21.3-rc0']))).toEqual(['v0.22.0', 'v0.21.3-rc0', 'v0.21.2', 'v0.21.1']);
  });

  it('keeps installed versions upstream no longer lists, after the listed releases', () => {
    expect(tags(installableReleaseListing([release('v2.10.0')], ['v2.8.0']))).toEqual(['v2.10.0', 'v2.8.0']);
    expect(tags(installableReleaseListing([], ['v2.8.0']))).toEqual(['v2.8.0']);
  });

  it('offers stable tags that are not semantic versions as published', () => {
    expect(tags(installableReleaseListing([release('b100'), release('b99')], []))).toEqual(['b100', 'b99']);
  });
});

describe('latest stable release', () => {
  it('is the highest stable semantic version, not the first listed or a pre-release', () => {
    const available = [release('b12129', true), release('v0.20.8'), release('v0.22.1-rc1', true), release('v0.22.0'), release('v0.21.2')];
    expect(latestStableRelease(available)?.tagName).toBe('v0.22.0');
  });

  it('falls back to the first stable release when tags are not semantic versions', () => {
    expect(latestStableRelease([release('b2', true), release('b1')])?.tagName).toBe('b1');
    expect(latestStableRelease([release('b2', true)])).toBeNull();
  });
});
