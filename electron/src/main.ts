/**
 * Electron Main Process
 *
 * Entry point for the Electron application.
 * Manages window lifecycle, Python sidecar, and IPC communication.
 */

import {
  app,
  BrowserWindow,
  ipcMain,
  dialog,
  shell,
  nativeTheme,
  type Event as ElectronEvent,
} from 'electron';
import * as path from 'path';
import * as fs from 'fs';
import { readLibraryDisplayScope } from './library-display-scope';
import { chooseModelImportPaths } from './model-import-picker';
import {
  persistLauncherRootOverride,
  resolveLauncherRoot,
} from './launcher-root';
import {
  createLauncherRootSelectionHandler,
  projectLauncherRootStartupState,
  type LauncherRootStartupState,
} from './launcher-root-recovery';
import {
  sanitizeOpenDialogOptions,
  validateApiCallPayload,
  validateExternalUrl,
} from './ipc-validation';
import { resolveBackendBinaryPath } from './backend-path';
import { PythonBridge } from './python-bridge';
import {
  LauncherRootRecoveryRequiredError,
  classifyBackendInitializationOutcome,
  observeBackendInitialization,
  projectBackendInitializationFailure,
} from './startup-task';
import {
  createWindowPresentationOwner,
  frameContainsWindowPresentationMarker,
  WINDOW_PRESENTATION_MARKER_CSS,
  type WindowPresentationOwner,
} from './window-presentation';
import log from 'electron-log';

// Configure logging
log.transports.file.level = 'info';
log.transports.console.level = 'debug';

// Window dimensions for the Electron desktop shell.
const WINDOW_WIDTH = 800;
const WINDOW_HEIGHT = 1000;
const MIN_WINDOW_WIDTH = 360;
const MIN_WINDOW_HEIGHT = 400;
const MODEL_LIBRARY_UPDATE_CHANNEL = 'model-library:update';
const MODEL_DOWNLOAD_UPDATE_CHANNEL = 'model-download:update';
const MODEL_DOWNLOAD_SUBSCRIBE_CHANNEL = 'model-download:subscribe';
const MODEL_DOWNLOAD_UNSUBSCRIBE_CHANNEL = 'model-download:unsubscribe';
const RUNTIME_PROFILE_UPDATE_CHANNEL = 'runtime-profile:update';
const RUNTIME_PROFILE_SUBSCRIBE_CHANNEL = 'runtime-profile:subscribe';
const RUNTIME_PROFILE_UNSUBSCRIBE_CHANNEL = 'runtime-profile:unsubscribe';
const SERVING_STATUS_UPDATE_CHANNEL = 'serving-status:update';
const SERVING_STATUS_ERROR_CHANNEL = 'serving-status:error';
const SERVING_STATUS_SUBSCRIBE_CHANNEL = 'serving-status:subscribe';
const SERVING_STATUS_UNSUBSCRIBE_CHANNEL = 'serving-status:unsubscribe';
const STATUS_TELEMETRY_UPDATE_CHANNEL = 'status-telemetry:update';
const STATUS_TELEMETRY_SUBSCRIBE_CHANNEL = 'status-telemetry:subscribe';
const STATUS_TELEMETRY_UNSUBSCRIBE_CHANNEL = 'status-telemetry:unsubscribe';
const LAUNCHER_ROOT_PRESENTATION_COMMITTED_CHANNEL =
  'launcher-root:presentation-committed';
const LAUNCHER_ROOT_PRESENTATION_TIMEOUT_CHANNEL =
  'launcher-root:presentation-timeout';
const WINDOW_PRESENTATION_DEADLINE_MS = 30_000;
const WINDOW_PRESENTATION_FALLBACK_GRACE_MS = 2_000;

