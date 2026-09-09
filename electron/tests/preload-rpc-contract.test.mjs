import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { runInNewContext } from 'node:vm';
import { RPC_METHOD_REGISTRY } from '../dist/rpc-method-registry.js';

const DEFERRED_UNREGISTERED_PRELOAD_METHODS = [];

const PRELOAD_SOURCE = readFileSync(new URL('../src/preload.ts', import.meta.url), 'utf8');
const MAIN_SOURCE = readFileSync(new URL('../src/main.ts', import.meta.url), 'utf8');
const COMPILED_PRELOAD_SOURCE = readFileSync(
  new URL('../dist/preload.js', import.meta.url),
  'utf8'
);
const COMPILED_PRELOAD_PATH = fileURLToPath(
  new URL('../dist/preload.js', import.meta.url)
);
const COMPILED_WINDOW_PRESENTATION_PATH = fileURLToPath(
  new URL('../dist/window-presentation.js', import.meta.url)
);
const ELECTRON_BINARY_PATH = createRequire(import.meta.url)('electron');
const CLOSED_STARTUP_STATES = [
  { status: 'initializing' },
  { status: 'ready', selectionAction: 'select-library', libraryScopeId: null },
  { status: 'ready', selectionAction: 'correct-launch-input', libraryScopeId: null },
  { status: 'ready', selectionAction: 'select-library', libraryScopeId: `display-v1:${'c'.repeat(64)}` },
  ...['invalid', 'unavailable'].flatMap((reason) =>
    ['persisted', 'environment', 'argument'].map((authoritySource) => ({
      status: 'recovery-required',
      reason,
      authoritySource,
      action: authoritySource === 'persisted'
        ? 'select-library'
        : 'correct-launch-input',
    }))
  ),
];
const CLOSED_SELECTION_RESULTS = [
  { status: 'cancelled' },
  { status: 'restarting' },
  { status: 'not-selectable', action: 'correct-launch-input' },
  {
    status: 'recovery-required',
    reason: 'invalid-selection',
    authorityState: 'unchanged',
  },
  {
    status: 'recovery-required',
    reason: 'chooser-unavailable',
    authorityState: 'unchanged',
  },
  ...[
    'unchanged',
    'replacement-visibility-unknown',
    'published-durability-unavailable',
  ].map((authorityState) => ({
    status: 'recovery-required',
    reason: 'persistence-unavailable',
    authorityState,
  })),
  {
    status: 'recovery-required',
    reason: 'restart-unavailable',
    authorityState: 'published',
  },
];
const MALFORMED_STARTUP_STATES = [
  { status: 'ready' },
  { status: 'ready', launcherRoot: '/sensitive/library' },
  { status: 'ready', selectionAction: 'select-library', libraryScopeId: '/sensitive/library' },
  {
    status: 'recovery-required',
    reason: 'invalid',
    authoritySource: 'environment',
    action: 'select-library',
  },
];
const MALFORMED_SELECTION_RESULTS = [
  { success: false, cancelled: true },
  { status: 'not-selectable', action: 'select-library' },
  { status: 'restarting', launcherRoot: '/sensitive/library' },
  {
    status: 'recovery-required',
    reason: 'invalid-selection',
    authorityState: 'replacement-visibility-unknown',
  },
  {
    status: 'recovery-required',
    reason: 'chooser-unavailable',
    authorityState: 'published',
  },
  {
    status: 'recovery-required',
    reason: 'restart-unavailable',
    authorityState: 'unchanged',
  },
];

