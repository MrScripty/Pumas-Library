/**
 * Electron Main Process
 *
 * Entry point for the Electron application.
 * Manages window lifecycle, Python sidecar, and IPC communication.
 */

import { app, BrowserWindow, ipcMain, dialog, shell, nativeTheme } from 'electron';
import * as path from 'path';
import { PythonBridge } from './python-bridge';
import log from 'electron-log';

// Configure logging
log.transports.file.level = 'info';
log.transports.console.level = 'debug';

// Window dimensions (matching PyWebView config from backend/config.py)
const WINDOW_WIDTH = 400;
const WINDOW_HEIGHT = 520;
const MIN_WINDOW_WIDTH = 360;
const MIN_WINDOW_HEIGHT = 400;

// Python sidecar bridge
let pythonBridge: PythonBridge | null = null;
let mainWindow: BrowserWindow | null = null;

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
 * Create the main application window
 */
async function createWindow(): Promise<void> {
  log.info('Creating main window...');

  mainWindow = new BrowserWindow({
    width: WINDOW_WIDTH,
    height: WINDOW_HEIGHT,
    minWidth: MIN_WINDOW_WIDTH,
    minHeight: MIN_WINDOW_HEIGHT,
    resizable: true,
    frame: false, // Frameless window (custom title bar)
    backgroundColor: '#000000',
    show: false, // Don't show until ready
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
    },
  });

  // Show window when ready - must be set up BEFORE loading content
  mainWindow.once('ready-to-show', () => {
    mainWindow?.show();
    log.info('Window shown');
  });

  // Load frontend content
  const frontendPath = getFrontendPath();
  const isDev = frontendPath.startsWith('http');
  const wantsDevTools = process.argv.includes('--dev') || process.argv.includes('--debug');

  if (isDev) {
    log.info(`Loading development server: ${frontendPath}`);
    try {
      await mainWindow.loadURL(frontendPath);
    } catch (error) {
      // Dev server not running, fall back to production build
      log.warn('Dev server not available, falling back to production build');
      const prodPath = app.isPackaged
        ? path.join(process.resourcesPath, 'frontend', 'index.html')
        : path.join(__dirname, '..', '..', 'frontend', 'dist', 'index.html');
      log.info(`Loading production build: ${prodPath}`);
      await mainWindow.loadFile(prodPath);
    }
  } else {
    log.info(`Loading production build: ${frontendPath}`);
    await mainWindow.loadFile(frontendPath);
  }

  // Open DevTools in development mode
  if (wantsDevTools) {
    mainWindow.webContents.openDevTools({ mode: 'detach' });
  }

  // Handle window closed
  mainWindow.on('closed', () => {
    mainWindow = null;
  });

  log.info('Main window created');
}

/**
 * Register IPC handlers for Python API bridge
 */
function registerIPCHandlers(): void {
  log.info('Registering IPC handlers...');

  // Generic API call handler - forwards to Python sidecar
  ipcMain.handle('api:call', async (_event, method: string, params: Record<string, unknown>) => {
    if (!pythonBridge) {
      throw new Error('Python bridge not initialized');
    }
    return await pythonBridge.call(method, params);
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
  ipcMain.handle('dialog:openFile', async (_event, options: Electron.OpenDialogOptions) => {
    if (!mainWindow) return { canceled: true, filePaths: [] };
    return await dialog.showOpenDialog(mainWindow, options);
  });

  // Shell handlers
  ipcMain.handle('shell:openExternal', async (_event, url: string) => {
    let parsedUrl: URL;
    try {
      parsedUrl = new URL(url);
    } catch {
      throw new Error('Invalid URL');
    }

    if (parsedUrl.protocol !== 'http:' && parsedUrl.protocol !== 'https:') {
      throw new Error('Only http/https URLs are allowed');
    }

    await shell.openExternal(parsedUrl.toString());
  });

  ipcMain.handle('shell:openPath', async (_event, filePath: string) => {
    await shell.openPath(filePath);
  });

  // Theme handler
  ipcMain.handle('theme:get', () => {
    return nativeTheme.shouldUseDarkColors ? 'dark' : 'light';
  });

  log.info('IPC handlers registered');
}

/**
 * Initialize the Rust backend sidecar process
 */
async function initializeBackend(): Promise<void> {
  log.info('Initializing backend bridge...');

  const binaryName = process.platform === 'win32' ? 'pumas-rpc.exe' : 'pumas-rpc';

  const rustBinaryPath = app.isPackaged
    ? path.join(process.resourcesPath, binaryName)
    : path.join(__dirname, '..', '..', 'rust', 'target', 'release', binaryName);

  // Data root: portable (next to AppImage) or project root in dev
  let launcherRoot: string;
  if (process.env.APPIMAGE) {
    // AppImage: store data next to the .AppImage file
    launcherRoot = path.join(path.dirname(process.env.APPIMAGE), 'pumas-data');
  } else if (app.isPackaged) {
    // Other packaged formats (.deb, etc.): use standard user data path
    launcherRoot = app.getPath('userData');
  } else {
    // Development mode: project root
    launcherRoot = path.join(__dirname, '..', '..');
  }

  pythonBridge = new PythonBridge({
    port: 0,
    debug: process.argv.includes('--dev') || process.argv.includes('--debug'),
    rustBinaryPath,
    launcherRoot,
  });

  await pythonBridge.start();
  log.info('Backend bridge initialized');
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

// App lifecycle handlers
app.whenReady().then(async () => {
  log.info('App ready');

  try {
    // Initialize Rust backend first
    await initializeBackend();

    // Register IPC handlers
    registerIPCHandlers();

    // Create the main window
    await createWindow();
  } catch (error) {
    log.error('Failed to initialize app:', error);
    app.quit();
  }
});

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

app.on('before-quit', async (event) => {
  event.preventDefault();
  await cleanup();
  app.exit(0);
});

// Handle uncaught exceptions
process.on('uncaughtException', (error) => {
  log.error('Uncaught exception:', error);
});

process.on('unhandledRejection', (reason) => {
  log.error('Unhandled rejection:', reason);
});