// Python sidecar bridge
let pythonBridge: PythonBridge | null = null;
let mainWindow: BrowserWindow | null = null;
let windowPresentationOwner: WindowPresentationOwner | null = null;
let backendInitializationPromise: Promise<void> | null = null;
let launcherRootStartupState: LauncherRootStartupState = { status: 'initializing' };
const selectLauncherRoot = createLauncherRootSelectionHandler({
  chooseLibraryRoot: async () => {
    const targetWindow = mainWindow;
    if (!targetWindow || targetWindow.isDestroyed()) {
      return { status: 'unavailable' };
    }

    const result = await dialog.showOpenDialog(targetWindow, {
      title: 'Select Existing Pumas Library',
      buttonLabel: 'Use This Library',
      properties: ['openDirectory'],
      message: 'Choose a launcher root, shared-resources directory, or shared-resources/models directory.',
    });
    if (result.canceled || result.filePaths.length === 0) {
      return { status: 'cancelled' };
    }
    return { status: 'selected', selectedPath: result.filePaths[0]! };
  },
  persistLauncherRoot: (selectedPath) => {
    persistLauncherRootOverride(app.getPath('userData'), selectedPath);
  },
  requestRestart: () => {
    app.relaunch();
    setTimeout(() => {
      app.quit();
    }, 100);
  },
});
let modelDownloadRendererSubscriptions = 0;
let runtimeProfileRendererSubscriptions = 0;
let servingStatusRendererSubscriptions = 0;
let statusTelemetryRendererSubscriptions = 0;
let applicationExitCode = 0;
let applicationCleanupStarted = false;

function requestApplicationExit(exitCode: number): void {
  applicationExitCode = Math.max(applicationExitCode, exitCode);
  process.exitCode = applicationExitCode;
  app.quit();
}

function logBackendInitializationFailure(message: string, error: unknown): void {
  const diagnostic = projectBackendInitializationFailure(message, error);

  if ('error' in diagnostic) {
    log.error(diagnostic.message, diagnostic.error);
  } else {
    log.error(diagnostic.message);
  }
}

function focusExistingWindow(): void {
  if (!mainWindow || mainWindow.isDestroyed()) {
    return;
  }

  windowPresentationOwner?.focusRequested();
}

function isReleaseSmokeMode(): boolean {
  return process.env.PUMAS_RELEASE_SMOKE === '1';
}

function getReleaseSmokeExitDelayMs(): number {
  const parsed = Number.parseInt(process.env.PUMAS_RELEASE_SMOKE_EXIT_MS ?? '', 10);

  if (Number.isNaN(parsed) || parsed <= 0) {
    return 1_500;
  }

  return parsed;
}

/**
 * Configure Wayland/GTK4 support for Linux
 * Must be called before app.whenReady()
 */
function configureLinuxDisplay(): void {
  if (process.platform !== 'linux') return;

  // Detect display server from environment
  const sessionType = process.env.XDG_SESSION_TYPE;

  if (sessionType === 'wayland') {
    // Enable native Wayland support
    app.commandLine.appendSwitch('ozone-platform', 'wayland');
    app.commandLine.appendSwitch('enable-features', 'WaylandWindowDecorations');
    log.info('Linux display configured: Wayland native');
  } else {
    // X11 or unknown - use X11 backend
    app.commandLine.appendSwitch('ozone-platform', 'x11');
    log.info('Linux display configured: X11');
  }

  // Use GTK4 for theming (better modern desktop support)
  app.commandLine.appendSwitch('gtk-version', '4');
}

/**
 * Get the path to the frontend content
 */
function getFrontendPath(): string {
  const isDev = process.argv.includes('--dev');

  if (isDev) {
    // Development mode: Vite dev server
    return 'http://127.0.0.1:3000';
  }

  // Production mode: bundled frontend
  if (app.isPackaged) {
    // Packaged app: resources directory
    return path.join(process.resourcesPath, 'frontend', 'index.html');
  }

  // Development build: local dist
  return path.join(__dirname, '..', '..', 'frontend', 'dist', 'index.html');
}

