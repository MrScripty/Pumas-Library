import assert from 'node:assert/strict';
import test from 'node:test';
import {
  PythonBridge,
  parseModelDownloadUpdateSseChunk,
  parseModelLibraryUpdateSseChunk,
  parseRuntimeProfileUpdateSseChunk,
  parseServingStatusUpdateSseChunk,
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
