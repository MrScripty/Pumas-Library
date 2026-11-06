import assert from 'node:assert/strict';
import test from 'node:test';
import { EventEmitter, once } from 'node:events';
import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { runInNewContext } from 'node:vm';
import {
  BackendReadyReader,
  PythonBridge,
  rpcRequestTimeoutMs,
  parseModelDownloadUpdateSseChunk,
  parseModelLibraryUpdateSseChunk,
  parseRuntimeProfileUpdateSseChunk,
  parseServingStatusUpdateSseChunk,
  parseStatusTelemetryUpdateSseChunk,
} from '../dist/python-bridge.js';

test('managed-Python Torch requests have finite bridge deadlines', () => {
  assert.equal(rpcRequestTimeoutMs('get_torch_release_options'), 900_000);
  assert.equal(rpcRequestTimeoutMs('preview_torch_runtime'), 900_000);
  assert.equal(rpcRequestTimeoutMs('trial_torch_runtime'), 90_000);
  assert.equal(rpcRequestTimeoutMs('find_torch_alternatives'), 900_000);
  assert.equal(rpcRequestTimeoutMs('install_version'), 60_000);
  assert.equal(rpcRequestTimeoutMs('get_torch_runtime_options'), 900_000);
});

test('bridge applies method-specific socket deadlines to Torch requests', async () => {
  const bridgeUrl = new URL('../dist/python-bridge.js', import.meta.url);
  const requireBridge = createRequire(bridgeUrl);
  const exports = {};
  const seen = [];
  const timers = new FakeTimerController();
  runInNewContext(readFileSync(bridgeUrl, 'utf8'), {
    exports,
    Buffer,
    process: { env: {} },
    setTimeout,
    clearTimeout,
    require(specifier) {
      if (specifier === 'electron-log') return { debug() {}, info() {}, warn() {}, error() {} };
      if (specifier === 'http') return {
        request(options, onResponse) {
          seen.push(options);
          const request = new EventEmitter();
          request.write = () => {};
          request.end = () => {
            const response = new EventEmitter();
            onResponse(response);
            response.emit('data', JSON.stringify({ result: { ok: true } }));
            response.emit('end');
          };
          return request;
        },
      };
      return requireBridge(specifier);
    },
  });
  const bridge = new exports.PythonBridge({
    port: 49152, debug: false, rustBinaryPath: process.execPath, launcherRoot: process.cwd(),
    timerController: timers,
  });
  bridge.process = {};

  await bridge.call('get_torch_release_options', { tag: 'v2.10.0' });
  await bridge.call('preview_torch_runtime', { tag: 'v2.10.0' });
  await bridge.call('trial_torch_runtime', { tag: 'v2.10.0', profileId: 'torch-profile' });
  await bridge.call('find_torch_alternatives', { tag: 'v2.10.0', build: 'cpu', python: 'python3.12' });
  await bridge.call('get_torch_runtime_options', {});
  assert.deepEqual(seen.map((request) => request.timeout), [900_000, 900_000, 90_000, 900_000, 900_000]);
  assert.equal(timers.pendingCount(), 0);
});

class FakeTimerController {
  timers = [];
  nextId = 1;

  setTimeout(callback, delayMs) {
    const timer = { id: this.nextId };
    this.nextId += 1;
    this.timers.push({ timer, callback, delayMs });
    return timer;
  }

  clearTimeout(timer) {
    this.timers = this.timers.filter((entry) => entry.timer !== timer);
  }

  pendingCount() {
    return this.timers.length;
  }

  nextDelay() {
    return this.timers[0]?.delayMs ?? null;
  }