/**
 * Resolve the runtime icon path for window/taskbar/dock usage.
 */
function getRuntimeIconPath(): string | undefined {
  const iconFile = process.platform === 'win32' ? 'icon.ico' : 'Pumas-Library_05.png';
  const iconPath = app.isPackaged
    ? path.join(process.resourcesPath, 'icons', iconFile)
    : path.join(__dirname, '..', 'build', 'icons', iconFile);

  if (fs.existsSync(iconPath)) {
    return iconPath;
  }

  log.warn(`Runtime icon not found at ${iconPath}`);
  return undefined;
}

/**
 * Create the main application window
 */
async function createWindow(): Promise<'shown' | 'fatal' | 'closed'> {
  log.info('Creating main window...');
  const windowIconPath = getRuntimeIconPath();

  const createdWindow = new BrowserWindow({
    width: WINDOW_WIDTH,
    height: WINDOW_HEIGHT,
    minWidth: MIN_WINDOW_WIDTH,
    minHeight: MIN_WINDOW_HEIGHT,
    resizable: true,
    frame: false, // Frameless window (custom title bar)
    backgroundColor: '#000000',
    icon: windowIconPath,
    show: false, // Don't show until ready
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
    },
  });
  mainWindow = createdWindow;

  let presentationTerminalSettled = false;
  let settlePresentationTerminal: (
    status: 'shown' | 'fatal' | 'closed'
  ) => void = () => {};
  const presentationTerminal = new Promise<'shown' | 'fatal' | 'closed'>((resolve) => {
    settlePresentationTerminal = (status) => {
      if (presentationTerminalSettled) {
        return;
      }
      presentationTerminalSettled = true;
      resolve(status);
    };
  });
  const presentationOwner = createWindowPresentationOwner({
    getAuthoritativeStatus: () => launcherRootStartupState.status,
    schedule: (callback, delayMs) => setTimeout(callback, delayMs),
    clearScheduled: (handle) => {
      clearTimeout(handle as ReturnType<typeof setTimeout>);
    },
    subscribeToPresentationFrames: (callback) => {
      if (createdWindow.webContents.isDestroyed()) {
        throw new Error('Application window is unavailable.');
      }
      createdWindow.webContents.beginFrameSubscription(false, callback);
    },
    unsubscribeFromPresentationFrames: () => {
      if (!createdWindow.webContents.isDestroyed()) {
        createdWindow.webContents.endFrameSubscription();
      }
    },
    insertPresentationMarker: (onInserted, onUnavailable) => {
      if (createdWindow.webContents.isDestroyed()) {
        onUnavailable();
        return;
      }
      void createdWindow.webContents.insertCSS(WINDOW_PRESENTATION_MARKER_CSS).then(
        (markerHandle) => {
          if (typeof markerHandle !== 'string' || markerHandle.length === 0) {
            onUnavailable();
            return;
          }
          onInserted(markerHandle);
        },
        onUnavailable
      );
    },
    removePresentationMarker: (markerHandle, onRemoved, onUnavailable) => {
      if (
        typeof markerHandle !== 'string' ||
        createdWindow.webContents.isDestroyed()
      ) {
        onUnavailable();
        return;
      }
      void createdWindow.webContents.removeInsertedCSS(markerHandle).then(
        onRemoved,
        onUnavailable
      );
    },
    invalidatePresentationFrame: () => {
      if (createdWindow.webContents.isDestroyed()) {
        throw new Error('Application window is unavailable.');
      }
      createdWindow.webContents.invalidate();
    },
    frameContainsPresentationMarker: frameContainsWindowPresentationMarker,
    showWindow: () => {
      if (createdWindow.isDestroyed()) {
        throw new Error('Application window is unavailable.');
      }
      createdWindow.show();
      settlePresentationTerminal('shown');
      log.info('Window shown after committed launcher-root presentation');
    },
    focusWindow: () => {
      if (createdWindow.isDestroyed()) {
        throw new Error('Application window is unavailable.');
      }
      if (createdWindow.isMinimized()) {
        createdWindow.restore();
      }
      createdWindow.focus();
    },
    reportFocusUnavailable: () => {
      log.warn('Application window focus request was unavailable.');
    },
    sendVisibilityTimeout: () => {
      if (createdWindow.isDestroyed()) {
        throw new Error('Application window is unavailable.');
      }
      createdWindow.webContents.send(LAUNCHER_ROOT_PRESENTATION_TIMEOUT_CHANNEL);
    },
    showNativeFatal: () => {
      settlePresentationTerminal('fatal');
      dialog.showErrorBox(
        'Pumas Library could not start',
        'The application window could not be prepared safely.'
      );
    },
    destroyWindow: () => {
      if (!createdWindow.isDestroyed()) {
        createdWindow.destroy();
      }
    },
    quitApplication: () => {
      requestApplicationExit(1);
    },
  }, {
    presentationDeadlineMs: WINDOW_PRESENTATION_DEADLINE_MS,
    fallbackGraceMs: WINDOW_PRESENTATION_FALLBACK_GRACE_MS,
  });
  windowPresentationOwner = presentationOwner;

  // Browser readiness alone cannot reveal an unverified launcher-root presentation.
  const handleBrowserReady = () => {
    presentationOwner.browserReady();
  };
  const handleDocumentChanged = (
    _event: ElectronEvent,
    _url: string,
    isInPlace: boolean,
    isMainFrame: boolean
  ) => {
    if (isMainFrame && !isInPlace) {
      presentationOwner.documentChanged();
    }
  };
  const handleDocumentReady = (_event: ElectronEvent, isMainFrame: boolean) => {
    if (isMainFrame) {
      presentationOwner.documentReady();
    }
  };
  const handlePreloadFailure = () => {
    log.error('Application preload was unavailable.');
    presentationOwner.preloadFailed();
  };
  createdWindow.on('ready-to-show', handleBrowserReady);
  createdWindow.webContents.on('did-start-navigation', handleDocumentChanged);
  createdWindow.webContents.on('did-frame-finish-load', handleDocumentReady);
  createdWindow.webContents.on('preload-error', handlePreloadFailure);

  // Load frontend content
  const frontendPath = getFrontendPath();
  const isDev = frontendPath.startsWith('http');
  const wantsDevTools = process.argv.includes('--dev') || process.argv.includes('--debug');

  if (isDev) {
    log.info(`Loading development server: ${frontendPath}`);
    try {
      await createdWindow.loadURL(frontendPath);
    } catch {
      // Dev server not running, fall back to production build
      log.warn('Dev server not available, falling back to production build');
      const prodPath = app.isPackaged
        ? path.join(process.resourcesPath, 'frontend', 'index.html')
        : path.join(__dirname, '..', '..', 'frontend', 'dist', 'index.html');
      log.info(`Loading production build: ${prodPath}`);
      try {
        await createdWindow.loadFile(prodPath);
      } catch {
        presentationOwner.loadFailed();
        return await presentationTerminal;
      }
    }
  } else {
    log.info(`Loading production build: ${frontendPath}`);
    try {
      await createdWindow.loadFile(frontendPath);
    } catch {
      presentationOwner.loadFailed();
      return await presentationTerminal;
    }
  }

  // Open DevTools in development mode
  if (wantsDevTools) {
    createdWindow.webContents.openDevTools({ mode: 'detach' });
  }

  // Handle window closed
  createdWindow.on('closed', () => {
    presentationOwner.dispose();
    settlePresentationTerminal('closed');
    createdWindow.removeListener('ready-to-show', handleBrowserReady);
    if (!createdWindow.isDestroyed() && !createdWindow.webContents.isDestroyed()) {
      createdWindow.webContents.removeListener(
        'did-start-navigation',
        handleDocumentChanged
      );
      createdWindow.webContents.removeListener(
        'did-frame-finish-load',
        handleDocumentReady
      );
      createdWindow.webContents.removeListener('preload-error', handlePreloadFailure);
    }
    if (windowPresentationOwner === presentationOwner) {
      windowPresentationOwner = null;
    }
    if (mainWindow !== createdWindow) {
      return;
    }
    servingStatusRendererSubscriptions = 0;
    runtimeProfileRendererSubscriptions = 0;
    statusTelemetryRendererSubscriptions = 0;
    pythonBridge?.stopServingStatusUpdateStream();
    pythonBridge?.stopRuntimeProfileUpdateStream();
    pythonBridge?.stopStatusTelemetryUpdateStream();
    mainWindow = null;
  });

  const presentationStatus = await presentationTerminal;
  if (presentationStatus === 'shown') {
    log.info('Main window created');
  }
  return presentationStatus;
}

