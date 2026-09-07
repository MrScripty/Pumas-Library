import assert from 'node:assert/strict';
import test from 'node:test';
import { areInferencePluginsEnabled, resolveReleaseSmokeScript } from './actions.mjs';
import { EXIT_CODES, isGuiEnabled } from './contract.mjs';
import { EventEmitter } from 'node:events';
import { LauncherError } from './errors.mjs';
import { createPlatformService } from './platform-service.mjs';
import { createWindowsPlatformService } from './platform-windows.mjs';

test('platform factory rejects an unsupported operating system', () => {
  assert.throws(
    () => createPlatformService('plan9'),
    (error) => error instanceof LauncherError
      && error.exitCode === EXIT_CODES.UNSUPPORTED_PLATFORM
      && /unsupported platform: plan9/.test(error.message)
  );
});

test('platform factory preserves the accepted operating-system identities', () => {
  assert.equal(createPlatformService('linux').id, 'linux');
  assert.equal(createPlatformService('darwin').id, 'darwin');
  assert.equal(createPlatformService('win32').id, 'win32');
});

test('Windows process adapter bounds and observes its taskkill helper', async () => {
  const helper = new EventEmitter();
  const killSignals = [];
  helper.kill = (signal) => {
    killSignals.push(signal);
    queueMicrotask(() => helper.emit('close', null, signal));
    return true;
  };
  const service = createWindowsPlatformService({
    spawnCommand(command, args, options) {
      assert.equal(command, 'taskkill.exe');
      assert.deepEqual(args, ['/pid', '42', '/t', '/f']);
      assert.equal(options.shell, false);
      return helper;
    },
  });

  await assert.rejects(
    service.processTree.terminate(
      { pid: 42 },
      { force: true, deadlineMs: 100 }
    ),
    (error) => error.code === 'TASKKILL_HELPER_TIMEOUT'
  );
  assert.deepEqual(killSignals, ['SIGKILL']);
});

test('Windows process adapter accepts an observed taskkill success', async () => {
  const helper = createCompletedTaskkill(0);
  const service = createWindowsPlatformService({ spawnCommand: () => helper });

  await service.processTree.terminate(
    { pid: 42 },
    { force: true, deadlineMs: 200 }
  );
});

test('Windows process adapter maps a taskkill failure to a stable code', async () => {
  const helper = createCompletedTaskkill(7);
  const service = createWindowsPlatformService({ spawnCommand: () => helper });

  await assert.rejects(
    service.processTree.terminate(
      { pid: 42 },
      { force: true, deadlineMs: 200 }
    ),
    (error) => error.code === 'TASKKILL_EXIT_7'
  );
});

test('Windows process adapter reports graceful tree termination unavailable', async () => {
  const service = createWindowsPlatformService();

  await assert.rejects(
    service.processTree.terminate(
      { pid: 42 },
      { force: false, deadlineMs: 100 }
    ),
    (error) => error.code === 'WINDOWS_GRACEFUL_TERMINATION_UNAVAILABLE'
  );
});

test('inference plugins are enabled by default and can be compiled out', () => {
  assert.equal(areInferencePluginsEnabled({}), true);
  assert.equal(areInferencePluginsEnabled({ PUMAS_INFERENCE_PLUGINS: 'true' }), true);
  assert.equal(areInferencePluginsEnabled({ PUMAS_INFERENCE_PLUGINS: 'false' }), false);
});

test('GUI selection defaults on and is independent of inference plugins', () => {
  for (const plugins of ['true', 'false']) {
    for (const value of [undefined, 'true']) {
      assert.equal(isGuiEnabled({ PUMAS_GUI: value, PUMAS_INFERENCE_PLUGINS: plugins }), true);
    }
    assert.equal(isGuiEnabled({ PUMAS_GUI: 'false', PUMAS_INFERENCE_PLUGINS: plugins }), false);
  }
});

test('invalid GUI selection is a typed usage failure', () => {
  for (const value of ['', '0', '1', 'FALSE', 'yes', ' true ']) {
    assert.throws(() => isGuiEnabled({ PUMAS_GUI: value }), (error) =>
      error instanceof LauncherError && error.exitCode === EXIT_CODES.USAGE_ERROR
      && error.showUsage && error.message === 'PUMAS_GUI must be true or false');
  }
});

test('resolveReleaseSmokeScript selects the CI-safe Electron entrypoint on Linux CI', () => {
  const previousCi = process.env.CI;
  process.env.CI = 'true';

  try {
    assert.equal(resolveReleaseSmokeScript({ id: 'linux' }), 'run:launcher-release-ci-smoke');
  } finally {
    restoreCi(previousCi);
  }
});

test('resolveReleaseSmokeScript keeps the standard Electron entrypoint outside Linux CI', () => {
  const previousCi = process.env.CI;
  process.env.CI = 'false';

  try {
    assert.equal(resolveReleaseSmokeScript({ id: 'linux' }), 'run:launcher-release');
    assert.equal(resolveReleaseSmokeScript({ id: 'darwin' }), 'run:launcher-release');
    assert.equal(resolveReleaseSmokeScript({ id: 'win32' }), 'run:launcher-release');
  } finally {
    restoreCi(previousCi);
  }
});

function restoreCi(previousCi) {
  if (previousCi === undefined) {
    delete process.env.CI;
    return;
  }

  process.env.CI = previousCi;
}

function createCompletedTaskkill(code) {
  const helper = new EventEmitter();
  helper.kill = () => true;
  queueMicrotask(() => {
    helper.emit('spawn');
    helper.emit('exit', code, null);
    helper.emit('close', code, null);
  });
  return helper;
}