function loadCompiledPreload() {
  let exposedApi;
  let invocationResult;
  const invocations = [];
  const requiredModules = [];
  const listeners = new Map();
  const module = { exports: {} };
  const electron = {
    contextBridge: {
      exposeInMainWorld: (name, api) => {
        assert.equal(name, 'electronAPI');
        exposedApi = api;
      },
    },
    ipcRenderer: {
      sendSync: (...args) => {
        invocations.push(args);
        return invocationResult;
      },
      invoke: async (...args) => {
        invocations.push(args);
        return invocationResult;
      },
      on: (channel, listener) => {
        const channelListeners = listeners.get(channel) ?? new Set();
        channelListeners.add(listener);
        listeners.set(channel, channelListeners);
      },
      removeListener: (channel, listener) => {
        listeners.get(channel)?.delete(listener);
      },
    },
    webUtils: {
      getPathForFile: () => '',
    },
  };

  runInNewContext(COMPILED_PRELOAD_SOURCE, {
    exports: module.exports,
    module,
    require: (specifier) => {
      requiredModules.push(specifier);
      assert.equal(specifier, 'electron');
      return electron;
    },
  }, { filename: 'dist/preload.js' });
  assert.ok(exposedApi);

  return {
    api: exposedApi,
    emit: (channel) => {
      for (const listener of listeners.get(channel) ?? []) {
        listener({}, undefined);
      }
    },
    invocations,
    requiredModules,
    respondWith: (value) => {
      invocationResult = value;
    },
  };
}

function toPlainValue(value) {
  return JSON.parse(JSON.stringify(value));
}

test('backend setup preload preserves selection and retry tokens without automatic retry', async () => {
  const harness = loadCompiledPreload();
  const token = '2e038924-e0e3-4266-95ef-f7a02997b7b6';
  const setup = {operationId:token, status:'completed', error:null};
  harness.respondWith({success:true, setup});
  for (const backend of ['python_conversion', 'llama_cpp', 'nvfp4', 'sherry']) {
    for (const previous of [undefined, null, token]) {
      const before = harness.invocations.length;
      assert.deepEqual(toPlainValue(await harness.api.start_backend_setup(backend, previous)), {success:true, setup});
      assert.equal(harness.invocations.length, before + 1);
      assert.deepEqual(toPlainValue(harness.invocations.at(-1)), ['api:call', 'start_backend_setup', {backend, expected_previous_operation_id:previous ?? null}]);
    }
    assert.deepEqual(toPlainValue(await harness.api.get_backend_setup(backend)), {success:true, setup});
    assert.deepEqual(toPlainValue(harness.invocations.at(-1)), ['api:call', 'get_backend_setup', {backend}]);
  }
  harness.respondWith({success:true, setup:null});
  assert.deepEqual(toPlainValue(await harness.api.get_backend_setup('nvfp4')), {success:true, setup:null});
  await assert.rejects(harness.api.start_backend_setup('nvfp4'), {status:'invalid'});
  for (const response of [{success:false, setup}, {success:true, setup:{...setup, status:'failed', error:'/private/setup'}}, {success:true, setup:{...setup, status:'failed', error:null}}]) {
    harness.respondWith(response);
    const before = harness.invocations.length;
    await assert.rejects(harness.api.get_backend_setup('sherry'), {status:'invalid'});
    await assert.rejects(harness.api.start_backend_setup('sherry', token), {status:'invalid'});
    assert.equal(harness.invocations.length, before + 2, 'invalid responses never trigger fallback or retry');
  }
  const beforeRejection = harness.invocations.length;
  harness.respondWith(Promise.reject(new Error('RPC method unavailable')));
  await assert.rejects(harness.api.start_backend_setup('nvfp4'), /RPC method unavailable/);
  assert.equal(harness.invocations.length, beforeRejection + 1, 'unsupported RPC never falls back to blocking setup');
  const before = harness.invocations.length;
  for (const backend of [undefined, null, false, 'LlamaCpp', 'unknown']) {
    assert.throws(() => harness.api.get_backend_setup(backend), {status:'invalid'});
    assert.throws(() => harness.api.start_backend_setup(backend), {status:'invalid'});
  }
  for (const previous of ['', token.toUpperCase(), `${token}\n`, 42]) {
    assert.throws(() => harness.api.start_backend_setup('llama_cpp', previous), {status:'invalid'});
  }
  assert.equal(harness.invocations.length, before, 'invalid arguments never reach IPC');
});