  async runNext() {
    assert.ok(this.timers.length > 0);
    const [entry] = this.timers.splice(0, 1);
    await entry.callback();
    return entry.delayMs;
  }
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

async function flushMicrotasks() {
  for (let index = 0; index < 12; index += 1) await Promise.resolve();
}

function fakeChild() {
  const child = new EventEmitter();
  child.pid = 12345;
  child.exitCode = null;
  child.signalCode = null;
  child.stdout = new EventEmitter();
  child.stderr = new EventEmitter();
  child.signals = [];
  child.kill = (signal) => { child.signals.push(signal); return true; };
  child.exit = (code = 0, signal = null) => {
    child.exitCode = code;
    child.signalCode = signal;
    child.emit('exit', code, signal);
  };
  return child;
}

const ANNOUNCED_PORT = 49153;

function readyLine(nonce, port = ANNOUNCED_PORT) {
  return `PUMAS_RPC_READY=${JSON.stringify({ version: 1, port, nonce })}\n`;
}

function stopFixture({ platform = 'linux', options = {}, request, server, announce = true } = {}) {
  const bridgeUrl = new URL('../dist/python-bridge.js', import.meta.url);
  const requireBridge = createRequire(bridgeUrl);
  const exports = {};
  const timers = new FakeTimerController();
  const children = [];
  runInNewContext(readFileSync(bridgeUrl, 'utf8'), {
    exports, Buffer, setTimeout, clearTimeout,
    process: { env: {}, platform },
    require(specifier) {
      if (specifier === 'electron-log') return { debug() {}, info() {}, warn() {}, error() {} };
      if (specifier === 'http' && request) return { request };
      if (specifier === 'net' && server) return { createServer: () => server };
      if (specifier === 'child_process') return {
        spawn(_cmd, args, spawnOptions) {
          const child = fakeChild();
          child.args = args;
          child.env = spawnOptions?.env;
          child.stdio = spawnOptions?.stdio;
          // Like the real backend, report readiness once the listener is bound.
          if (announce) {
            setImmediate(() => child.stdout.emit('data', Buffer.from(readyLine(child.env.PUMAS_RPC_LAUNCH_NONCE))));
          }
          children.push(child);
          return child;
        },
      };
      return requireBridge(specifier);
    },
  });
  const bridge = new exports.PythonBridge({
    port: 49152, debug: false, rustBinaryPath: process.execPath,
    launcherRoot: process.cwd(), timerController: timers, ...options,
  });
  const child = fakeChild();
  bridge.process = child;
  const rpc = deferred();
  const calls = [];
  if (!request) bridge.call = (method) => { calls.push(method); return rpc.promise; };
  return { bridge, child, rpc, calls, timers, children };
}

function assertStopResourcesReleased({ child, timers }) {
  assert.equal(child.listenerCount('exit'), 0);
  assert.equal(timers.pendingCount(), 0);
}

test('shutdown acknowledgement waits for natural exit without a one-second kill', async () => {
  const fixture = stopFixture();
  const { bridge, child, rpc, timers } = fixture;
  let settled = false;
  const stopped = bridge.stop().then(() => { settled = true; });
  rpc.resolve({ status: 'shutting_down' });
  await flushMicrotasks();
  assert.equal(settled, false);
  assert.deepEqual(child.signals, []);
  assert.equal(timers.nextDelay(), 30_000);
  child.exit();
  await stopped;
  assert.equal(bridge.process, null);
  assertStopResourcesReleased(fixture);
});

test('stop observes exit during the shutdown RPC and shares its completed result', async () => {
  const fixture = stopFixture();
  const { bridge, child, calls } = fixture;
  bridge.call = () => { calls.push('shutdown'); child.exit(); return Promise.resolve({ status: 'shutting_down' }); };
  const stopped = bridge.stop();
  assert.equal(bridge.stop(), stopped);
  await stopped;
  assert.equal(bridge.stop(), stopped);
  assert.deepEqual(calls, ['shutdown']);
  assert.deepEqual(child.signals, []);
  assertStopResourcesReleased(fixture);
});

test('natural nonzero exit is a shared failed cleanup receipt', async () => {
  const fixture = stopFixture();
  const { bridge, child, rpc } = fixture;
  const stopped = bridge.stop();
  const rejected = assert.rejects(stopped, /code=7.*cleanup/i);
  rpc.resolve({ status: 'shutting_down' });
  child.exit(7);
  await rejected;
  assert.equal(bridge.stop(), stopped);
  assert.equal(bridge.process, null);
  assertStopResourcesReleased(fixture);
});

test('lost shutdown RPC uses Unix SIGTERM but still waits for the exit receipt', async () => {
  const fixture = stopFixture();
  const { bridge, child, rpc } = fixture;
  const stopped = bridge.stop();
  rpc.reject(new Error('connection reset'));
  await flushMicrotasks();
  assert.deepEqual(child.signals, ['SIGTERM']);
  child.exit();
  await stopped;
  assertStopResourcesReleased(fixture);
});

for (const acknowledgement of [null, {}, { status: 'stopped' }, { status: 'shutting_down', managed_processes: 0 }]) {
  test(`invalid shutdown acknowledgement cannot authorize completion: ${JSON.stringify(acknowledgement)}`, async () => {
    const fixture = stopFixture();
    const { bridge, child, rpc } = fixture;
    const stopped = bridge.stop();
    rpc.resolve(acknowledgement);
    await flushMicrotasks();
    assert.deepEqual(child.signals, ['SIGTERM']);
    child.exit();
    await stopped;
    assertStopResourcesReleased(fixture);
  });
}

test('Windows RPC loss does not mislabel SIGTERM as graceful shutdown', async () => {
  const fixture = stopFixture({ platform: 'win32' });
  const { bridge, child, rpc, timers } = fixture;
  const stopped = bridge.stop();
  const rejected = assert.rejects(stopped, /forced.*cleanup/i);
  rpc.reject(new Error('connection reset'));
  await flushMicrotasks();
  assert.deepEqual(child.signals, []);
  await timers.runNext();
  await flushMicrotasks();
  assert.deepEqual(child.signals, ['SIGKILL']);
  child.exit(0);
  await rejected;
  assertStopResourcesReleased(fixture);
});

test('forced termination waits for delayed exit and rejects even an eventual zero code', async () => {
  const fixture = stopFixture({ options: { shutdownGraceMs: 42, shutdownForceWaitMs: 17 } });
  const { bridge, child, rpc, timers } = fixture;
  let settled = false;
  const stopped = bridge.stop();
  const rejected = assert.rejects(stopped, /forced.*cleanup/i).then(() => { settled = true; });
  rpc.resolve({ status: 'shutting_down' });
  await flushMicrotasks();
  assert.equal(await timers.runNext(), 42);
  await flushMicrotasks();
  assert.deepEqual(child.signals, ['SIGKILL']);
  assert.equal(timers.nextDelay(), 17);
  assert.equal(settled, false);
  child.exit();
  await rejected;
  assert.equal(bridge.process, null);
  assertStopResourcesReleased(fixture);
});

for (const killResult of ['accepted', 'false', 'throws']) {
  test(`unconfirmed forced exit preserves the exact child after kill ${killResult}`, async () => {
    const fixture = stopFixture();
    const { bridge, child, rpc, timers } = fixture;
    child.kill = (signal) => {
      child.signals.push(signal);
      if (killResult === 'throws') throw new Error('permission denied');
      return killResult !== 'false';
    };
    const stopped = bridge.stop();
    const rejected = assert.rejects(stopped, /exit.*unconfirmed.*cleanup/i);
    rpc.resolve({ status: 'shutting_down' });
    await flushMicrotasks();
    await timers.runNext();
    await flushMicrotasks();
    assert.equal(timers.nextDelay(), 5_000);
    await timers.runNext();
    await rejected;
    assert.equal(bridge.process, child);
    assert.equal(bridge.stop(), stopped);
    assert.deepEqual(child.signals, ['SIGKILL']);
    await assert.rejects(bridge.start(), /stopping|unconfirmed/i);
    assertStopResourcesReleased(fixture);
  });
}

test('stop observes synchronous exit triggered by a signal', async () => {
  const fixture = stopFixture();
  const { bridge, child, rpc } = fixture;
  child.kill = (signal) => { child.signals.push(signal); child.exit(); return true; };
  const stopped = bridge.stop();
  rpc.reject(new Error('RPC unavailable'));
  await stopped;
  assert.deepEqual(child.signals, ['SIGTERM']);
  assertStopResourcesReleased(fixture);
});

test('start and a queued restart cannot replace the child while stop is pending', async () => {
  const fixture = stopFixture();
  const { bridge, child, rpc, timers } = fixture;
  bridge.scheduleRestart('Rust');
  const restart = timers.timers[0].callback;
  const stopped = bridge.stop();
  await assert.rejects(bridge.start(), /stopping/i);
  await restart();
  assert.equal(bridge.process, child);
  rpc.resolve({ status: 'shutting_down' });
  child.exit();
  await stopped;
  assertStopResourcesReleased(fixture);
});

test('stop signals and releases only its captured child', async () => {
  const fixture = stopFixture();
  const { bridge, child, rpc } = fixture;
  const stopped = bridge.stop();
  const replacement = fakeChild();
  bridge.process = replacement;
  rpc.reject(new Error('disconnected'));
  await flushMicrotasks();
  assert.deepEqual(child.signals, ['SIGTERM']);
  assert.deepEqual(replacement.signals, []);
  child.exit();
  await stopped;
  assert.equal(bridge.process, replacement);
  assertStopResourcesReleased(fixture);
});

test('overlapping starts share startup and cancelled readiness cannot publish running', async () => {
  const fixture = stopFixture();
  const { bridge, children, rpc, timers } = fixture;
  bridge.process = null;
  const ready = deferred();
  bridge.waitForReady = () => ready.promise;
  const starting = bridge.start();
  const rejected = assert.rejects(starting, /stopped during startup/i);
  assert.equal(bridge.start(), starting);
  const child = children[0];
  assert.ok(child);
  const stopped = bridge.stop();
  await assert.rejects(bridge.start(), /stopping/i);
  rpc.resolve({ status: 'shutting_down' });
  child.exit();
  await assert.rejects(bridge.start(), /stopping/i);
  ready.resolve();
  await rejected;
  await stopped;
  assert.equal(bridge.isRunning(), false);
  assert.equal(timers.pendingCount(), 0);
  assert.equal(children.length, 1);
});

test('late exit from a stopped child cannot erase its replacement or readiness', async () => {
  const { bridge, children, timers } = stopFixture();
  bridge.process = null;
  bridge.waitForReady = async () => {};
  await bridge.start();
  const oldChild = children[0];
  bridge.call = () => Promise.resolve({ status: 'shutting_down' });
  const stopped = bridge.stop();
  oldChild.exit();
  await stopped;
  await bridge.start();
  const current = children[1];
  oldChild.emit('exit', 1, null);
  assert.equal(bridge.process, current);
  assert.equal(bridge.isRunning(), true);
  const finalStop = bridge.stop();
  current.exit();
  await finalStop;
  assert.equal(timers.pendingCount(), 0);
});

for (const autoRestart of [false, true]) {
  test(`stop preserves failure observed by the startup exit listener (autoRestart=${autoRestart})`, async () => {
    const { bridge, children, calls, timers } = stopFixture({ options: { autoRestart } });
    bridge.process = null;
    bridge.waitForReady = async () => {};
    await bridge.start();
    const child = children[0];
    child.exit(9);
    assert.equal(bridge.process, null, 'the live process slot is released by its actual exit listener');
    assert.equal(timers.pendingCount(), autoRestart ? 1 : 0);
    const stopped = bridge.stop();
    await assert.rejects(stopped, /code=9.*cleanup failed/i);
    assert.equal(bridge.stop(), stopped);
    assert.deepEqual(calls, []);
    assert.deepEqual(child.signals, []);
    assert.equal(timers.pendingCount(), 0);
    assert.equal(child.listenerCount('exit'), 1, 'the temporary shutdown observer is removed');
  });
}

for (const reason of ['migration-required', 'backend-unavailable']) {
  test(`initial ${reason} refusal never starts an automatic retry chain`, async () => {
    const { bridge, children, timers } = stopFixture();
    bridge.process = null;
    const health = deferred();
    bridge.call = () => health.promise;
    const starting = bridge.start();
    const rejected = assert.rejects(starting, error => error.reason === reason);
    const child = children[0];
    child.exit(1);
    if (reason === 'migration-required') {
      child.stdout.emit('data', Buffer.from('PUMAS_STARTUP_FAILURE={"version":1,"reason":"migration-required"}\n'));
    }
    child.emit('close', 1, null);
    health.resolve({ status: 'unavailable' });
    await rejected;
    assert.equal(children.length, 1);
    assert.equal(timers.pendingCount(), 0);
    assert.equal(bridge.isRunning(), false);
  });
}

test('failed replacement startups retain the admitted runtime retry budget and backoff', async () => {
  const { bridge, children, timers } = stopFixture({ options: { maxRestarts: 3 } });
  bridge.process = null;
  bridge.call = async () => ({ status: 'ok', launch_nonce: bridge.launchNonce });
  await bridge.start();
  assert.equal(bridge.isRunning(), true);
  children[0].exit(1);

  for (const delayMs of [1000, 2000, 3000]) {
    const health = deferred();
    bridge.call = () => health.promise;
    assert.equal(timers.nextDelay(), delayMs);
    await timers.runNext();
    const starting = bridge.startPromise;
    const rejected = assert.rejects(starting, error => error.reason === 'backend-unavailable');
    const child = children.at(-1);
    child.exit(1);
    child.emit('close', 1, null);
    health.resolve({ status: 'unavailable' });
    await rejected;
    await flushMicrotasks();
    assert.equal(bridge.isRunning(), false);
  }

  assert.equal(children.length, 4, 'one initial child plus the configured three replacement attempts');
  assert.equal(timers.pendingCount(), 0, 'exhausting the budget must not schedule another attempt');
  await assert.rejects(bridge.stop(), /code=1.*cleanup failed/i);
});

test('an unsuccessful restart attempt cannot erase the previous cleanup failure', async () => {
  const { bridge, children, timers } = stopFixture();
  bridge.process = null;
  bridge.waitForReady = async () => {};
  await bridge.start();
  children[0].exit(9);
  const priorStop = bridge.stop();
  await assert.rejects(priorStop, /code=9.*cleanup failed/i);
  bridge.options.rustBinaryPath = '/nonexistent/pumas-rpc-for-stop-receipt-test';
  await assert.rejects(bridge.start(), /Backend binary not found/);
  assert.equal(bridge.stop(), priorStop);
  await assert.rejects(bridge.stop(), /code=9.*cleanup failed/i);
  assert.equal(children.length, 1);
  assert.equal(timers.pendingCount(), 0);
});

test('a new owner replaces the prior failure receipt without accepting stale child exits', async () => {
  const { bridge, children, timers } = stopFixture();
  bridge.process = null;
  bridge.waitForReady = async () => {};
  await bridge.start();
  const previous = children[0];
  previous.exit(9);
  await assert.rejects(bridge.stop(), /code=9.*cleanup failed/i);
  await bridge.start();
  const current = children[1];
  previous.emit('exit', 11, null);
  assert.equal(bridge.process, current);
  current.exit();
  previous.emit('exit', 13, null);
  await bridge.stop();
  assert.equal(timers.pendingCount(), 0);
});

for (const stopIntervenes of [false, true]) {
  test(`startup refusal never schedules a replacement (stop=${stopIntervenes})`, async () => {
    const { bridge, children, timers } = stopFixture();
    bridge.process = null;
    const ready = deferred();
    bridge.waitForReady = () => ready.promise;
    const starting = bridge.start();
    const startupFailed = assert.rejects(starting, /stopped during startup/);
    children[0].exit(9);
    assert.equal(timers.pendingCount(), 0);
    const stopped = stopIntervenes ? bridge.stop() : null;
    const stopFailed = stopped ? assert.rejects(stopped, /code=9.*cleanup failed/i) : null;
    ready.resolve();
    await startupFailed;
    await flushMicrotasks();
    assert.equal(children.length, 1, 'startup failure requires user recovery, not a retry');
    if (stopFailed) await stopFailed;
    else await assert.rejects(bridge.stop(), /code=9.*cleanup failed/i);
    assert.equal(timers.pendingCount(), 0);
  });
}

test('spawn error followed by close during stop records failed-start without waiting for exit', async () => {
  const { bridge, children, timers } = stopFixture();
  bridge.process = null;
  const ready = deferred();
  bridge.waitForReady = () => ready.promise;
  const starting = bridge.start();
  const startupFailed = assert.rejects(starting, /stopped during startup/);
  const child = children[0];
  delete child.pid;
  const stopped = bridge.stop();
  const stopFailed = assert.rejects(stopped, /failed to start.*cleanup/i);
  child.emit('error', new Error('spawn ENOENT'));
  assert.equal(bridge.process, child, 'an error alone does not establish resource closure');
  child.emit('close', -2, null);
  ready.resolve();
  await startupFailed;
  await stopFailed;
  assert.equal(bridge.process, null);
  assert.equal(bridge.stop(), stopped);
  assert.deepEqual(child.signals, []);
  assert.equal(timers.pendingCount(), 0);
});

test('stop does not claim completion while the captured startup is unsettled', async () => {
  const { bridge, children, timers } = stopFixture();
  bridge.process = null;
  const ready = deferred();
  bridge.waitForReady = () => ready.promise;
  const starting = bridge.start();
  const startupFailed = assert.rejects(starting, /stopped during startup/);
  let stopSettled = false;
  const stopped = bridge.stop().then(() => { stopSettled = true; });
  children[0].exit();
  await flushMicrotasks();
  assert.equal(stopSettled, false);
  assert.equal(timers.nextDelay(), 30_000);
  ready.resolve();
  await startupFailed;
  await stopped;
  assert.equal(bridge.startPromise, null);
  assert.equal(timers.pendingCount(), 0);
});

test('unsettled startup after child exit yields bounded unconfirmed cleanup', async () => {
  const { bridge, children, timers } = stopFixture();
  bridge.process = null;
  const ready = deferred();
  bridge.waitForReady = () => ready.promise;
  const starting = bridge.start();
  const startupFailed = assert.rejects(starting, /stopped during startup/);
  const stopped = bridge.stop();
  const stopFailed = assert.rejects(stopped, /startup.*unconfirmed.*cleanup/i);
  children[0].exit();
  await flushMicrotasks();
  await timers.runNext();
  await stopFailed;
  assert.equal(bridge.process, null);
  assert.notEqual(bridge.startPromise, null);
  await assert.rejects(bridge.start(), /stopping/);
  ready.resolve();
  await startupFailed;
  assert.equal(timers.pendingCount(), 0);
});

test('spawn failure recorded before stop still waits for close rather than exitCode alone', async () => {
  const { bridge, children, rpc, timers } = stopFixture();
  bridge.process = null;
  const ready = deferred();
  bridge.waitForReady = () => ready.promise;
  const starting = bridge.start();
  const startupFailed = assert.rejects(starting, /stopped during startup/);
  const child = children[0];
  delete child.pid;
  child.exitCode = -2;
  child.emit('error', new Error('spawn ENOENT'));
  let settled = false;
  const stopped = bridge.stop();
  const stopFailed = assert.rejects(stopped, /failed to start.*cleanup/i).then(() => { settled = true; });
  rpc.reject(new Error('connection refused'));
  await flushMicrotasks();
  assert.equal(settled, false);
  assert.equal(bridge.process, child);
  assert.deepEqual(child.signals, []);
  child.emit('close', -2, null);
  ready.resolve();
  await startupFailed;
  await stopFailed;
  assert.equal(bridge.process, null);
  assert.equal(timers.pendingCount(), 0);
});

function hangingShutdownFixture() {
  const requests = [];
  const fixture = stopFixture({ request() {
    const request = new EventEmitter();
    request.write = () => {};
    request.end = () => {};
    request.destroyed = false;
    request.destroy = () => {
      request.destroyed = true;
      request.emit('error', new Error('aborted'));
    };
    requests.push(request);
    return request;
  } });
  return { ...fixture, requests };
}

test('natural exit cancels a hanging shutdown RPC and removes its deadline', async () => {
  const fixture = hangingShutdownFixture();
  const { bridge, child, timers, requests } = fixture;
  const stopped = bridge.stop();
  assert.equal(requests.length, 1);
  assert.equal(timers.pendingCount(), 2, 'natural-exit grace and RPC deadline are separately owned');
  child.exit();
  await stopped;
  assert.equal(requests[0].destroyed, true);
  assert.equal(bridge.pendingRpcCalls.size, 0);
  assert.deepEqual(child.signals, []);
  assertStopResourcesReleased(fixture);
});

test('whole shutdown grace bounds a hung RPC before forced-exit observation', async () => {
  const fixture = hangingShutdownFixture();
  const { bridge, child, timers, requests } = fixture;
  const stopped = bridge.stop();
  const rejected = assert.rejects(stopped, /exit.*unconfirmed.*cleanup/i);
  assert.equal(await timers.runNext(), 30_000);
  await flushMicrotasks();
  assert.equal(requests[0].destroyed, true);
  assert.equal(bridge.pendingRpcCalls.size, 0);
  assert.deepEqual(child.signals, ['SIGKILL']);
  assert.equal(timers.pendingCount(), 1);
  assert.equal(await timers.runNext(), 5_000);
  await rejected;
  assert.equal(bridge.process, child);
  assertStopResourcesReleased(fixture);
});

test('a previously observed child exit is checked before sending RPC or signals', async () => {
  const fixture = stopFixture();
  const { bridge, child, calls } = fixture;
  child.exit(8);
  await assert.rejects(bridge.stop(), /code=8.*cleanup/i);
  assert.deepEqual(calls, []);
  assert.deepEqual(child.signals, []);
  assertStopResourcesReleased(fixture);
});

test('natural signal termination does not count as a clean backend receipt', async () => {
  const fixture = stopFixture();
  const { bridge, child } = fixture;
  const stopped = bridge.stop();
  child.exit(null, 'SIGTERM');
  await assert.rejects(stopped, /signal=SIGTERM.*cleanup failed/i);
  assertStopResourcesReleased(fixture);
});

test('synchronous forced exit is observed but cannot become successful cleanup', async () => {
  const fixture = stopFixture();
  const { bridge, child, rpc, timers } = fixture;
  child.kill = (signal) => { child.signals.push(signal); child.exit(null, signal); return true; };
  const stopped = bridge.stop();
  const rejected = assert.rejects(stopped, /forced.*SIGKILL.*cleanup unconfirmed/i);
  rpc.resolve({ status: 'shutting_down' });
  await flushMicrotasks();
  await timers.runNext();
  await rejected;
  assertStopResourcesReleased(fixture);
});

test('a failed force attempt followed by exit is not mislabeled as confirmed termination', async () => {
  const fixture = stopFixture();
  const { bridge, child, rpc, timers } = fixture;
  child.kill = () => false;
  const stopped = bridge.stop();
  const rejected = assert.rejects(stopped, (error) => {
    assert.match(error.message, /exit observed after forced termination attempt.*cleanup unconfirmed/i);
    assert.match(error.cause.message, /SIGKILL was not delivered/);
    return true;
  });
  rpc.resolve({ status: 'shutting_down' });
  await flushMicrotasks();
  await timers.runNext();
  await flushMicrotasks();
  child.exit();
  await rejected;
  assertStopResourcesReleased(fixture);
});

test('failed signals retain their cause when forced exit is unconfirmed', async () => {
  const fixture = stopFixture();
  const { bridge, child, rpc, timers } = fixture;
  const denied = new Error('permission denied');
  child.kill = () => { throw denied; };
  const stopped = bridge.stop();
  const rejected = assert.rejects(stopped, (error) => {
    assert.match(error.message, /exit.*unconfirmed/i);
    assert.equal(error.cause, denied);
    return true;
  });
  rpc.reject(new Error('RPC disconnected'));
  await flushMicrotasks();
  await timers.runNext();
  await flushMicrotasks();
  await timers.runNext();
  await rejected;
  assertStopResourcesReleased(fixture);
});

for (const invalid of [0, -1, 1.5, NaN, Infinity, 2_147_483_648]) {
  test(`shutdown options reject an invalid timer duration: ${invalid}`, () => {
    for (const option of ['shutdownGraceMs', 'shutdownForceWaitMs']) {
      assert.throws(() => stopFixture({ options: { [option]: invalid } }), /Shutdown deadlines/);
    }
  });
}

test('bridge wall-clock deadline closes a connected response that keeps sending bytes', { timeout: 5_000 }, async () => {
  const timers = new FakeTimerController();
  let streamed;
  const streaming = new Promise((resolve) => { streamed = resolve; });
  let responseClosed;
  const closed = new Promise((resolve) => { responseClosed = resolve; });
  let chunksSent = 0;
  const server = createServer((_request, response) => {
    response.writeHead(200, { 'Content-Type': 'application/json' });
    response.write('{"result":');
    const interval = setInterval(() => {
      response.write(' ');
      chunksSent += 1;
      if (chunksSent === 2) streamed();
    }, 5);
    response.on('close', () => {
      clearInterval(interval);
      responseClosed();
    });
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const address = server.address();
  assert.ok(address && typeof address !== 'string');
  const bridge = new PythonBridge({
    port: address.port, debug: false, rustBinaryPath: process.execPath,
    launcherRoot: process.cwd(), timerController: timers,
  });
  bridge.port = address.port;
  bridge.process = {};

  try {
    const call = bridge.call('preview_torch_runtime', { tag: 'v2.10.0' });
    void call.catch(() => {});
    await streaming;
    assert.equal(chunksSent >= 2, true);
    assert.equal(timers.nextDelay(), 900_000);
    await timers.runNext();
    await assert.rejects(call, /RPC request timeout/);
    await closed;
    assert.equal(timers.pendingCount(), 0);
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
});

test('stop cancels outstanding RPC calls and releases their deadlines', async () => {
  const bridgeUrl = new URL('../dist/python-bridge.js', import.meta.url);
  const requireBridge = createRequire(bridgeUrl);
  const exports = {};
  const timers = new FakeTimerController();
  const requests = [];
  let destroyed = 0;
  runInNewContext(readFileSync(bridgeUrl, 'utf8'), {
    exports,
    Buffer,
    process: { env: {} },
    setTimeout,
    clearTimeout,
    require(specifier) {
      if (specifier === 'electron-log') return { debug() {}, info() {}, warn() {}, error() {} };
      if (specifier === 'http') return {
        request() {
          const request = new EventEmitter();
          request.write = () => {};
          request.end = () => {
            if (requests.length === 2) request.emit('error', new Error('shutdown unavailable'));
          };
          request.destroy = () => {
            destroyed += 1;
            request.emit('error', new Error('aborted'));
          };
          requests.push(request);
          return request;
        },
      };
      return requireBridge(specifier);
    },
  });
  const bridge = new exports.PythonBridge({
    port: 49152, debug: false, rustBinaryPath: process.execPath,
    launcherRoot: process.cwd(), timerController: timers,
  });
  const child = fakeChild();
  child.kill = () => { child.exit(); return true; };
  bridge.process = child;

  const pending = bridge.call('preview_torch_runtime', { tag: 'v2.10.0' });
  assert.equal(timers.nextDelay(), 900_000);
  const stopped = bridge.stop();
  const late = bridge.call('preview_torch_runtime', { tag: 'v2.10.0' });
  void late.catch(() => {});
  assert.equal(requests.length, 2, 'new RPC calls are rejected after stop begins');
  await assert.rejects(late, /Backend bridge stopping/);
  await assert.rejects(pending, /Backend bridge stopped/);
  await stopped;
  assert.equal(requests.length, 2, 'shutdown is attempted after cancelling pending calls');
  assert.equal(destroyed, 1);
  assert.equal(timers.pendingCount(), 0);
});

function createBridge(timerController) {
  return new PythonBridge({
    port: 49152,
    debug: false,
    autoRestart: true,
    maxRestarts: 3,
    rustBinaryPath: process.execPath,
    launcherRoot: process.cwd(),
    timerController,
  });
}

test('launch binds port 0, exits on stdin EOF and passes a fresh nonce only in the environment', async () => {
  const { bridge, children } = stopFixture({ options: { port: 0 } });
  bridge.process = null;
  bridge.waitForReady = async () => {};
  bridge.call = () => Promise.resolve({ status: 'shutting_down' });
  const nonces = [];
  for (let launch = 0; launch < 2; launch += 1) {
    await bridge.start();
    const child = children[launch];
    const portFlag = child.args.indexOf('--port');
    assert.equal(child.args[portFlag + 1], '0');
    assert.ok(child.args.includes('--exit-on-stdin-eof'));
    assert.equal(child.stdio[0], 'pipe', 'stdin stays piped so parent death reaches the backend');
    const nonce = child.env.PUMAS_RPC_LAUNCH_NONCE;
    assert.match(nonce, /^[0-9a-f]{64}$/);
    assert.ok(child.args.every((arg) => !arg.includes(nonce)), 'argv is readable by other local users');
    nonces.push(nonce);
    const stopped = bridge.stop();
    child.exit();
    await stopped;
  }
  assert.notEqual(nonces[0], nonces[1]);
});

test('readiness uses the announced port and requires the health echo of this launch nonce', async () => {
  const { bridge, children, timers } = stopFixture({ options: { port: 0 } });
  bridge.process = null;
  const answers = [];
  bridge.call = async (method) => {
    assert.equal(method, 'health_check');
    answers.push(bridge.port);
    // Another process answering on the port echoes some other nonce.
    return answers.length === 1
      ? { status: 'ok', launch_nonce: 'another-launch' }
      : { status: 'ok', launch_nonce: bridge.launchNonce };
  };
  const starting = bridge.start();
  for (let turn = 0; turn < 50 && timers.nextDelay() !== 100; turn += 1) {
    await new Promise((resolve) => setImmediate(resolve));
  }
  assert.equal(bridge.isRunning(), false, 'a foreign nonce is not readiness');
  assert.equal(await timers.runNext(), 100);
  await starting;
  assert.deepEqual(answers, [ANNOUNCED_PORT, ANNOUNCED_PORT]);
  assert.equal(bridge.isRunning(), true);
  bridge.call = () => Promise.resolve({ status: 'shutting_down' });
  const stopped = bridge.stop();
  children[0].exit();
  await stopped;
  assert.equal(timers.pendingCount(), 0);
});

test('a readiness line naming another launch nonce refuses startup', async () => {
  const { bridge, children, timers } = stopFixture({ options: { port: 0 }, announce: false });
  bridge.process = null;
  bridge.call = async () => ({ status: 'ok', launch_nonce: bridge.launchNonce });
  const starting = bridge.start();
  const refused = assert.rejects(starting, /does not echo this launch nonce/);
  children[0].stdout.emit('data', Buffer.from(readyLine('a-different-launch')));
  await refused;
  assert.equal(bridge.isRunning(), false);
  bridge.call = () => Promise.resolve({ status: 'shutting_down' });
  const stopped = bridge.stop();
  children[0].exit();
  await stopped;
  assert.equal(timers.pendingCount(), 0);
});

test('the readiness reader joins split chunks and ignores other output', async () => {
  const reader = new BackendReadyReader('n0nce');
  reader.push('RPC_PORT=1234\nordinary log line\nPUMAS_RPC_READY={"version":1,');
  reader.push('"port":4321,"nonce":"n0nce"}\r\n');
  assert.equal(await reader.port, 4321);
  for (const [line, reason] of [
    ['PUMAS_RPC_READY=not-json\n', /not JSON/],
    ['PUMAS_RPC_READY={"version":2,"port":1,"nonce":"n0nce"}\n', /version 2/],
    ['PUMAS_RPC_READY={"version":1,"port":0,"nonce":"n0nce"}\n', /no valid port/],
    ['PUMAS_RPC_READY={"version":1,"port":1,"nonce":null}\n', /does not echo/],
  ]) {
    const refused = new BackendReadyReader('n0nce');
    refused.push(line);
    await assert.rejects(refused.port, reason);
  }
});

test('bridge is not reported running until the RPC server is ready', () => {
  const bridge = createBridge(new FakeTimerController());
  bridge.process = {};

  assert.equal(bridge.isRunning(), false);

  bridge.serverReady = true;
  assert.equal(bridge.isRunning(), true);

  bridge.process = null;
  assert.equal(bridge.isRunning(), false);
});

test('stop clears bridge lifecycle timers when backend is idle', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);

  bridge.startHealthCheck();
  bridge.scheduleRestart('Rust');

  assert.equal(timers.pendingCount(), 2);

  await bridge.stop();

  assert.equal(timers.pendingCount(), 0);
  assert.equal(bridge.healthCheckTimer, null);
  assert.equal(bridge.restartTimer, null);
});

test('health check timer reschedules only while process remains active', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);
  bridge.process = {};
  bridge.call = async () => ({ status: 'ok' });

  bridge.startHealthCheck();
  assert.equal(timers.pendingCount(), 1);

  assert.equal(await timers.runNext(), 30000);
  assert.equal(timers.pendingCount(), 1);

  bridge.process = null;

  assert.equal(await timers.runNext(), 30000);
  assert.equal(timers.pendingCount(), 0);
  assert.equal(bridge.healthCheckTimer, null);
});

