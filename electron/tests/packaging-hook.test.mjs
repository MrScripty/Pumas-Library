import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

test('attribution hook loads when electron is the detected workspace root', () => {
  const result = spawnSync(process.execPath, ['-e', `
    const { createRequire } = require('node:module');
    const builderRequire = createRequire(require.resolve('electron-builder'));
    const { resolveFunction } = builderRequire('app-builder-lib/out/util/resolve');
    const { build } = require('./package.json');
    resolveFunction('commonjs', build.beforePack, 'beforePack', process.cwd())
      .then(hook => hook())
      .catch(error => { console.error(error); process.exitCode = 1; });
  `], { cwd: fileURLToPath(new URL('..', import.meta.url)), encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
});