test('model import selection crosses the bundled preload as a closed desktop outcome', async () => {
  const { chooseModelImportPaths } = await import('../dist/model-import-picker.js');
  const harness = loadCompiledPreload();
  for (const native of [{canceled:true,filePaths:[]}, {canceled:false,filePaths:['/models/ e\u0301.gguf','/models/ e\u0301.gguf']}]) {
    const produced = await chooseModelImportPaths(async () => native);
    harness.respondWith(produced);
    assert.deepEqual(toPlainValue(await harness.api.open_model_import_dialog()), produced);
  }
  harness.respondWith({status:'selected',paths:[]});
  assert.deepEqual(toPlainValue(await harness.api.open_model_import_dialog()), {status:'invalid'});
  harness.respondWith(Promise.reject(new Error('/private/native-failure')));
  assert.deepEqual(toPlainValue(await harness.api.open_model_import_dialog()), {status:'unavailable'});
  assert.equal(harness.invocations.every(([channel]) => channel === 'dialog:openFile'), true);
});

test('partial recovery bridge sends only the current model ticket request', async () => {
  const harness = loadCompiledPreload();
  const recoveryToken = `v1:${'a'.repeat(64)}`;
  harness.respondWith({ success: false, action: 'none', download_id: null, status: null, reason_code: 'recover_failed', error: 'The partial download could not be resumed.' });
  await harness.api.resume_partial_download('llm/example', recoveryToken);
  assert.deepEqual(toPlainValue(harness.invocations), [
    ['api:call', 'resume_partial_download', { modelId: 'llm/example', recoveryToken }],
  ]);
  assert.equal('recover_download' in harness.api, false);
  assert.equal('list_interrupted_downloads' in harness.api, false);
});

test('launcher bootstrap synchronously decodes terminal display scope without a filesystem path', () => {
  const harness = loadCompiledPreload();
  const ready = {
    status: 'ready', selectionAction: 'select-library', libraryScopeId: `display-v1:${'b'.repeat(64)}`,
  };
  harness.respondWith(ready);
  assert.deepEqual(toPlainValue(harness.api.get_launcher_root_bootstrap()), ready);
  assert.deepEqual(toPlainValue(harness.invocations), [['launcher:getRootBootstrap']]);
  for (const libraryScopeId of ['/models/library', '', undefined]) {
    harness.respondWith({ ...ready, libraryScopeId });
    assert.throws(() => harness.api.get_launcher_root_bootstrap(), /Invalid launcher-root startup state/);
  }
  harness.respondWith({ ...ready, libraryScopeId: null });
  assert.equal(harness.api.get_launcher_root_bootstrap().libraryScopeId, null);
});