test('restart timer uses backoff and clears before scheduling a replacement', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);
  let restartStarts = 0;
  bridge.start = async () => {
    restartStarts += 1;
  };

  bridge.scheduleRestart('Rust');
  assert.equal(timers.pendingCount(), 1);
  assert.equal(timers.nextDelay(), 1000);

  bridge.scheduleRestart('Rust');
  assert.equal(timers.pendingCount(), 1);
  assert.equal(timers.nextDelay(), 2000);

  await timers.runNext();

  assert.equal(restartStarts, 1);
  assert.equal(timers.pendingCount(), 0);
  assert.equal(bridge.restartTimer, null);
});

test('parseModelLibraryUpdateSseChunk parses split model-library update events', () => {
  let parsed = parseModelLibraryUpdateSseChunk(
    '',
    'event: model-library-update\ndata: {"cursor":"model-library-updates:1"'
  );

  assert.deepEqual(parsed.payloads, []);
  assert.notEqual(parsed.buffer, '');

  parsed = parseModelLibraryUpdateSseChunk(
    parsed.buffer,
    ',"events":[],"stale_cursor":false,"snapshot_required":false}\n\n'
  );

  assert.equal(parsed.buffer, '');
  assert.deepEqual(parsed.payloads, [
    {
      cursor: 'model-library-updates:1',
      events: [],
      stale_cursor: false,
      snapshot_required: false,
    },
  ]);
});

