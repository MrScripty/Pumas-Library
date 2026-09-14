#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const plan = JSON.parse(fs.readFileSync(path.join(root, 'scripts/release/artifact-plan.json')));
const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'))).version;
const platforms = { linux: 'linux', win: 'windows', mac: 'macos' };

export function checkArtifacts(directory, platform) {
  if (!['all', 'required'].includes(platform) && !(platform in platforms)) throw new Error(`Unknown platform: ${platform}`);
  const targets = plan.desktop_targets.filter(target => platform === 'all'
    || (platform === 'required' ? target.release_gating !== 'best-effort' : target.os === platforms[platform]));
  const expected = plan.artifacts.filter(artifact => targets.some(target => target.id === artifact.target))
    .map(artifact => artifact.filename.replaceAll('{version}', version)).sort();
  // A packaging directory also contains unpacked application executables.
  // Only assembly inputs recurse: upload-artifact carries installers alone.
  const files = fs.readdirSync(directory, { recursive: ['all', 'required'].includes(platform), withFileTypes: true })
    .filter(entry => entry.isFile() && /\.(AppImage|deb|exe|dmg|zip|crate)$/.test(entry.name))
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