function sandboxedPreloadHarnessSource() {
  const startupValues = [
    ...CLOSED_STARTUP_STATES,
    ...MALFORMED_STARTUP_STATES,
  ];
  const selectionValues = [
    ...CLOSED_SELECTION_RESULTS,
    ...MALFORMED_SELECTION_RESULTS,
  ];
  const rendererProbe = `(async () => {
    const startup = [];
    const bootstrap = [];
    const bootstrapErrors = [];
    for (let index = 0; index < ${CLOSED_STARTUP_STATES.length}; index += 1) {
      bootstrap.push(window.electronAPI.get_launcher_root_bootstrap());
    }
    for (let index = 0; index < ${MALFORMED_STARTUP_STATES.length}; index += 1) {
      try { window.electronAPI.get_launcher_root_bootstrap(); }
      catch (error) { bootstrapErrors.push(error.message); }
    }
    const selection = [];
    for (let index = 0; index < ${CLOSED_STARTUP_STATES.length}; index += 1) {
      startup.push(await window.electronAPI.get_launcher_root_state());
    }
    const startupErrors = [];
    for (let index = 0; index < ${MALFORMED_STARTUP_STATES.length}; index += 1) {
      try { await window.electronAPI.get_launcher_root_state(); }
      catch (error) { startupErrors.push(error.message); }
    }
    for (let index = 0; index < ${CLOSED_SELECTION_RESULTS.length}; index += 1) {
      selection.push(await window.electronAPI.select_launcher_root());
    }
    const selectionErrors = [];
    for (let index = 0; index < ${MALFORMED_SELECTION_RESULTS.length}; index += 1) {
      try { await window.electronAPI.select_launcher_root(); }
      catch (error) { selectionErrors.push(error.message); }
    }
    let timeoutCalls = 0;
    const unsubscribeTimeout = window.electronAPI.onLauncherRootPresentationTimeout(() => {
      timeoutCalls += 1;
    });
    await window.electronAPI.notify_launcher_root_presentation_committed('ready');
    unsubscribeTimeout();
    return {
      hasBridge: typeof window.electronAPI === 'object',
      startup,
      bootstrap,
      bootstrapErrors,
      startupErrors,
      selection,
      selectionErrors,
      timeoutCalls,
    };
  })()`;

  return `
const { app, BrowserWindow, ipcMain } = require('electron');
const preloadPath = process.argv.at(-2);
const windowPresentationPath = process.argv.at(-1);
const {
  createWindowPresentationOwner,
  frameContainsWindowPresentationMarker,
  WINDOW_PRESENTATION_MARKER_CSS,
} = require(windowPresentationPath);
const startupValues = ${JSON.stringify(startupValues)};
const selectionValues = ${JSON.stringify(selectionValues)};
const expectedStartup = ${JSON.stringify(CLOSED_STARTUP_STATES)};
const expectedSelection = ${JSON.stringify(CLOSED_SELECTION_RESULTS)};
let startupIndex = 0;
let bootstrapIndex = 0;
let selectionIndex = 0;
const presentationCommits = [];
let presentationOwner;
let oracleWindow;
ipcMain.handle('launcher:getRootState', () => startupValues[startupIndex++]);
ipcMain.on('launcher:getRootBootstrap', (event) => {
  event.returnValue = event.sender === oracleWindow?.webContents &&
    event.senderFrame === oracleWindow.webContents.mainFrame
    ? startupValues[bootstrapIndex++] : null;
});
ipcMain.handle('launcher:chooseLibraryRoot', () => selectionValues[selectionIndex++]);
ipcMain.handle('launcher-root:presentation-committed', (event, presentation) => {
  presentationCommits.push(presentation);
  if (
    presentationOwner &&
    oracleWindow &&
    event.sender === oracleWindow.webContents &&
    event.senderFrame === oracleWindow.webContents.mainFrame
  ) {
    presentationOwner.rendererCommitted({
      currentTopDocument: true,
      presentation,
    });
  }
});
const deadline = setTimeout(() => {
  process.stderr.write('PUMAS_PRELOAD_ORACLE_TIMEOUT\\n');
  app.exit(2);
}, 10000);

app.whenReady().then(async () => {
  const failures = [];
  const window = new BrowserWindow({
    show: false,
    webPreferences: {
      preload: preloadPath,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
    },
  });
  oracleWindow = window;
  let frameSubscription;
  let markerFrames = 0;
  let markerFreeFramesAfterMarker = 0;
  let showCalls = 0;
  let settleShown;
  const shown = new Promise((resolve) => {
    settleShown = resolve;
  });
  presentationOwner = createWindowPresentationOwner({
    getAuthoritativeStatus: () => 'ready',
    schedule: (callback, delayMs) => setTimeout(callback, delayMs),
    clearScheduled: (handle) => clearTimeout(handle),
    subscribeToPresentationFrames: (callback) => {
      frameSubscription = callback;
      window.webContents.beginFrameSubscription(false, callback);
    },
    unsubscribeFromPresentationFrames: () => {
      window.webContents.endFrameSubscription();
      frameSubscription = undefined;
    },
    insertPresentationMarker: (onInserted, onUnavailable) => {
      void window.webContents.insertCSS(WINDOW_PRESENTATION_MARKER_CSS).then(
        onInserted,
        onUnavailable
      );
    },
    removePresentationMarker: (markerHandle, onRemoved, onUnavailable) => {
      if (typeof markerHandle !== 'string') {
        onUnavailable();
        return;
      }
      void window.webContents.removeInsertedCSS(markerHandle).then(
        onRemoved,
        onUnavailable
      );
    },
    invalidatePresentationFrame: () => window.webContents.invalidate(),
    frameContainsPresentationMarker: (frame) => {
      const containsMarker = frameContainsWindowPresentationMarker(frame);
      if (containsMarker) {
        markerFrames += 1;
      } else if (markerFrames > 0) {
        markerFreeFramesAfterMarker += 1;
      }
      return containsMarker;
    },
    showWindow: () => {
      showCalls += 1;
      if (markerFrames < 1 || markerFreeFramesAfterMarker < 1 || frameSubscription) {
        failures.push('show-before-marker-barrier');
      }
      window.show();
      settleShown();
    },
    focusWindow: () => window.focus(),
    reportFocusUnavailable: () => failures.push('focus-unavailable'),
    sendVisibilityTimeout: () => failures.push('unexpected-timeout'),
    showNativeFatal: () => failures.push('native-fatal'),
    destroyWindow: () => window.destroy(),
    quitApplication: () => failures.push('fatal-quit'),
  }, {
    presentationDeadlineMs: 30_000,
    fallbackGraceMs: 2_000,
  });
  window.on('ready-to-show', () => presentationOwner.browserReady());
  window.webContents.on('did-frame-finish-load', (_event, isMainFrame) => {
    if (isMainFrame) {
      presentationOwner.documentReady();
    }
  });
  window.webContents.on('preload-error', () => failures.push('preload-error'));
  window.webContents.on('did-fail-load', () => failures.push('did-fail-load'));
  window.webContents.on('render-process-gone', () => failures.push('render-process-gone'));
  window.webContents.on('unresponsive', () => failures.push('unresponsive'));
  await window.loadURL('data:text/html,<html><body>preload oracle</body></html>');
  window.webContents.send('launcher-root:presentation-timeout');
  const result = await window.webContents.executeJavaScript(${JSON.stringify(rendererProbe)});
  await shown;
  const correctStartupErrors = result.startupErrors.length === ${MALFORMED_STARTUP_STATES.length} &&
    result.startupErrors.every((message) => message === 'Invalid launcher-root startup state.');
  const correctSelectionErrors = result.selectionErrors.length === ${MALFORMED_SELECTION_RESULTS.length} &&
    result.selectionErrors.every((message) => message === 'Invalid launcher-root selection result.');
  const correct = failures.length === 0 &&
    result.hasBridge &&
    JSON.stringify(result.bootstrap) === JSON.stringify(expectedStartup) &&
    result.bootstrapErrors.length === ${MALFORMED_STARTUP_STATES.length} &&
    result.bootstrapErrors.every((message) => message === 'Invalid launcher-root startup state.') &&
    JSON.stringify(result.startup) === JSON.stringify(expectedStartup) &&
    JSON.stringify(result.selection) === JSON.stringify(expectedSelection) &&
    correctStartupErrors &&
    correctSelectionErrors &&
    result.timeoutCalls === 1 &&
    JSON.stringify(presentationCommits) === JSON.stringify(['ready']) &&
    showCalls === 1 &&
    markerFrames >= 1 &&
    markerFreeFramesAfterMarker >= 1;
  clearTimeout(deadline);
  window.destroy();
  process.stdout.write(correct ? 'PUMAS_PRELOAD_ORACLE_OK\\n' : 'PUMAS_PRELOAD_ORACLE_MISMATCH\\n');
  app.exit(correct ? 0 : 1);
}).catch(() => {
  clearTimeout(deadline);
  process.stderr.write('PUMAS_PRELOAD_ORACLE_FAILURE\\n');
  app.exit(1);
});
`;
}