test('parseModelLibraryUpdateSseChunk ignores unrelated and malformed events', () => {
  const parsed = parseModelLibraryUpdateSseChunk(
    '',
    [
      'event: message',
      'data: {"cursor":"ignored"}',
      '',
      'event: model-library-update',
      'data: not-json',
      '',
      'event: model-library-update',
      'data: {"cursor":"model-library-updates:2","stale_cursor":false,"snapshot_required":true}',
      '',
      '',
    ].join('\n')
  );

  assert.equal(parsed.buffer, '');
  assert.deepEqual(parsed.payloads, [
    {
      cursor: 'model-library-updates:2',
      stale_cursor: false,
      snapshot_required: true,
    },
  ]);
});

test('parseModelDownloadUpdateSseChunk parses split model download update events', () => {
  let parsed = parseModelDownloadUpdateSseChunk(
    '',
    'event: model-download-update\ndata: {"cursor":"download:1"'
  );

  assert.deepEqual(parsed.payloads, []);
  assert.notEqual(parsed.buffer, '');

  parsed = parseModelDownloadUpdateSseChunk(
    parsed.buffer,
    ',"snapshot":{"cursor":"download:1","revision":1,"downloads":[]},"stale_cursor":false,"snapshot_required":true}\n\n'
  );

  assert.equal(parsed.buffer, '');
  assert.deepEqual(parsed.payloads, [
    {
      cursor: 'download:1',
      snapshot: {
        cursor: 'download:1',
        revision: 1,
        downloads: [],
      },
      stale_cursor: false,
      snapshot_required: true,
    },
  ]);
});

