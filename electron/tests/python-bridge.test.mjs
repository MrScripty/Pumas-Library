import assert from 'node:assert/strict';
import test from 'node:test';
import { PythonBridge } from '../dist/python-bridge.js';

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