async function runSandboxedPreloadOracle() {
  const temporaryDirectory = mkdtempSync(join(tmpdir(), 'pumas-preload-oracle-'));
  const harnessPath = join(temporaryDirectory, 'main.cjs');
  writeFileSync(harnessPath, sandboxedPreloadHarnessSource(), 'utf8');
  const environment = { ...process.env };
  delete environment.ELECTRON_RUN_AS_NODE;

  try {
    const child = spawn(ELECTRON_BINARY_PATH, [
      '--disable-gpu',
      '--disable-dev-shm-usage',
      harnessPath,
      COMPILED_PRELOAD_PATH,
      COMPILED_WINDOW_PRESENTATION_PATH,
    ], {
      detached: true,
      env: environment,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });

    let forcedTermination = false;
    const outcome = await new Promise((resolve, reject) => {
      const deadline = setTimeout(() => {
        forcedTermination = true;
        try {
          process.kill(-child.pid, 'SIGKILL');
        } catch (error) {
          if (error.code !== 'ESRCH') {
            reject(error);
          }
        }
      }, 12_000);
      child.once('error', (error) => {
        clearTimeout(deadline);
        reject(error);
      });
      child.once('close', (code, signal) => {
        clearTimeout(deadline);
        resolve({ code, signal, forcedTermination });
      });
    });

    await waitForProcessGroupExit(child.pid, 2_000);
    assert.deepEqual(
      outcome,
      { code: 0, signal: null, forcedTermination: false },
      `${stdout}\n${stderr}`
    );
    assert.match(stdout, /PUMAS_PRELOAD_ORACLE_OK/);
    assert.doesNotMatch(
      `${stdout}\n${stderr}`,
      /Unable to load preload|module not found|preload-error/i
    );
  } finally {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
}

async function waitForProcessGroupExit(processGroupId, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (processGroupExists(processGroupId) && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  assert.equal(processGroupExists(processGroupId), false);
}

function processGroupExists(processGroupId) {
  try {
    process.kill(-processGroupId, 0);
    return true;
  } catch (error) {
    if (error.code === 'ESRCH') {
      return false;
    }
    throw error;
  }
}

function preloadRpcMethodNames() {
  return [
    ...new Set(
      [...PRELOAD_SOURCE.matchAll(/apiCall\('([^']+)'/g)]
        .map((match) => match[1])
        .filter((methodName) => methodName !== undefined)
    ),
  ].sort();
}

test('preload apiCall methods are registered or tracked deferred drift', () => {
  const registeredMethods = new Set(RPC_METHOD_REGISTRY.methods);
  const unregisteredMethods = preloadRpcMethodNames()
    .filter((methodName) => !registeredMethods.has(methodName))
    .sort();

  assert.deepEqual(unregisteredMethods, DEFERRED_UNREGISTERED_PRELOAD_METHODS);
});

test('preload exposes the status telemetry snapshot method used by the renderer', () => {
  assert.match(
    PRELOAD_SOURCE,
    /get_status_telemetry_snapshot:\s*\(\)\s*=>\s*apiCall\('get_status_telemetry_snapshot'\)/
  );
});

test('preload runtime-decodes launcher-root startup and selection IPC values', () => {
  assert.match(
    PRELOAD_SOURCE,
    /get_launcher_root_state:\s*async\s*\(\)\s*=>\s*\{[\s\S]*?decodeLauncherRootStartupState\([\s\S]*?launcher:getRootState/
  );
  assert.match(
    PRELOAD_SOURCE,
    /select_launcher_root:\s*async\s*\(\)\s*=>\s*\{[\s\S]*?decodeLauncherRootSelectionResult\([\s\S]*?launcher:chooseLibraryRoot/
  );
});

test('compiled sandboxed preload loads only the accepted Electron runtime module', () => {
  assert.deepEqual(loadCompiledPreload().requiredModules, ['electron']);
});

test('compiled preload accepts every closed launcher-root state', async () => {
  const runtime = loadCompiledPreload();

  for (const state of CLOSED_STARTUP_STATES) {
    runtime.respondWith(state);
    assert.deepEqual(toPlainValue(await runtime.api.get_launcher_root_state()), state);
  }
  for (const result of CLOSED_SELECTION_RESULTS) {
    runtime.respondWith(result);
    assert.deepEqual(toPlainValue(await runtime.api.select_launcher_root()), result);
  }
});

test('compiled preload rejects malformed launcher-root values without private detail', async () => {
  const runtime = loadCompiledPreload();

  for (const state of MALFORMED_STARTUP_STATES) {
    runtime.respondWith(state);
    await assert.rejects(
      runtime.api.get_launcher_root_state(),
      (error) => {
        assert.equal(error.name, 'LauncherRootRecoveryContractError');
        assert.equal(error.code, 'launcher_root_contract_invalid');
        assert.equal(error.message, 'Invalid launcher-root startup state.');
        return true;
      }
    );
  }
  for (const result of MALFORMED_SELECTION_RESULTS) {
    runtime.respondWith(result);
    await assert.rejects(
      runtime.api.select_launcher_root(),
      (error) => {
        assert.equal(error.name, 'LauncherRootRecoveryContractError');
        assert.equal(error.code, 'launcher_root_contract_invalid');
        assert.equal(error.message, 'Invalid launcher-root selection result.');
        return true;
      }
    );
  }
});

test('compiled preload exposes a one-shot presentation acknowledgement and latched timeout', async () => {
  const runtime = loadCompiledPreload();
  runtime.emit('launcher-root:presentation-timeout');
  let timeoutCalls = 0;
  const unsubscribe = runtime.api.onLauncherRootPresentationTimeout(() => {
    timeoutCalls += 1;
  });

  assert.equal(timeoutCalls, 1);
  await runtime.api.notify_launcher_root_presentation_committed('ready');
  assert.deepEqual(toPlainValue(runtime.invocations.at(-1)), [
    'launcher-root:presentation-committed',
    'ready',
  ]);
  unsubscribe();
  runtime.emit('launcher-root:presentation-timeout');
  assert.equal(timeoutCalls, 1);
});

test('pinned Electron loads the compiled preload in the production sandbox', {
  skip: process.platform !== 'linux' ||
    process.env.PUMAS_RUN_REAL_ELECTRON_PRELOAD_ORACLE !== '1',
  timeout: 20_000,
}, async () => {
  await runSandboxedPreloadOracle();
});

test('main composes one selection owner and requests relaunch before delayed quit', () => {
  const ownerStart = MAIN_SOURCE.indexOf(
    'const selectLauncherRoot = createLauncherRootSelectionHandler({'
  );
  const handlerStart = MAIN_SOURCE.indexOf(
    "ipcMain.handle('launcher:getRootState'"
  );
  assert.ok(ownerStart >= 0);
  assert.ok(handlerStart > ownerStart);

  const ownerSource = MAIN_SOURCE.slice(ownerStart, handlerStart);
  const relaunchCall = ownerSource.indexOf('app.relaunch();');
  const delayedQuit = ownerSource.indexOf('setTimeout(');
  assert.ok(relaunchCall >= 0);
  assert.ok(delayedQuit > relaunchCall);
  assert.match(
    MAIN_SOURCE.slice(handlerStart),
    /launcher:chooseLibraryRoot[\s\S]*?await selectLauncherRoot\(launcherRootStartupState\)/
  );
});

test('main delegates first visibility to one current-document presentation owner', () => {
  assert.match(
    MAIN_SOURCE,
    /const WINDOW_PRESENTATION_DEADLINE_MS = 30_000;[\s\S]*?const WINDOW_PRESENTATION_FALLBACK_GRACE_MS = 2_000;/
  );
  assert.match(
    MAIN_SOURCE,
    /createWindowPresentationOwner\([\s\S]*?presentationDeadlineMs:\s*WINDOW_PRESENTATION_DEADLINE_MS[\s\S]*?fallbackGraceMs:\s*WINDOW_PRESENTATION_FALLBACK_GRACE_MS/
  );
  assert.match(
    MAIN_SOURCE,
    /ipcMain\.handle\([\s\S]*?LAUNCHER_ROOT_PRESENTATION_COMMITTED_CHANNEL[\s\S]*?event\.sender\s*===\s*targetWindow\.webContents[\s\S]*?event\.senderFrame\s*===\s*targetWindow\.webContents\.mainFrame/
  );
  assert.match(MAIN_SOURCE, /handleBrowserReady[\s\S]*?\.browserReady\(\)/);
  assert.match(MAIN_SOURCE, /\.on\('ready-to-show',\s*handleBrowserReady\)/);
  assert.doesNotMatch(MAIN_SOURCE, /\.on\('ready-to-show',[\s\S]{0,180}?\.show\(\)/);
  assert.match(
    MAIN_SOURCE,
    /handleDocumentChanged[\s\S]*?isMainFrame\s*&&\s*!isInPlace[\s\S]*?\.documentChanged\(\)/
  );
  assert.match(MAIN_SOURCE, /\.on\('did-start-navigation',\s*handleDocumentChanged\)/);
  assert.match(
    MAIN_SOURCE,
    /handleDocumentReady[\s\S]*?isMainFrame[\s\S]*?\.documentReady\(\)/
  );
  assert.match(MAIN_SOURCE, /\.on\('did-frame-finish-load',\s*handleDocumentReady\)/);
  assert.match(MAIN_SOURCE, /handlePreloadFailure[\s\S]*?\.preloadFailed\(\)/);
  assert.match(MAIN_SOURCE, /\.on\('preload-error',\s*handlePreloadFailure\)/);
  assert.match(MAIN_SOURCE, /focusExistingWindow\([\s\S]*?\.focusRequested\(\)/);
});

test('main preserves a closed nonzero presentation-fatal exit through cleanup', () => {
  assert.match(
    MAIN_SOURCE,
    /let applicationExitCode = 0;[\s\S]*?function requestApplicationExit\(exitCode:[\s\S]*?Math\.max\(applicationExitCode, exitCode\)[\s\S]*?app\.quit\(\)/
  );
  assert.match(
    MAIN_SOURCE,
    /async function createWindow\(\): Promise<'shown' \| 'fatal' \| 'closed'>[\s\S]*?return await presentationTerminal;/
  );
  assert.match(
    MAIN_SOURCE,
    /createdWindow\.on\('closed',[\s\S]*?settlePresentationTerminal\('closed'\)/
  );
  assert.match(
    MAIN_SOURCE,
    /const presentationStatus = await createWindow\(\);[\s\S]*?presentationStatus !== 'shown'[\s\S]*?return;/
  );
  assert.doesNotMatch(
    MAIN_SOURCE,
    /presentationOwner\.loadFailed\(\);\s*throw new Error/
  );
  assert.match(
    MAIN_SOURCE,
    /quitApplication:\s*\(\)\s*=>\s*\{[\s\S]*?requestApplicationExit\(1\)/
  );
  assert.match(
    MAIN_SOURCE,
    /app\.on\('before-quit',[\s\S]*?void cleanup\(\)\.then\([\s\S]*?app\.exit\(applicationExitCode\)[\s\S]*?Cleanup failed\.[\s\S]*?Math\.max\(applicationExitCode, 1\)[\s\S]*?app\.exit\(applicationExitCode\)/
  );
  assert.doesNotMatch(MAIN_SOURCE, /app\.exit\(0\)/);
});

test('deferred preload drift exceptions still describe live drift', () => {
  const registeredMethods = new Set(RPC_METHOD_REGISTRY.methods);
  const preloadMethods = new Set(preloadRpcMethodNames());

  for (const methodName of DEFERRED_UNREGISTERED_PRELOAD_METHODS) {
    assert.ok(preloadMethods.has(methodName), `${methodName} is no longer forwarded by preload`);
    assert.ok(!registeredMethods.has(methodName), `${methodName} is now registered`);
  }
});