test('parseRuntimeProfileUpdateSseChunk parses split runtime-profile update events', () => {
  let parsed = parseRuntimeProfileUpdateSseChunk(
    '',
    'event: runtime-profile-update\ndata: {"cursor":"runtime-profiles:1"'
  );

  assert.deepEqual(parsed.payloads, []);
  assert.notEqual(parsed.buffer, '');

  parsed = parseRuntimeProfileUpdateSseChunk(
    parsed.buffer,
    ',"events":[],"stale_cursor":true,"snapshot_required":true}\n\n'
  );

  assert.equal(parsed.buffer, '');
  assert.deepEqual(parsed.payloads, [
    {
      cursor: 'runtime-profiles:1',
      events: [],
      stale_cursor: true,
      snapshot_required: true,
    },
  ]);
});

test('parseServingStatusUpdateSseChunk parses split serving-status update events', () => {
  let parsed = parseServingStatusUpdateSseChunk(
    '',
    'event: serving-status-update\ndata: {"cursor":"serving-status:1"'
  );

  assert.deepEqual(parsed.payloads, []);
  assert.notEqual(parsed.buffer, '');

  parsed = parseServingStatusUpdateSseChunk(
    parsed.buffer,
    ',"events":[],"stale_cursor":false,"snapshot_required":true}\n\n'
  );

  assert.equal(parsed.buffer, '');
  assert.deepEqual(parsed.payloads, [
    {
      cursor: 'serving-status:1',
      events: [],
      stale_cursor: false,
      snapshot_required: true,
    },
  ]);
});