/**
 * Register IPC handlers for Python API bridge
 */
function registerIPCHandlers(): void {
  log.info('Registering IPC handlers...');

  // Generic API call handler - forwards validated renderer requests to the backend sidecar.
  ipcMain.handle('api:call', async (_event, method: unknown, params: unknown) => {
    const request = validateApiCallPayload(method, params);

    if (backendInitializationPromise) {
      await backendInitializationPromise;
    }

    if (!pythonBridge) {
      throw new Error('Python bridge not initialized');
    }
    return await pythonBridge.call(request.method, request.params);
  });

  // Window control handlers
  ipcMain.handle('window:close', () => {
    mainWindow?.close();
  });

  ipcMain.handle('window:minimize', () => {
    mainWindow?.minimize();
  });

  ipcMain.handle('window:maximize', () => {
    if (mainWindow?.isMaximized()) {
      mainWindow.unmaximize();
    } else {
      mainWindow?.maximize();
    }
  });

  // File dialog handler
  ipcMain.handle('dialog:openFile', async (_event, options: unknown) => {
    const targetWindow = mainWindow;
    if (!targetWindow || targetWindow.isDestroyed()) return { status: 'unavailable' };
    return chooseModelImportPaths(() => dialog.showOpenDialog(targetWindow, sanitizeOpenDialogOptions(options)));
  });

  ipcMain.handle('launcher:getRootState', () => launcherRootStartupState);
  // A snapshot only: never wait for the backend or perform IO in synchronous IPC.
  ipcMain.on('launcher:getRootBootstrap', (event) => {
    const window = mainWindow;
    event.returnValue = window && !window.isDestroyed() &&
      event.sender === window.webContents &&
      event.senderFrame === window.webContents.mainFrame
      ? launcherRootStartupState
      : null;
  });

  ipcMain.handle(
    LAUNCHER_ROOT_PRESENTATION_COMMITTED_CHANNEL,
    (event, presentation: unknown) => {
      const targetWindow = mainWindow;
      const targetOwner = windowPresentationOwner;
      if (!targetWindow || targetWindow.isDestroyed() || !targetOwner) {
        return;
      }
      const outcome = targetOwner.rendererCommitted({
        currentTopDocument:
          event.sender === targetWindow.webContents &&
          event.senderFrame === targetWindow.webContents.mainFrame,
        presentation,
      });
      if (outcome.status === 'invalid') {
        throw new Error('Invalid launcher-root presentation state.');
      }
    }
  );

  ipcMain.handle('launcher:chooseLibraryRoot', async () => {
    const result = await selectLauncherRoot(launcherRootStartupState);
    if (result.status === 'restarting') {
      log.info('Persisted launcher root override');
    } else if (result.status === 'recovery-required') {
      log.error(
        `Launcher root selection requires recovery: ${result.reason} (${result.authorityState}).`
      );
    }
    return result;
  });

  // Shell handlers
  ipcMain.handle('shell:openExternal', async (_event, url: unknown) => {
    await shell.openExternal(validateExternalUrl(url));
  });

  // Theme handler
  ipcMain.handle('theme:get', () => {
    return nativeTheme.shouldUseDarkColors ? 'dark' : 'light';
  });

  ipcMain.handle(STATUS_TELEMETRY_SUBSCRIBE_CHANNEL, () => {
    statusTelemetryRendererSubscriptions += 1;
    if (statusTelemetryRendererSubscriptions === 1) {
      startStatusTelemetryUpdateForwarder();
    }
  });

  ipcMain.handle(STATUS_TELEMETRY_UNSUBSCRIBE_CHANNEL, () => {
    statusTelemetryRendererSubscriptions = Math.max(0, statusTelemetryRendererSubscriptions - 1);
    if (statusTelemetryRendererSubscriptions === 0) {
      pythonBridge?.stopStatusTelemetryUpdateStream();
    }
  });

  ipcMain.handle(MODEL_DOWNLOAD_SUBSCRIBE_CHANNEL, () => {
    modelDownloadRendererSubscriptions += 1;
    if (modelDownloadRendererSubscriptions === 1) {
      startModelDownloadUpdateForwarder();
    }
  });

  ipcMain.handle(MODEL_DOWNLOAD_UNSUBSCRIBE_CHANNEL, () => {
    modelDownloadRendererSubscriptions = Math.max(0, modelDownloadRendererSubscriptions - 1);
    if (modelDownloadRendererSubscriptions === 0) {
      pythonBridge?.stopModelDownloadUpdateStream();
    }
  });

  ipcMain.handle(RUNTIME_PROFILE_SUBSCRIBE_CHANNEL, () => {
    runtimeProfileRendererSubscriptions += 1;
    if (runtimeProfileRendererSubscriptions === 1) {
      startRuntimeProfileUpdateForwarder();
    }
  });

  ipcMain.handle(RUNTIME_PROFILE_UNSUBSCRIBE_CHANNEL, () => {
    runtimeProfileRendererSubscriptions = Math.max(0, runtimeProfileRendererSubscriptions - 1);
    if (runtimeProfileRendererSubscriptions === 0) {
      pythonBridge?.stopRuntimeProfileUpdateStream();
    }
  });

  ipcMain.handle(SERVING_STATUS_SUBSCRIBE_CHANNEL, () => {
    servingStatusRendererSubscriptions += 1;
    if (servingStatusRendererSubscriptions === 1) {
      try {
        startServingStatusUpdateForwarder();
      } catch (error) {
        servingStatusRendererSubscriptions = Math.max(0, servingStatusRendererSubscriptions - 1);
        throw error;
      }
    }
  });

  ipcMain.handle(SERVING_STATUS_UNSUBSCRIBE_CHANNEL, () => {
    servingStatusRendererSubscriptions = Math.max(0, servingStatusRendererSubscriptions - 1);
    if (servingStatusRendererSubscriptions === 0) {
      pythonBridge?.stopServingStatusUpdateStream();
    }
  });

  log.info('IPC handlers registered');
}

