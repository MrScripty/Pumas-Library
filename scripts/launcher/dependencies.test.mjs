import assert from 'node:assert/strict';
import test from 'node:test';
import {
  createDependencyPlan,
  ensureDependencyPlan,
  installDependencyPlan,
} from './dependencies.mjs';

const runtime = {
  context: {
    displayName: './launcher.sh',
    electronDir: '/repo/electron',
    frontendDir: '/repo/frontend',
    repoRoot: '/repo',
  },
  platformService: {
    cargoCommand: 'cargo',
    corepackCommand: 'corepack',
  },
};

test('createDependencyPlan checks command and workspace dependency contracts', async () => {
  const commandChecks = [];
  const runCalls = [];
  const existingPaths = new Set([
    '/repo/node_modules',
    '/repo/frontend/node_modules',
    '/repo/electron/node_modules',
  ]);
  const dependencies = createDependencyPlan({
    commandExistsFn(command, args) {
      commandChecks.push({ command, args });
      return true;
    },
    existsSyncFn(path) {
      return existingPaths.has(path);
    },
    async runCommandFn(command, args, options) {
      runCalls.push({ command, args, options });
    },
  });

  assert.deepEqual(dependencies.map((dependency) => dependency.name), [
    'cargo',
    'node',
    'corepack',
    'workspace_node_modules',
  ]);
  assert.equal(dependencies[0].check(runtime), true);
  assert.equal(dependencies[2].check(runtime), true);
  assert.equal(dependencies[3].check(runtime), true);

  await dependencies[3].install(runtime);

  assert.deepEqual(commandChecks, [
    { command: 'cargo', args: ['--version'] },
    { command: 'corepack', args: ['--version'] },
  ]);
  assert.deepEqual(runCalls, [
    {
      command: 'corepack',
      args: ['pnpm', 'install', '--frozen-lockfile'],
      options: { cwd: '/repo' },
    },
  ]);
});

test('installDependencyPlan skips dependencies that already pass checks', async () => {
  const calls = [];
  const dependencies = [
    {
      name: 'already_ready',
      check() {
        calls.push('check');
        return true;
      },
      async install() {
        calls.push('install');
        return true;
      },
    },
  ];

  await installDependencyPlan(dependencies, runtime);

  assert.deepEqual(calls, ['check']);
});

test('installDependencyPlan rechecks after install before marking dependency done', async () => {
  const calls = [];
  const dependencies = [
    {
      name: 'installable',
      check() {
        calls.push('check');
        return calls.length > 2;
      },
      async install() {
        calls.push('install');
        return true;
      },
    },
  ];

  await installDependencyPlan(dependencies, runtime);

  assert.deepEqual(calls, ['check', 'install', 'check']);
});

test('installDependencyPlan fails when install reports failure', async () => {
  const dependencies = [
    {
      name: 'missing_tool',
      check() {
        return false;
      },
      async install() {
        return false;
      },
    },
  ];

  await assert.rejects(
    installDependencyPlan(dependencies, runtime),
    /missing_tool install failed/
  );
});

test('installDependencyPlan fails when install does not satisfy the recheck', async () => {
  const calls = [];
  const dependencies = [
    {
      name: 'still_missing',
      check() {
        calls.push('check');
        return false;
      },
      async install() {
        calls.push('install');
        return true;
      },
    },
  ];

  await assert.rejects(
    installDependencyPlan(dependencies, runtime),
    /still_missing install failed verification/
  );
  assert.deepEqual(calls, ['check', 'install', 'check']);
});

test('ensureDependencyPlan reports the first missing runtime dependency', () => {
  const dependencies = [
    {
      name: 'missing_tool',
      check() {
        return false;
      },
    },
  ];

  assert.throws(
    () => ensureDependencyPlan(dependencies, runtime),
    /missing dependency: missing_tool/
  );
});