test('parseStatusTelemetryUpdateSseChunk parses split status telemetry update events', () => {
  let parsed = parseStatusTelemetryUpdateSseChunk(
    '',
    'event: status-telemetry-update\ndata: {"cursor":"status-telemetry:1"'
  );

  assert.deepEqual(parsed.payloads, []);
  assert.notEqual(parsed.buffer, '');

  parsed = parseStatusTelemetryUpdateSseChunk(
    parsed.buffer,
    ',"snapshot":{"cursor":"status-telemetry:1","revision":1},"stale_cursor":false,"snapshot_required":true}\n\n'
  );

  assert.equal(parsed.buffer, '');
  assert.deepEqual(parsed.payloads, [
    {
      cursor: 'status-telemetry:1',
      snapshot: {
        cursor: 'status-telemetry:1',
        revision: 1,
      },
      stale_cursor: false,
      snapshot_required: true,
    },
  ]);
});

test('parseStatusTelemetryUpdateSseChunk ignores unrelated and malformed events', () => {
  const parsed = parseStatusTelemetryUpdateSseChunk(
    '',
    [
      'event: message',
      'data: {"cursor":"ignored"}',
      '',
      'event: status-telemetry-update',
      'data: not-json',
      '',
      'event: status-telemetry-update',
      'data: {"cursor":"status-telemetry:2","snapshot":{"cursor":"status-telemetry:2","revision":2},"stale_cursor":false,"snapshot_required":true}',
      '',
      '',
    ].join('\n')
  );

  assert.equal(parsed.buffer, '');
  assert.deepEqual(parsed.payloads, [
    {
      cursor: 'status-telemetry:2',
      snapshot: {
        cursor: 'status-telemetry:2',
        revision: 2,
      },
      stale_cursor: false,
      snapshot_required: true,
    },
  ]);
});

