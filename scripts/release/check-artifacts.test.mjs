import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { checkArtifacts } from './check-artifacts.mjs';

const version = JSON.parse(fs.readFileSync(new URL('../../package.json', import.meta.url))).version;
const appImage = `Pumas.Library-${version}.AppImage`;
const deb = `pumas-library-electron_${version}_amd64.deb`;
const fullNames = [
  `Pumas.Library-${version}.AppImage`,
  `pumas-library-electron_${version}_amd64.deb`,
  `Pumas.Library.Setup.${version}.exe`,
  `Pumas.Library.${version}.exe`,
  `Pumas.Library-${version}-arm64.dmg`,
];
const noInferenceNames = [
  `Pumas.Library-no-inference-${version}.AppImage`,
  `pumas-library-electron-no-inference_${version}_amd64.deb`,
  `Pumas.Library.Setup.no-inference.${version}.exe`,
  `Pumas.Library.no-inference.${version}.exe`,
  `Pumas.Library-no-inference-${version}-arm64.dmg`,
];
const headlessNames = [
  `pumas-rpc-no-inference-${version}-linux-x86_64.tar.gz`,
  `pumas-rpc-no-inference-${version}-macos-arm64.tar.gz`,
  `pumas-rpc-no-inference-${version}-windows-x86_64.zip`,
];
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
  for (const name of [...noInferenceNames, ...headlessNames]) {
    fs.writeFileSync(path.join(directory, name), 'variant fixture');
  }
  assert.equal(checkArtifacts(directory, 'all').length, 13);
  fs.mkdirSync(path.join(directory, 'duplicate'));
  fs.copyFileSync(path.join(directory, deb), path.join(directory, 'duplicate', deb));
  assert.throws(() => checkArtifacts(directory, 'all'), /Invalid installer set/);
});


test('required candidates include both Windows installers', t => {
  const directory = fixture(t);
  assert.throws(() => checkArtifacts(directory, 'required'), /Invalid installer set/);
  fs.writeFileSync(path.join(directory, `Pumas.Library-${version}-arm64.dmg`), 'mac fixture');
  assert.throws(() => checkArtifacts(directory, 'required'), /Invalid installer set/);
  assert.throws(() => checkArtifacts(directory, 'all'), /Invalid installer set/);
  fs.writeFileSync(path.join(directory, `Pumas.Library.Setup.${version}.exe`), 'partial Windows fixture');
  assert.throws(() => checkArtifacts(directory, 'required'), /Invalid installer set/);
  assert.throws(() => checkArtifacts(directory, 'all'), /Invalid installer set/);
  assert.throws(() => checkArtifacts(directory, 'win'), /Invalid installer set/);
  fs.writeFileSync(path.join(directory, `Pumas.Library.${version}.exe`), 'portable fixture');
  for (const name of [...noInferenceNames, ...headlessNames]) {
    fs.writeFileSync(path.join(directory, name), 'variant fixture');
  }
  assert.equal(checkArtifacts(directory, 'required').length, 13);
  assert.equal(checkArtifacts(directory, 'all').length, 13);
});

test('variant inventories isolate full, no-inference, and headless cohorts', t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-artifacts-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const cohort = (name, names) => {
    const dir = path.join(directory, name);
    fs.mkdirSync(dir);
    for (const file of names) fs.writeFileSync(path.join(dir, file), 'installer fixture');
    return dir;
  };
  const linuxFull = fullNames.slice(0, 2);
  const winFull = fullNames.slice(2, 4);
  const macFull = fullNames.slice(4, 5);
  assert.equal(checkArtifacts(cohort('linux', linuxFull), 'linux').length, 2);
  assert.equal(checkArtifacts(cohort('win', winFull), 'win').length, 2);
  assert.equal(checkArtifacts(cohort('mac', macFull), 'mac').length, 1);
  assert.equal(checkArtifacts(cohort('linux-no-inf', noInferenceNames.slice(0, 2)), 'linux-no-inference').length, 2);
  assert.equal(checkArtifacts(cohort('win-no-inf', noInferenceNames.slice(2, 4)), 'win-no-inference').length, 2);
  assert.equal(checkArtifacts(cohort('mac-no-inf', noInferenceNames.slice(4, 5)), 'mac-no-inference').length, 1);
  assert.equal(checkArtifacts(cohort('headless-linux', headlessNames.slice(0, 1)), 'headless-linux').length, 1);
  assert.equal(checkArtifacts(cohort('headless-macos', headlessNames.slice(1, 2)), 'headless-macos').length, 1);
  assert.equal(checkArtifacts(cohort('headless-windows', headlessNames.slice(2, 3)), 'headless-windows').length, 1);
  // Full cohorts reject no-inference lookalikes and vice versa.
  assert.throws(() => checkArtifacts(path.join(directory, 'linux'), 'linux-no-inference'), /Invalid installer set/);
  assert.throws(() => checkArtifacts(path.join(directory, 'linux'), 'bogus'), /Unknown platform/);
  // The assembled candidate still carries every cohort.
  for (const entry of fs.readdirSync(directory)) {
    fs.rmSync(path.join(directory, entry), { recursive: true });
  }
  for (const file of [...fullNames, ...noInferenceNames, ...headlessNames]) {
    fs.writeFileSync(path.join(directory, file), 'installer fixture');
  }
  assert.equal(checkArtifacts(directory, 'all').length, 13);
  assert.equal(checkArtifacts(directory, 'required').length, 13);
});
