import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { runInNewContext } from 'node:vm';
import test from 'node:test';
import { setImmediate } from 'node:timers/promises';

const mainUrl = new URL('../dist/main.js', import.meta.url);
const requireMain = createRequire(mainUrl);

function createMainHarness({ root, bridgeRuntime } = {}) {
  const app = new EventEmitter();
  const ipcMain = new EventEmitter();
  const handlers = new Map();
  ipcMain.handle = (channel, handler) => { handlers.set(channel, handler); };
  const timers = new Set();
  const windows = [];
  const exits = [];
  const errors = [];
  let ready;
  const readiness = new Promise((resolve) => { ready = resolve; });
  Object.assign(app, {
    isPackaged: false,
    commandLine: { appendSwitch() {} },
    requestSingleInstanceLock: () => true,
    whenReady: () => readiness,
    getPath: () => root,
    quit() { app.emit('before-quit', { preventDefault() {} }); },
    exit(code) {
      exits.push(code);
      for (const window of windows) if (!window.destroyed) window.destroy();
    },
  });
  class BrowserWindow extends EventEmitter {
    destroyed = false;
    contents = new EventEmitter();
    constructor() {
      super();
      this.contents.isDestroyed = () => this.destroyed;
      this.contents.mainFrame = {};
      windows.push(this);
    }
    static getAllWindows() { return windows.filter((window) => !window.destroyed); }
    get webContents() {
      if (this.destroyed) throw new TypeError('Object has been destroyed');
      return this.contents;
    }
    isDestroyed() { return this.destroyed; }
    async loadFile() {}
    destroy() {
      this.destroyed = true;
      this.emit('closed');
    }
  }
  const logger = {
    transports: { file: {}, console: {} },
    info() {}, warn() {}, error(...args) { errors.push(args); },
  };
  const runtimeProcess = {
    argv: [],
    env: root ? { PUMAS_RPC_BINARY: process.execPath } : {},
    platform: 'linux',
    on() {},
  };
  runInNewContext(readFileSync(mainUrl, 'utf8'), {
    exports: {},
    __dirname: root ? join(root, 'electron', 'dist') : fileURLToPath(new URL('../dist', import.meta.url)),
    process: runtimeProcess,
    setTimeout(callback) { timers.add(callback); return callback; },
    clearTimeout(callback) { timers.delete(callback); },
    require(specifier) {
      if (specifier === 'electron') return {
        app, BrowserWindow, ipcMain,
      };
      if (specifier === 'electron-log') return logger;
      if (specifier === './python-bridge' && bridgeRuntime) return bridgeRuntime.module;
      if (specifier === './launcher-root' && root) {
        const exports = {};
        runInNewContext(readFileSync(new URL('../dist/launcher-root.js', import.meta.url), 'utf8'), {
          exports, require: requireMain, process: runtimeProcess,
        });
        return exports;
      }
      return requireMain(specifier);
    },
  });
  return { app, ipcMain, handlers, timers, windows, ready, exits, errors };
}

test('closing the native window settles presentation without accessing its destroyed getter', async () => {
  const { app, timers, windows } = createMainHarness();
  const activation = app.listeners('activate')[0]();
  // Let the native load finish so the closed lifecycle is installed.
  await setImmediate();
  const [window] = windows;
  try {
    assert.doesNotThrow(() => window.destroy());
    await activation;
    assert.equal(timers.size, 0, 'closed presentation must release its deadline');
    // A later activation must be able to own a new window.
    const replacement = app.listeners('activate')[0]();
    await setImmediate();
    assert.equal(windows.length, 2);
    windows[1].destroy();
    await replacement;
  } finally {
    timers.clear();
  }
});

function createFailedProcessRuntime() {
  const timers = new Set();
  const requests = [];
  const children = [];
  let now = 0;
  const module = {};
  const server = new EventEmitter();
  Object.assign(server, {
    listen(_port, _host, callback) { queueMicrotask(callback); },
    address: () => ({ port: 49152 }),
    close(callback) { callback(); },
  });
  runInNewContext(readFileSync(new URL('../dist/python-bridge.js', import.meta.url), 'utf8'), {
    exports: module,
    Buffer,
    process: { env: {} },
    Date: { now: () => now },
    setTimeout(callback, delayMs) {
      const timer = { callback, delayMs };
      timers.add(timer);
      return timer;
    },
    clearTimeout(timer) { timers.delete(timer); },
    require(specifier) {
      if (specifier === 'electron-log') return { info() {}, warn() {}, error() {} };
      if (specifier === 'net') return { createServer: () => server };
      if (specifier === 'child_process') return {
        spawn() {
          const child = new EventEmitter();
          children.push(child);
          return child;
        },
      };
      if (specifier === 'http') return {
        request() {
          const request = new EventEmitter();
          Object.assign(request, { write() {}, end() {} });
          requests.push(request);
          return request;
        },
      };
      return requireMain(specifier);
    },
  });
  return { module, timers, requests, children, expireStartup() { now = 30_000; } };
}

