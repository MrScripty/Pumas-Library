import assert from 'node:assert/strict';
import test from 'node:test';
import { corepackPnpmArgs, installArgs, rootScriptArgs, workspaceScriptArgs } from './package-manager.mjs';

test('installArgs preserves the frozen pnpm workspace install contract', () => {
  assert.deepEqual(corepackPnpmArgs(installArgs()), ['pnpm', 'install', '--frozen-lockfile']);
});

test('workspaceScriptArgs target a workspace path filter and preserve forwarded args', () => {
  assert.deepEqual(
    corepackPnpmArgs(workspaceScriptArgs('./electron', 'dev', ['--devtools'])),
    ['pnpm', '--filter', './electron', 'run', 'dev', '--', '--devtools']
  );
});

test('rootScriptArgs run root-owned scripts without a workspace filter', () => {
  assert.deepEqual(corepackPnpmArgs(rootScriptArgs('test:launcher')), ['pnpm', 'run', 'test:launcher']);
});
