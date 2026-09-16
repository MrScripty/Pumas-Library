#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const plan = JSON.parse(fs.readFileSync(path.join(root, 'scripts/release/artifact-plan.json')));
const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'))).version;
const platforms = { linux: 'linux', win: 'windows', mac: 'macos' };
// Variant tokens: `<os>` (full GUI+inference), `<os>-no-inference` (GUI without
// inference plugins), `headless-<os>` (inference-disabled RPC archives).
const headlessPlatforms = {
  'headless-linux': 'linux',
  'headless-macos': 'macos',
  'headless-windows': 'windows',
};
const osByTarget = new Map([
  ...plan.desktop_targets.map(target => [target.id, target.os]),
  ...(plan.headless_targets ?? []).map(target => [target.id, target.os]),
]);

function expectedNames(platform) {
  const wanted = plan.artifacts.filter(artifact => {
    if (platform === 'all' || platform === 'required') return true;
    const os = osByTarget.get(artifact.target);
    if (platform in platforms) {
      return os === platforms[platform] && (artifact.variant ?? 'full') === 'full';
    }
    if (platform.endsWith('-no-inference')) {
      const osKey = platform.replace(/-no-inference$/, '');
      return os === platforms[osKey] && artifact.variant === 'no-inference';
    }
    if (platform in headlessPlatforms) {
      return os === headlessPlatforms[platform] && artifact.variant === 'headless';
    }
    throw new Error(`Unknown platform: ${platform}`);
  });
  return wanted.map(artifact => artifact.filename.replaceAll('{version}', version)).sort();
}

export function checkArtifacts(directory, platform) {
  const expected = expectedNames(platform);
  if (expected.length === 0) throw new Error(`Unknown platform: ${platform}`);
  // A packaging directory also contains unpacked application executables.
  // Only assembly inputs recurse: upload-artifact carries installers alone.
  const files = fs.readdirSync(directory, { recursive: ['all', 'required'].includes(platform), withFileTypes: true })
    .filter(entry => entry.isFile() && /\.(AppImage|deb|exe|dmg|zip|tar\.gz|tgz|crate)$/.test(entry.name))
    .map(entry => path.join(entry.parentPath, entry.name));
  const names = files.map(file => path.basename(file)).sort();
  if (JSON.stringify(names) !== JSON.stringify(expected)) {
    throw new Error(`Invalid installer set. Expected ${JSON.stringify(expected)}; found ${JSON.stringify(names)}`);
  }
  for (const file of files) if (fs.statSync(file).size === 0) throw new Error(`Empty installer: ${file}`);
  return files;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [directory, platform, output] = process.argv.slice(2);
  const files = checkArtifacts(directory, platform);
  if (output) {
    // Refuse stale output instead of mixing different build cohorts.
    fs.mkdirSync(output);
    const checksums = [];
    for (const file of files.sort()) {
      const name = path.basename(file);
      fs.copyFileSync(file, path.join(output, name), fs.constants.COPYFILE_EXCL);
      const hash = createHash('sha256');
      for await (const chunk of fs.createReadStream(path.join(output, name))) hash.update(chunk);
      checksums.push(`${hash.digest('hex')}  ${name}`);
    }
    fs.writeFileSync(path.join(output, 'checksums-sha256.txt'), `${checksums.join('\n')}\n`);
  }
  console.log(`Verified ${files.length} nonempty installers for ${platform} v${version}`);
}
