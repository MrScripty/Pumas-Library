import type { VersionRelease } from '../types/versions';

/**
 * Which upstream releases the version manager offers, shared by the install
 * list and the "new version" indicator.
 *
 * - An installed version is always listed, even when newer releases or a
 *   pre-release flag would otherwise hide it.
 * - Pre-releases are not offered. The backend marks a release as a
 *   pre-release when its publisher does or when its tag carries a semantic
 *   version pre-release part (`v0.21.3-rc0`).
 * - For semantic version tags, only the newest patch of each major.minor is
 *   offered (`v1.1.3` hides `v1.1.2`; `v1.2.0` is still offered). Tags that
 *   are not semantic versions are offered as published.
 */

interface ParsedVersion {
  major: number;
  minor: number;
  patch: number;
}

// A pre-release part is excluded here: such tags are never stable releases.
function parseStableVersion(tag: string): ParsedVersion | null {
  const match = tag.trim().match(/^v?(\d+)\.(\d+)\.(\d+)(?:\+[0-9A-Za-z.-]+)?$/);
  if (!match) return null;
  return { major: Number(match[1]), minor: Number(match[2]), patch: Number(match[3]) };
}

function compareVersions(a: ParsedVersion, b: ParsedVersion): number {
  return a.major - b.major || a.minor - b.minor || a.patch - b.patch;
}

function newestPatchPerMinor(stable: VersionRelease[]): Set<string> {
  const newest = new Map<string, { release: VersionRelease; version: ParsedVersion }>();
  const offered = new Set<string>();
  for (const release of stable) {
    const version = parseStableVersion(release.tagName);
    if (!version) {
      offered.add(release.tagName);
      continue;
    }
    const line = `${version.major}.${version.minor}`;
    const current = newest.get(line);
    if (!current || compareVersions(version, current.version) > 0) {
      newest.set(line, { release, version });
    }
  }
  for (const { release } of newest.values()) offered.add(release.tagName);
  return offered;
}

/** Releases to list for installation, in upstream order, with installed versions kept. */
export function installableReleaseListing(
  availableVersions: VersionRelease[],
  installedVersions: string[],
): VersionRelease[] {
  const installed = new Set(installedVersions);
  const offered = newestPatchPerMinor(availableVersions.filter((release) => !release.prerelease));
  const rows = availableVersions.filter(
    (release) => installed.has(release.tagName) || offered.has(release.tagName),
  );
  const listed = new Set(rows.map((release) => release.tagName));
  for (const tag of installedVersions) {
    if (!listed.has(tag)) {
      // Installed metadata stays authoritative even if its release was
      // removed upstream or discovery is temporarily unavailable.
      rows.push({ tagName: tag, name: tag, publishedAt: '', prerelease: false });
      listed.add(tag);
    }
  }
  return rows;
}

/**
 * The newest stable release: the highest semantic version when tags are
 * semantic versions, otherwise the first stable release upstream lists.
 */
export function latestStableRelease(availableVersions: VersionRelease[]): VersionRelease | null {
  const stable = availableVersions.filter((release) => !release.prerelease);
  let latest: { release: VersionRelease; version: ParsedVersion } | null = null;
  for (const release of stable) {
    const version = parseStableVersion(release.tagName);
    if (version && (!latest || compareVersions(version, latest.version) > 0)) {
      latest = { release, version };
    }
  }
  return latest?.release ?? stable[0] ?? null;
}