function startModelLibraryUpdateForwarder(): void {
  if (!pythonBridge?.isRunning()) {
    return;
  }

  pythonBridge.startModelLibraryUpdateStream((payload) => {
    const targetWindow = mainWindow;
    if (!targetWindow || targetWindow.isDestroyed()) {
      return;
    }

    targetWindow.webContents.send(MODEL_LIBRARY_UPDATE_CHANNEL, payload);
  });
}

function startRuntimeProfileUpdateForwarder(): void {
  if (!pythonBridge?.isRunning() || runtimeProfileRendererSubscriptions === 0) {
    return;
  }

  pythonBridge.startRuntimeProfileUpdateStream((payload) => {
    const targetWindow = mainWindow;
    if (!targetWindow || targetWindow.isDestroyed()) {
      return;
    }

    targetWindow.webContents.send(RUNTIME_PROFILE_UPDATE_CHANNEL, payload);
  });
}

function startModelDownloadUpdateForwarder(): void {
  if (!pythonBridge?.isRunning() || modelDownloadRendererSubscriptions === 0) {
    return;
  }

  pythonBridge.startModelDownloadUpdateStream((payload) => {
    const targetWindow = mainWindow;
    if (!targetWindow || targetWindow.isDestroyed()) {
      return;
    }

    targetWindow.webContents.send(MODEL_DOWNLOAD_UPDATE_CHANNEL, payload);
  });
}

