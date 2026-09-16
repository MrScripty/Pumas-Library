import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const require = createRequire(import.meta.url);
const plan = JSON.parse(fs.readFileSync(path.join(root, 'scripts/release/artifact-plan.json'), 'utf8'));
const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
const config = require('../../electron/electron-builder.no-inference.cjs');

function planFilename(id) {
  const artifact = plan.artifacts.find(entry => entry.id === id);
  assert.ok(artifact, `artifact plan is missing ${id}`);
  return artifact.filename.replaceAll('{version}', version);
}

test('no-inference packaging config matches the artifact plan', () => {
  assert.equal(config.appId, 'com.pumas.library.no-inference');
  assert.equal(
    config.appImage.artifactName.replaceAll('${version}', version).replaceAll('${ext}', 'AppImage'),
    planFilename('desktop-linux-appimage-no-inference'),
  );
  assert.equal(
    config.deb.artifactName.replaceAll('${version}', version).replaceAll('${ext}', 'deb'),
    planFilename('desktop-linux-deb-no-inference'),
  );
  assert.equal(
    config.nsis.artifactName.replaceAll('${version}', version).replaceAll('${ext}', 'exe'),
    planFilename('desktop-windows-nsis-no-inference'),
  );
  assert.equal(
    config.portable.artifactName.replaceAll('${version}', version).replaceAll('${ext}', 'exe'),
    planFilename('desktop-windows-portable-no-inference'),
  );
  assert.equal(
    config.dmg.artifactName.replaceAll('${version}', version).replaceAll('${ext}', 'dmg'),
    planFilename('desktop-macos-dmg-no-inference'),
  );
  // The variant reuses the full packaging contract, not a fork of it.
  const base = require('../../electron/package.json').build;
  assert.equal(config.productName, base.productName);
  assert.equal(config.beforePack, base.beforePack);
  assert.deepEqual(config.files, base.files);
  assert.deepEqual(config.extraResources, base.extraResources);
});
