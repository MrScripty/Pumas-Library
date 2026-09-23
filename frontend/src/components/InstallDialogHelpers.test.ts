import { describe, expect, it } from 'vitest';
import { filterVersions } from './InstallDialogHelpers';

describe('Torch release scope', () => {
  const releases = ['v2.10.0', 'v2.9.1', 'v2.9.0'].map(tagName => ({
    tagName, name: tagName, publishedAt: '', prerelease: false,
  }));

  it('keeps older stable patches reachable and retains local installs without discovery', () => {
    expect(filterVersions(releases, [], false, false, true).map(item => item.tagName))
      .toEqual(['v2.10.0', 'v2.9.1', 'v2.9.0']);
    expect(filterVersions([], ['v2.8.0'], false, true, true).map(item => item.tagName))
      .toEqual(['v2.8.0']);
  });
});
