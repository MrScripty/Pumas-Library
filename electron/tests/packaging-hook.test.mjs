import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

test('desktop packages stage notices for the current source version', () => {
  const workspace = JSON.parse(readFileSync(new URL('../../package.json', import.meta.url), 'utf8'));
  const desktop = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
  const notices = desktop.build.extraResources.filter(resource => resource.to === 'THIRD-PARTY-NOTICES.txt');
  assert.equal(notices.length, 1);
  assert.equal(notices[0].from, `../docs/release-attribution/${workspace.version}/THIRD-PARTY-NOTICES.txt`);
});

test('attribution hook accepts Electron Builder beforePack context', () => {
  const result = spawnSync(process.execPath, ['-e', `
    const { createRequire } = require('node:module');
    const builderRequire = createRequire(require.resolve('electron-builder'));
    const { resolveFunction } = builderRequire('app-builder-lib/out/util/resolve');
    const { build } = require('./package.json');
    resolveFunction('commonjs', build.beforePack, 'beforePack', process.cwd())
      .then(hook => hook({ appOutDir: '/tmp/pumas-package', electronPlatformName: 'linux', arch: 'x64' }))
      .catch(error => { console.error(error); process.exitCode = 1; });
  `], { cwd: fileURLToPath(new URL('..', import.meta.url)), encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
});
