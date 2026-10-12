import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import test from 'node:test';
import { LibraryUpgradeCoordinator, runLibraryMetadataUpgrade, validateLibraryUpgradeConfirmation } from '../dist/library-upgrade.js';

const confirmation = { oldWritersStopped: true, noDowngradeAccepted: true };
const target = { launcherRoot: '/selected library', rustBinaryPath: '/packaged/pumas-rpc' };
const state = { status: 'ready', selectionAction: 'select-library', libraryScopeId: null };
function deferred() { let resolve; let reject; const promise = new Promise((a,b) => { resolve=a; reject=b; }); return { promise, resolve, reject }; }
function fixture(overrides = {}) {
  const calls = [];
  const adapter = {
    getTarget: () => target,
    upgrade: async (selected) => { calls.push(['upgrade', selected]); },
    open: async (selected) => { calls.push(['open', selected]); return state; },
    isClosing: () => false,
    reportFailure: (stage, error) => { calls.push(['failure',stage,error]); },
    ...overrides,
  };
  return { calls, adapter, coordinator: new LibraryUpgradeCoordinator(adapter) };
}

test('upgrade requires both acknowledgments and rejects renderer-selected paths', async () => {
  const { coordinator, calls } = fixture();
  for (const input of [null, {}, { oldWritersStopped: true }, { ...confirmation, noDowngradeAccepted: false }, { ...confirmation, launcherRoot: '/other' }]) {
    assert.throws(() => coordinator.run(input), /both confirmations/);
  }
  assert.deepEqual(calls, []);
  assert.deepEqual(validateLibraryUpgradeConfirmation(confirmation), confirmation);
});

test('unavailable targets and application close start no migration', async () => {
  for (const overrides of [{ getTarget: () => null }, { isClosing: () => true }]) {
    const { coordinator, calls } = fixture(overrides);
    assert.deepEqual(await coordinator.run(confirmation), { status: 'unavailable' });
    assert.deepEqual(calls, []);
  }
});

test('one captured target is upgraded once, then opened and replayed honestly', async () => {
  const hold = deferred();
  const { coordinator, calls } = fixture({ upgrade: async () => { await hold.promise; } });
  const first = coordinator.run(confirmation);
  assert.equal(coordinator.run(confirmation), first);
  assert.equal(coordinator.active, true);
  let settled = false;
  const draining = coordinator.settle().then(() => { settled = true; });
  await Promise.resolve(); assert.equal(settled, false); assert.deepEqual(calls, []);
  hold.resolve();
  assert.deepEqual(await first, { status: 'ready', state });
  await draining;
  assert.equal(coordinator.active, false);
  assert.deepEqual(await coordinator.run(confirmation), { status: 'ready', state });
  assert.deepEqual(calls, [['open', target]]);
});

test('close during metadata work waits for the owned conversion and prevents reopening', async () => {
  const hold = deferred(); let closing = false; let opened = false;
  const { coordinator } = fixture({ upgrade: () => hold.promise, isClosing: () => closing, open: async () => { opened = true; return state; } });
  const running = coordinator.run(confirmation); closing = true;
  assert.deepEqual(await coordinator.run(confirmation), { status: 'unavailable' });
  let settled = false; const draining = coordinator.settle().then(() => { settled = true; });
  await Promise.resolve(); assert.equal(settled, false);
  hold.resolve(); assert.deepEqual(await running, { status: 'upgraded' }); await draining;
  assert.equal(opened, false);
});

test('close during backend open also drains the entire owned operation', async () => {
  const hold = deferred();
  const { coordinator } = fixture({ open: () => hold.promise });
  const running = coordinator.run(confirmation); await Promise.resolve();
  let settled = false; const draining = coordinator.settle().then(() => { settled = true; });
  await Promise.resolve(); assert.equal(settled, false);
  hold.resolve(state); await running; await draining; assert.equal(settled, true);
});

for (const stage of ['upgrade', 'open']) {
  test(`a ${stage} failure never repeats conversion or fabricates readiness`, async () => {
    let mutations = 0;
    const failure = new Error('owned failure');
    const { coordinator, calls } = fixture({
      upgrade: async () => { mutations++; if (stage === 'upgrade') throw failure; },
      open: async () => { throw failure; },
    });
    assert.deepEqual(await coordinator.run(confirmation), { status: 'failed', stage });
    assert.deepEqual(await coordinator.run(confirmation), { status: 'failed', stage });
    assert.equal(mutations, 1);
    assert.deepEqual(calls, [['failure', stage, failure]]);
  });
}

function childFixture() {
  const child = new EventEmitter(); child.stdout = new EventEmitter(); child.stderr = new EventEmitter();
  const calls = [];
  const promise = runLibraryMetadataUpgrade(target, (...args) => { calls.push(args); return child; });
  return { child, calls, promise };
}

test('a diagnostic failure cannot turn uncertain metadata publication into a retry', async () => {
  let mutations = 0;
  const { coordinator } = fixture({
    upgrade: async () => { mutations++; throw new Error('uncertain publication'); },
    reportFailure: () => { throw new Error('logger unavailable'); },
  });
  assert.deepEqual(await coordinator.run(confirmation), { status: 'failed', stage: 'upgrade' });
  assert.deepEqual(await coordinator.run(confirmation), { status: 'failed', stage: 'upgrade' });
  assert.equal(mutations, 1);
});

test('the owned process uses only the explicit metadata CLI for the selected root, without a shell', async () => {
  const { child, calls, promise } = childFixture();
  assert.deepEqual(calls, [[target.rustBinaryPath,
    ['--migrate-download-store-offline', '--launcher-root', target.launcherRoot, '--confirm-old-writers-stopped'],
    { cwd: target.launcherRoot, stdio: ['ignore','pipe','pipe'] }]]);
  let settled = false; promise.then(() => { settled = true; });
  child.emit('exit',0,null); await Promise.resolve(); assert.equal(settled,false);
  child.emit('close',0,null); await promise;
});

test('spawn error waits for stdio closure; diagnostic capture stays bounded', async () => {
  const { child, promise } = childFixture();
  const rejected = assert.rejects(promise, error => error.cause?.message === 'failed spawn' && error.message.length < 8300);
  child.emit('error',new Error('failed spawn'));
  child.stderr.emit('data',Buffer.from('x'.repeat(100000)));
  child.emit('close',null,null); await rejected;
});

test('nonzero and signaled migration exits fail; synchronous spawn failure allocates nothing', async () => {
  for (const [code,signal] of [[1,null],[null,'SIGTERM']]) {
    const { child, promise } = childFixture(); const rejected = assert.rejects(promise,/Metadata upgrade failed/);
    child.emit('close',code,signal); await rejected;
  }
  await assert.rejects(runLibraryMetadataUpgrade(target, () => { throw new Error('allocation failed'); }), /allocation failed/);
});