function startStatusTelemetryUpdateForwarder(): void {
  if (!pythonBridge?.isRunning() || statusTelemetryRendererSubscriptions === 0) {
    return;
  }

  pythonBridge.startStatusTelemetryUpdateStream((payload) => {
    const targetWindow = mainWindow;
    if (!targetWindow || targetWindow.isDestroyed()) {
      return;
    }

    targetWindow.webContents.send(STATUS_TELEMETRY_UPDATE_CHANNEL, payload);
  });
}

function startServingStatusUpdateForwarder(): void {
  if (!pythonBridge?.isRunning() || servingStatusRendererSubscriptions === 0) {
    return;
  }

  pythonBridge.startServingStatusUpdateStream(
    (payload) => {
      const targetWindow = mainWindow;
      if (!targetWindow || targetWindow.isDestroyed()) {
        return;
      }

      targetWindow.webContents.send(SERVING_STATUS_UPDATE_CHANNEL, payload);
    },
    (message) => {
      const targetWindow = mainWindow;
      if (!targetWindow || targetWindow.isDestroyed()) {
        return;
      }

      targetWindow.webContents.send(SERVING_STATUS_ERROR_CHANNEL, { message });
    }
  );
}

/**
 * Initialize the Rust backend sidecar process
 */
async function initializeBackend(): Promise<void> {
  if (backendInitializationPromise) {
    await backendInitializationPromise;
    return;
  }

  backendInitializationPromise = (async () => {
    log.info('Initializing backend bridge...');

    const rustBinaryPath = resolveBackendBinaryPath({
      defaultBuildProfile: process.argv.includes('--dev') ? 'debug' : 'release',
      isPackaged: app.isPackaged,
      overridePath: process.env.PUMAS_RPC_BINARY,
      platform: process.platform,
      resourcesPath: process.resourcesPath,
      sourceRoot: path.join(__dirname, '..', '..'),
    });

    const launcherRootResolution = resolveLauncherRoot({
      appImagePath: process.env.APPIMAGE,
      devRoot: path.join(__dirname, '..', '..'),
      execPath: process.execPath,
      isPackaged: app.isPackaged,
      userDataPath: app.getPath('userData'),
    });
    launcherRootStartupState = projectLauncherRootStartupState(
      launcherRootResolution,
      launcherRootResolution.status === 'resolved'
        ? readLibraryDisplayScope(launcherRootResolution.launcherRoot)
        : null
    );
    if (launcherRootResolution.status === 'recovery-required') {
      throw new LauncherRootRecoveryRequiredError(launcherRootResolution);
    }
    const launcherRoot = launcherRootResolution.launcherRoot;
    log.info(`Resolved launcher root from ${launcherRootResolution.source}`);

    pythonBridge = new PythonBridge({
      port: 0,
      debug: process.argv.includes('--dev') || process.argv.includes('--debug'),
      rustBinaryPath,
      launcherRoot,
    });

    await pythonBridge.start();
    startModelLibraryUpdateForwarder();
    startModelDownloadUpdateForwarder();
    startRuntimeProfileUpdateForwarder();
    startServingStatusUpdateForwarder();
    startStatusTelemetryUpdateForwarder();
    log.info('Backend bridge initialized');
  })();

  try {
    await backendInitializationPromise;
  } catch (error) {
    const failedBridge = pythonBridge;
    if (failedBridge) {
      try {
        await failedBridge.stop();
        if (pythonBridge === failedBridge) {
          pythonBridge = null;
        }
      } catch {
        // Keep the owner available for application cleanup to retry shutdown.
        log.error('Failed to stop backend bridge after initialization failure.');
      }
    }
    throw error;
  } finally {
    backendInitializationPromise = null;
  }
}

