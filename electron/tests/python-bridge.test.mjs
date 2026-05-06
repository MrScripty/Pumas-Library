import assert from 'node:assert/strict';
import test from 'node:test';
import {
  PythonBridge,
  parseModelLibraryUpdateSseChunk,
  parseRuntimeProfileUpdateSseChunk,
  parseStatusTelemetryUpdateSseChunk,
} from '../dist/python-bridge.js';

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

  bridge.modelLibraryUpdateListener = () => {};
  bridge.modelLibraryUpdateBuffer = 'partial';
  bridge.modelLibraryUpdateCursor = 'model-library-updates:42';
  bridge.modelLibraryUpdateRequest = {
    destroy() {
      destroyed = true;
    },
  };
  bridge.modelLibraryUpdateReconnectTimer = reconnectTimer;
  timers.timers.push({ timer: reconnectTimer, callback: () => {}, delayMs: 1000 });

  await bridge.stop();

  assert.equal(destroyed, true);
  assert.equal(bridge.modelLibraryUpdateListener, null);
  assert.equal(bridge.modelLibraryUpdateRequest, null);
  assert.equal(bridge.modelLibraryUpdateBuffer, '');
  assert.equal(bridge.modelLibraryUpdateCursor, null);
  assert.equal(bridge.modelLibraryUpdateReconnectTimer, null);
  assert.equal(timers.pendingCount(), 0);
});

test('stop clears runtime-profile update stream lifecycle', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);
  let destroyed = false;
  const reconnectTimer = { id: 100 };

  bridge.runtimeProfileUpdateListener = () => {};
  bridge.runtimeProfileUpdateBuffer = 'partial';
  bridge.runtimeProfileUpdateRequest = {
    destroy() {
      destroyed = true;
    },
  };
  bridge.runtimeProfileUpdateReconnectTimer = reconnectTimer;
  timers.timers.push({ timer: reconnectTimer, callback: () => {}, delayMs: 1000 });

  await bridge.stop();

  assert.equal(destroyed, true);
  assert.equal(bridge.runtimeProfileUpdateListener, null);
  assert.equal(bridge.runtimeProfileUpdateRequest, null);
  assert.equal(bridge.runtimeProfileUpdateBuffer, '');
  assert.equal(bridge.runtimeProfileUpdateReconnectTimer, null);
  assert.equal(timers.pendingCount(), 0);
});

test('stop clears status telemetry update stream lifecycle', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);
  let destroyed = false;
  const reconnectTimer = { id: 101 };

  bridge.statusTelemetryUpdateListener = () => {};
  bridge.statusTelemetryUpdateBuffer = 'partial';
  bridge.statusTelemetryUpdateCursor = 'status-telemetry:42';
  bridge.statusTelemetryUpdateRequest = {
    destroy() {
      destroyed = true;
    },
  };
  bridge.statusTelemetryUpdateReconnectTimer = reconnectTimer;
  timers.timers.push({ timer: reconnectTimer, callback: () => {}, delayMs: 1000 });

  await bridge.stop();

  assert.equal(destroyed, true);
  assert.equal(bridge.statusTelemetryUpdateListener, null);
  assert.equal(bridge.statusTelemetryUpdateRequest, null);
  assert.equal(bridge.statusTelemetryUpdateBuffer, '');
  assert.equal(bridge.statusTelemetryUpdateCursor, null);
  assert.equal(bridge.statusTelemetryUpdateReconnectTimer, null);
  assert.equal(timers.pendingCount(), 0);
});

test('model-library update stream reconnect uses bridge timer ownership', async () => {
  const timers = new FakeTimerController();
  const bridge = createBridge(timers);
  let opened = 0;

  bridge.process = {};
  bridge.modelLibraryUpdateListener = () => {};
  bridge.openModelLibraryUpdateStream = () => {
    opened += 1;
  };

  bridge.scheduleModelLibraryUpdateReconnect();

  assert.equal(timers.pendingCount(), 1);
  assert.equal(timers.nextDelay(), 1000);

  await timers.runNext();

  assert.equal(opened, 1);
  assert.equal(bridge.modelLibraryUpdateReconnectTimer, null);
});