test('stop clears model-library update stream lifecycle', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);
  let destroyed = false;
  const reconnectTimer = { id: 99 };

  bridge.modelLibraryUpdateStream.listener = () => {};
  bridge.modelLibraryUpdateStream.buffer = 'partial';
  bridge.modelLibraryUpdateStream.cursor = 'model-library-updates:42';
  bridge.modelLibraryUpdateStream.request = {
    destroy() {
      destroyed = true;
    },
  };
  bridge.modelLibraryUpdateStream.reconnectTimer = reconnectTimer;
  timers.timers.push({ timer: reconnectTimer, callback: () => {}, delayMs: 1000 });

  await bridge.stop();

  assert.equal(destroyed, true);
  assert.equal(bridge.modelLibraryUpdateStream.listener, null);
  assert.equal(bridge.modelLibraryUpdateStream.request, null);
  assert.equal(bridge.modelLibraryUpdateStream.buffer, '');
  assert.equal(bridge.modelLibraryUpdateStream.cursor, null);
  assert.equal(bridge.modelLibraryUpdateStream.reconnectTimer, null);
  assert.equal(timers.pendingCount(), 0);
});

test('stop clears model download update stream lifecycle', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);
  let destroyed = false;
  const reconnectTimer = { id: 102 };

  bridge.modelDownloadUpdateStream.listener = () => {};
  bridge.modelDownloadUpdateStream.buffer = 'partial';
  bridge.modelDownloadUpdateStream.cursor = 'download:42';
  bridge.modelDownloadUpdateStream.request = {
    destroy() {
      destroyed = true;
    },
  };
  bridge.modelDownloadUpdateStream.reconnectTimer = reconnectTimer;
  timers.timers.push({ timer: reconnectTimer, callback: () => {}, delayMs: 1000 });

  await bridge.stop();

  assert.equal(destroyed, true);
  assert.equal(bridge.modelDownloadUpdateStream.listener, null);
  assert.equal(bridge.modelDownloadUpdateStream.request, null);
  assert.equal(bridge.modelDownloadUpdateStream.buffer, '');
  assert.equal(bridge.modelDownloadUpdateStream.cursor, null);
  assert.equal(bridge.modelDownloadUpdateStream.reconnectTimer, null);
  assert.equal(timers.pendingCount(), 0);
});

test('stop clears runtime-profile update stream lifecycle', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);
  let destroyed = false;
  const reconnectTimer = { id: 100 };

  bridge.runtimeProfileUpdateStream.listener = () => {};
  bridge.runtimeProfileUpdateStream.buffer = 'partial';
  bridge.runtimeProfileUpdateStream.request = {
    destroy() {
      destroyed = true;
    },
  };
  bridge.runtimeProfileUpdateStream.reconnectTimer = reconnectTimer;
  timers.timers.push({ timer: reconnectTimer, callback: () => {}, delayMs: 1000 });

  await bridge.stop();

  assert.equal(destroyed, true);
  assert.equal(bridge.runtimeProfileUpdateStream.listener, null);
  assert.equal(bridge.runtimeProfileUpdateStream.request, null);
  assert.equal(bridge.runtimeProfileUpdateStream.buffer, '');
  assert.equal(bridge.runtimeProfileUpdateStream.reconnectTimer, null);
  assert.equal(timers.pendingCount(), 0);
});

test('stop clears serving-status update stream lifecycle', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);
  let destroyed = false;
  const reconnectTimer = { id: 103 };

  bridge.servingStatusUpdateStream.listener = () => {};
  bridge.servingStatusUpdateStream.buffer = 'partial';
  bridge.servingStatusUpdateStream.request = {
    destroy() {
      destroyed = true;
    },
  };
  bridge.servingStatusUpdateStream.reconnectTimer = reconnectTimer;
  timers.timers.push({ timer: reconnectTimer, callback: () => {}, delayMs: 1000 });

  await bridge.stop();

  assert.equal(destroyed, true);
  assert.equal(bridge.servingStatusUpdateStream.listener, null);
  assert.equal(bridge.servingStatusUpdateStream.request, null);
  assert.equal(bridge.servingStatusUpdateStream.buffer, '');
  assert.equal(bridge.servingStatusUpdateStream.cursor, null);
  assert.equal(bridge.servingStatusUpdateStream.reconnectTimer, null);
  assert.equal(timers.pendingCount(), 0);
});

test('stop clears status telemetry update stream lifecycle', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);
  let destroyed = false;
  const reconnectTimer = { id: 101 };

  bridge.statusTelemetryUpdateStream.listener = () => {};
  bridge.statusTelemetryUpdateStream.buffer = 'partial';
  bridge.statusTelemetryUpdateStream.cursor = 'status-telemetry:42';
  bridge.statusTelemetryUpdateStream.request = {
    destroy() {
      destroyed = true;
    },
  };
  bridge.statusTelemetryUpdateStream.reconnectTimer = reconnectTimer;
  timers.timers.push({ timer: reconnectTimer, callback: () => {}, delayMs: 1000 });

  await bridge.stop();

  assert.equal(destroyed, true);
  assert.equal(bridge.statusTelemetryUpdateStream.listener, null);
  assert.equal(bridge.statusTelemetryUpdateStream.request, null);
  assert.equal(bridge.statusTelemetryUpdateStream.buffer, '');
  assert.equal(bridge.statusTelemetryUpdateStream.cursor, null);
  assert.equal(bridge.statusTelemetryUpdateStream.reconnectTimer, null);
  assert.equal(timers.pendingCount(), 0);
});