/**
 * Clean up resources before quitting
 */
async function cleanup(): Promise<void> {
  log.info('Cleaning up...');

  if (pythonBridge) {
    await pythonBridge.stop();
    pythonBridge = null;
  }

  log.info('Cleanup complete');
}

// Configure Linux display before app is ready
configureLinuxDisplay();

const hasSingleInstanceLock = app.requestSingleInstanceLock();

if (!hasSingleInstanceLock) {
  log.info('Another Pumas Library instance is already running; exiting duplicate launch.');
  app.quit();
} else {
  app.on('second-instance', () => {
    focusExistingWindow();
  });

  // App lifecycle handlers
  void app.whenReady().then(async () => {
    log.info('App ready');
    const runtimeIconPath = getRuntimeIconPath();
    const releaseSmokeMode = isReleaseSmokeMode();

    if (process.platform === 'darwin' && runtimeIconPath && app.dock) {
      app.dock.setIcon(runtimeIconPath);
    }

    try {
      // Register IPC handlers
      registerIPCHandlers();

      // Attach rejection handling before window creation can delay consumption.
      const backendInitialization = observeBackendInitialization(initializeBackend());

      if (!releaseSmokeMode) {
        void backendInitialization.then((outcome) => {
          const disposition = classifyBackendInitializationOutcome(outcome, 'desktop');
          if (disposition.status === 'ready') {
            return;
          }

          if (disposition.status === 'recovery-required') {
            log.error('Launcher root recovery is required before backend startup.');
            return;
          }

          logBackendInitializationFailure(
            'Failed to initialize backend bridge',
            disposition.error
          );
          requestApplicationExit(1);
        });
      }

      // Backend warmup and the hidden window presentation proceed in parallel.
      const presentationStatus = await createWindow();
      if (presentationStatus !== 'shown') {
        return;
      }

      if (releaseSmokeMode) {
        const outcome = await backendInitialization;
        const disposition = classifyBackendInitializationOutcome(
          outcome,
          'release-smoke'
        );
        if (disposition.status === 'fatal') {
          throw disposition.error;
        }

        const exitDelayMs = getReleaseSmokeExitDelayMs();
        log.info(`Release smoke startup succeeded; exiting in ${exitDelayMs}ms`);
        setTimeout(() => {
          app.quit();
        }, exitDelayMs);
        return;
      }

    } catch (error) {
      logBackendInitializationFailure('Failed to initialize app', error);
      requestApplicationExit(1);
    }
  });
}

app.on('window-all-closed', () => {
  // On macOS, apps typically stay open until explicitly quit
  // On Linux/Windows, quit when all windows are closed
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('activate', async () => {
  // On macOS, recreate window when dock icon is clicked
  if (BrowserWindow.getAllWindows().length === 0) {
    await createWindow();
  }
});

app.on('before-quit', (event) => {
  event.preventDefault();
  if (applicationCleanupStarted) {
    return;
  }
  applicationCleanupStarted = true;
  void cleanup().then(
    () => {
      app.exit(applicationExitCode);
    },
    () => {
      log.error('Cleanup failed.');
      applicationExitCode = Math.max(applicationExitCode, 1);
      process.exitCode = applicationExitCode;
      app.exit(applicationExitCode);
    }
  );
});

// Handle uncaught exceptions
process.on('uncaughtException', (error) => {
  log.error('Uncaught exception:', error);
});

process.on('unhandledRejection', (reason) => {
  log.error('Unhandled rejection:', reason);
});