test('failed backend startup stops its pending restart before application cleanup completes', async () => {
  const root = mkdtempSync(join(tmpdir(), 'pumas-main-lifecycle-'));
  mkdirSync(join(root, 'shared-resources', 'models'), { recursive: true });
  const runtime = createFailedProcessRuntime();
  const harness = createMainHarness({ root, bridgeRuntime: runtime });
  try {
    harness.ready();
    await setImmediate();
    assert.equal(runtime.children.length, 1);
    assert.equal(runtime.requests.length, 1);
    // Exercise the actual compiled main handler, not a copy of its sender guard.
    const contents = harness.windows[0].webContents;
    const bootstrap = { sender: contents, senderFrame: contents.mainFrame };
    harness.ipcMain.emit('launcher:getRootBootstrap', bootstrap);
    assert.equal(bootstrap.returnValue.status, 'ready');
    assert.equal(bootstrap.returnValue.libraryScopeId, null);
    for (const denied of [
      { sender: contents, senderFrame: {} },
      { sender: {}, senderFrame: contents.mainFrame },
    ]) {
      harness.ipcMain.emit('launcher:getRootBootstrap', denied);
      assert.equal(denied.returnValue, null);
    }
    assert.equal(runtime.requests.length, 1, 'bootstrap must not start another backend request');
    runtime.children[0].emit('exit', 1, null);
    assert.equal(runtime.timers.size, 1, 'crash has a pending restart before startup fails');
    runtime.expireStartup();
    runtime.requests[0].emit('error', new Error('connection refused'));
    await setImmediate();
    assert.equal(harness.exits.length, 1, 'failed startup must finish application cleanup');
    assert.equal(runtime.timers.size, 0, 'failed initialization must retain its bridge until restart is stopped');
    assert.ok(harness.errors.some((args) => args.some((value) =>
      value?.message === 'RPC server failed to start within timeout'
    )), 'cleanup must preserve the original startup failure');
  } finally {
    runtime.timers.clear();
    harness.timers.clear();
    rmSync(root, { recursive: true, force: true });
  }
});

test('actual main liveness IPC validates requests and scalar results without retries', async () => {
  const root = mkdtempSync(join(tmpdir(), 'pumas-main-liveness-'));
  mkdirSync(join(root, 'shared-resources', 'models'), { recursive: true });
  const calls = [];
  let response = false;
  class FixtureBridge {
    isRunning() { return true; }
    async start() {}
    async stop() {}
    startModelLibraryUpdateStream() {}
    startModelDownloadUpdateStream() {}
    startRuntimeProfileUpdateStream() {}
    startServingStatusUpdateStream() {}
    startStatusTelemetryUpdateStream() {}
    stopModelLibraryUpdateStream() {}
    stopModelDownloadUpdateStream() {}
    stopRuntimeProfileUpdateStream() {}
    stopServingStatusUpdateStream() {}
    stopStatusTelemetryUpdateStream() {}
    async call(method, params) {
      calls.push({ method, params });
      if (response instanceof Error) throw response;
      return response;
    }
  }
  const harness = createMainHarness({ root, bridgeRuntime: { module: { PythonBridge: FixtureBridge } } });
  try {
    harness.ready();
    await setImmediate();
    const invoke = harness.handlers.get('api:call');
    assert.equal(typeof invoke, 'function');
    for (const method of ['is_ollama_running', 'is_torch_running']) {
      for (const params of [undefined, {}]) {
        for (const value of [true, false]) {
          response = value;
          assert.equal(await invoke({}, method, params), value);
          assert.deepEqual(JSON.parse(JSON.stringify(calls.at(-1))), { method, params: {} });
        }
      }
      const beforeInvalid = calls.length;
      for (const params of [null, [], true, 1, '', { app_id: 'ollama' }, { extra: true }]) {
        await assert.rejects(invoke({}, method, params), /Invalid API params/);
      }
      assert.equal(calls.length, beforeInvalid, 'invalid requests never reach the backend');
      for (const invalid of [undefined, null, 0, 1, '', 'false', [], {}, { success: true }, { running: false }]) {
        response = invalid;
        const before = calls.length;
        await assert.rejects(invoke({}, method, {}), /Desktop contract invalid/);
        assert.equal(calls.length, before + 1, 'malformed reads do not retry');
      }
      response = new Error('liveness transport unavailable');
      const before = calls.length;
      await assert.rejects(invoke({}, method, {}), /liveness transport unavailable/);
      assert.equal(calls.length, before + 1);
    }
    // This slice does not reinterpret responses for other routes.
    response = { success: true, running: false };
    assert.equal(await invoke({}, 'get_app_status', { app_id: 'ollama' }), response);
    assert.deepEqual(harness.errors, []);
  } finally {
    for (const window of harness.windows) window.destroy();
    harness.timers.clear();
    rmSync(root, { recursive: true, force: true });
  }
});
