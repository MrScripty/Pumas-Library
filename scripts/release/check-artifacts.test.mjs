import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { checkArtifacts } from './check-artifacts.mjs';

const version = JSON.parse(fs.readFileSync(new URL('../../package.json', import.meta.url))).version;
const appImage = `Pumas.Library-${version}.AppImage`;
const deb = `pumas-library-electron_${version}_amd64.deb`;
function fixture(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-artifacts-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.writeFileSync(path.join(directory, appImage), 'appimage fixture');
  fs.writeFileSync(path.join(directory, deb), 'deb fixture');
  return directory;
}

test('installer inventory rejects missing, extra, empty and duplicate outputs', t => {
  const directory = fixture(t);
  assert.equal(checkArtifacts(directory, 'linux').length, 2);
  fs.unlinkSync(path.join(directory, deb));
  assert.throws(() => checkArtifacts(directory, 'linux'), /Invalid installer set/);
  fs.writeFileSync(path.join(directory, deb), '');
  assert.throws(() => checkArtifacts(directory, 'linux'), /Empty installer/);
  fs.writeFileSync(path.join(directory, deb), 'deb fixture');
  fs.writeFileSync(path.join(directory, 'stale.exe'), 'stale');
  assert.throws(() => checkArtifacts(directory, 'linux'), /Invalid installer set/);
  fs.unlinkSync(path.join(directory, 'stale.exe'));
  fs.mkdirSync(path.join(directory, 'linux-unpacked'));
  fs.writeFileSync(path.join(directory, 'linux-unpacked', 'internal.exe'), 'internal');
  assert.equal(checkArtifacts(directory, 'linux').length, 2);
  fs.rmSync(path.join(directory, 'linux-unpacked'), { recursive: true });
  for (const name of [`Pumas.Library.Setup.${version}.exe`, `Pumas.Library.${version}.exe`, `Pumas.Library-${version}-arm64.dmg`]) {
    fs.writeFileSync(path.join(directory, name), 'installer fixture');
  }
  assert.equal(checkArtifacts(directory, 'all').length, 5);
  fs.mkdirSync(path.join(directory, 'duplicate'));
  fs.copyFileSync(path.join(directory, deb), path.join(directory, 'duplicate', deb));
  assert.throws(() => checkArtifacts(directory, 'all'), /Invalid installer set/);
});


test('required candidates omit Windows, while Windows and all-platform checks stay strict', t => {
  const directory = fixture(t);
  assert.throws(() => checkArtifacts(directory, 'required'), /Invalid installer set/);
  fs.writeFileSync(path.join(directory, `Pumas.Library-${version}-arm64.dmg`), 'mac fixture');
  assert.equal(checkArtifacts(directory, 'required').length, 3);
  assert.throws(() => checkArtifacts(directory, 'all'), /Invalid installer set/);
  fs.writeFileSync(path.join(directory, `Pumas.Library.Setup.${version}.exe`), 'partial Windows fixture');
  assert.throws(() => checkArtifacts(directory, 'required'), /Invalid installer set/);
  assert.throws(() => checkArtifacts(directory, 'all'), /Invalid installer set/);
  assert.throws(() => checkArtifacts(directory, 'win'), /Invalid installer set/);
});