test('model-library update stream reconnect uses bridge timer ownership', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);
  let opened = 0;

  bridge.process = {};
  bridge.modelLibraryUpdateStream.listener = () => {};
  bridge.modelLibraryUpdateStream.open = () => {
    opened += 1;
  };

  bridge.modelLibraryUpdateStream.scheduleReconnect();

  assert.equal(timers.pendingCount(), 1);
  assert.equal(timers.nextDelay(), 1000);

  await timers.runNext();

  assert.equal(opened, 1);
  assert.equal(bridge.modelLibraryUpdateStream.reconnectTimer, null);
});

test('all named observation streams use separate nonpooled connections', async () => {
  const bridgeUrl = new URL('../dist/python-bridge.js', import.meta.url);
  const requireBridge = createRequire(bridgeUrl);
  const exports = {};
  const seen = [];
  runInNewContext(readFileSync(bridgeUrl, 'utf8'), {
    exports, Buffer, process: { env: {} }, setTimeout, clearTimeout,
    require(specifier) {
      if (specifier === 'electron-log') return { debug() {}, info() {}, warn() {}, error() {} };
      if (specifier === 'http') return {
        get(options) {
          seen.push(options);
          const request = new EventEmitter();
          request.destroy = () => {};
          return request;
        },
      };
      return requireBridge(specifier);
    },
  });
  const bridge = new exports.PythonBridge({
    port: 49152, debug: false, rustBinaryPath: process.execPath, launcherRoot: process.cwd(),
    timerController: new FakeTimerController(),
  });
  bridge.process = {};
  for (const method of ['startModelLibraryUpdateStream', 'startModelDownloadUpdateStream',
    'startRuntimeProfileUpdateStream', 'startServingStatusUpdateStream', 'startStatusTelemetryUpdateStream']) {
    bridge[method](() => {});
  }
  assert.equal(seen.length, 5);
  assert.ok(seen.every((options) => options.agent === false));
  assert.equal(new Set(seen.map((options) => options.path)).size, 5);
  bridge.process = null;
  await bridge.stop();
});

function loggingBridgeFixture({ responseBody = '{}', statusCode = 200 } = {}) {
  const bridgeUrl = new URL('../dist/python-bridge.js', import.meta.url);
  const requireBridge = createRequire(bridgeUrl);
  const exports = {};
  const logs = { debug: [], info: [], warn: [], error: [] };
  const children = [];
  const logger = Object.fromEntries(Object.keys(logs).map((level) => [level, (...args) => logs[level].push(args.join(' '))]));
  runInNewContext(readFileSync(bridgeUrl, 'utf8'), {
    exports, Buffer, setTimeout, clearTimeout,
    process: { env: {} },
    require(specifier) {
      if (specifier === 'electron-log') return logger;
      if (specifier === 'child_process') return {
        spawn() { const child = fakeChild(); children.push(child); return child; },
      };
      if (specifier === 'http') return {
        request(_options, onResponse) {
          const request = new EventEmitter();
          request.write = () => {};
          request.destroy = () => {};
          request.end = () => {
            const response = new EventEmitter();
            response.statusCode = statusCode;
            onResponse(response);
            response.emit('data', responseBody);
            response.emit('end');
          };
          return request;
        },
      };
      return requireBridge(specifier);
    },
  });
  const bridge = new exports.PythonBridge({
    port: 49152, debug: false, rustBinaryPath: process.execPath, launcherRoot: process.cwd(),
    timerController: new FakeTimerController(),
  });
  return { bridge, logs, children };
}

test('an invalid JSON-RPC body is reported by size, never echoed to the renderer', async () => {
  const { bridge } = loggingBridgeFixture({ responseBody: '<html>synthetic-backend-secret</html>', statusCode: 502 });
  bridge.process = {};
  await assert.rejects(bridge.call('get_status', {}), (error) => {
    assert.match(error.message, /Invalid JSON-RPC response for get_status \(HTTP 502, 37 bytes\)/);
    assert.ok(!error.message.includes('synthetic-backend-secret'));
    return true;
  });
});

test('sidecar stdout and stderr stay at debug, out of the persisted info log', async () => {
  const { bridge, logs, children } = loggingBridgeFixture({ responseBody: '{"result":{"status":"starting"}}' });
  const starting = bridge.start();
  const refused = assert.rejects(starting);
  await new Promise((resolve) => setImmediate(resolve));
  const [child] = children;
  child.stdout.emit('data', Buffer.from('stdout synthetic-sidecar-secret\n'));
  child.stderr.emit('data', Buffer.from('INFO stderr synthetic-sidecar-secret\n'));
  child.exit(1);
  child.emit('close', 1, null);
  await refused;
  assert.equal(logs.debug.filter((line) => line.includes('synthetic-sidecar-secret')).length, 2);
  for (const level of ['info', 'warn', 'error']) {
    assert.ok(logs[level].every((line) => !line.includes('synthetic-sidecar-secret')), `${level} must not hold sidecar output`);
  }
});

test('consecutive failed health checks restart a hung backend; a healthy check resets the count', async () => {
  for (const platform of ['linux', 'win32']) {
    const fixture = stopFixture({ platform });
    const { bridge, child, timers } = fixture;
    bridge.serverReady = true;
    const answers = [null, null, { status: 'ok' }, null, null, null];
    bridge.call = async (method) => {
      assert.equal(method, 'health_check');
      return answers.shift();
    };
    bridge.startHealthCheck();
    for (let check = 0; check < 5; check += 1) {
      assert.equal(await timers.runNext(), 30_000);
      assert.deepEqual(child.signals, [], `check ${check + 1} must not restart: the count was reset by a healthy check`);
    }
    assert.equal(await timers.runNext(), 30_000);
    if (platform === 'win32') {
      assert.deepEqual(child.signals, ['SIGKILL']);
      assert.equal(timers.pendingCount(), 0, 'no further health checks after the restart begins');
    } else {
      assert.deepEqual(child.signals, ['SIGTERM']);
      assert.equal(timers.pendingCount(), 1, 'only the force-stop deadline remains');
      assert.equal(await timers.runNext(), 5_000);
      assert.deepEqual(child.signals, ['SIGTERM', 'SIGKILL']);
    }
    assert.equal(timers.pendingCount(), 0);
  }
});

test('a hung backend that exits after SIGTERM is not force-killed', async () => {
  const fixture = stopFixture();
  const { bridge, child, timers } = fixture;
  bridge.serverReady = true;
  bridge.call = async () => null;
  bridge.startHealthCheck();
  for (let check = 0; check < 3; check += 1) await timers.runNext();
  assert.deepEqual(child.signals, ['SIGTERM']);
  bridge.process = null; // the exit handler released the slot
  await timers.runNext();
  assert.deepEqual(child.signals, ['SIGTERM']);
});
