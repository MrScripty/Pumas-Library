import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import test from 'node:test';
import { StaleInstanceReleaseCoordinator, runStaleInstanceRelease } from '../dist/stale-instance-release.js';

const staleInstance = { pid: 42, startedAt: '2026-11-10T19:40:23Z', version: '0.8.0', leaseChecked: true };
const confirmation = { pid: 42, startedAt: '2026-11-10T19:40:23Z', confirmed: true };
const target = { launcherRoot: '/selected library', rustBinaryPath: '/packaged/pumas-rpc', staleInstance };
const state = { status: 'ready', selectionAction: 'select-library', libraryScopeId: null };
function fixture(overrides = {}) {
  const calls = [];
  const adapter = {
    getTarget: () => target,
    release: async (selected) => { calls.push(['release', selected]); },
    open: async (selected) => { calls.push(['open', selected]); return state; },
    isClosing: () => false,
    reportFailure: (stage) => { calls.push(['failure', stage]); },
    ...overrides,
  };
  return { calls, coordinator: new StaleInstanceReleaseCoordinator(adapter) };
}

test('only a confirmation of exactly the reported entry releases anything', async () => {
  const { coordinator, calls } = fixture();
  for (const input of [null, {}, { ...confirmation, confirmed: false }, { ...confirmation, pid: 43 },
    { ...confirmation, startedAt: '2000-01-01T00:00:00Z' }, { ...confirmation, launcherRoot: '/other' }]) {
    assert.deepEqual(await coordinator.run(input), { status: 'unavailable' });
  }
  assert.deepEqual(calls, []);
});

test('no target or a closing application starts no release', async () => {
  for (const overrides of [{ getTarget: () => null }, { isClosing: () => true }]) {
    const { coordinator, calls } = fixture(overrides);
    assert.deepEqual(await coordinator.run(confirmation), { status: 'unavailable' });
    assert.deepEqual(calls, []);
  }
});

test('a confirmed release opens the library; a refusal carries its reason', async () => {
  const ok = fixture();
  assert.deepEqual(await ok.coordinator.run(confirmation), { status: 'ready', state });
  assert.deepEqual(ok.calls.map(([step]) => step), ['release', 'open']);

  const refused = fixture({ release: async () => { throw new Error('process 42 exists; close it'); } });
  assert.deepEqual(await refused.coordinator.run(confirmation),
    { status: 'failed', stage: 'release', message: 'process 42 exists; close it' });
  assert.deepEqual(refused.calls, [['failure', 'release']]);

  const unopened = fixture({ open: async () => { throw new Error('backend refused'); } });
  assert.deepEqual(await unopened.coordinator.run(confirmation),
    { status: 'failed', stage: 'open', message: 'backend refused' });
});

function fakeChild() {
  const child = new EventEmitter();
  child.stdout = new EventEmitter();
  child.stderr = new EventEmitter();
  return child;
}

test('the release runs the backend with the confirmed entry and no shell', async () => {
  let spawned;
  const child = fakeChild();
  const running = runStaleInstanceRelease(target, (command, args, options) => {
    spawned = { command, args, options };
    return child;
  });
  child.emit('close', 0, null);
  await running;
  assert.equal(spawned.command, '/packaged/pumas-rpc');
  assert.deepEqual(spawned.args, ['--release-stale-instance', '--launcher-root', '/selected library',
    '--expected-pid', '42', '--expected-started-at', '2026-11-10T19:40:23Z']);
  assert.equal(spawned.options.shell, undefined);
});

test('a refused release reports the backend refusal line', async () => {
  const child = fakeChild();
  const running = runStaleInstanceRelease(target, () => child);
  child.stderr.emit('data', Buffer.from('log line\nError: The library\'s instance entry changed since it was confirmed\n'));
  child.emit('close', 1, null);
  await assert.rejects(running, /instance entry changed since it was confirmed/);
});
