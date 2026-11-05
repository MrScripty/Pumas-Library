/**
 * Window, navigation and permission boundary for every renderer.
 *
 * The preload exposes the full desktop bridge to any document it runs in, so
 * no renderer may open a child window, navigate away from the bundled app, or
 * receive browser permissions. External links leave through the system
 * browser after the same URL validation as `shell:openExternal`.
 */

import { pathToFileURL } from 'url';
import { validateExternalUrl } from './ipc-validation';

/** The documents that belong to this application. */
export interface AppOriginPolicy {
  isAppUrl(url: unknown): boolean;
}

export interface AppOriginEntries {
  /** Absolute filesystem path of the bundled frontend entry document. */
  frontendEntryPath: string;
  /** Development server origin, only when the build is not packaged. */
  devServerOrigin?: string | null;
  /** Windows paths compare case-insensitively. */
  caseInsensitivePaths?: boolean;
}

function comparablePathname(pathname: string, caseInsensitive: boolean): string | null {
  let decoded: string;
  try {
    decoded = decodeURIComponent(pathname);
  } catch {
    return null;
  }
  return caseInsensitive ? decoded.toLowerCase() : decoded;
}

export function createAppOriginPolicy(entries: AppOriginEntries): AppOriginPolicy {
  const caseInsensitive = entries.caseInsensitivePaths === true;
  const entryPathname = comparablePathname(pathToFileURL(entries.frontendEntryPath).pathname, caseInsensitive);
  const devOrigin = entries.devServerOrigin ? new URL(entries.devServerOrigin).origin : null;
  return {
    isAppUrl(url: unknown): boolean {
      if (typeof url !== 'string' || url.length === 0) return false;
      let parsed: URL;
      try {
        parsed = new URL(url);
      } catch {
        return false;
      }
      if (parsed.protocol === 'file:') {
        // Only the bundled entry document; never another local file.
        return parsed.host === '' && entryPathname !== null
          && comparablePathname(parsed.pathname, caseInsensitive) === entryPathname;
      }
      return devOrigin !== null
        && (parsed.protocol === 'http:' || parsed.protocol === 'https:')
        && parsed.origin === devOrigin;
    },
  };
}

/**
 * Only clipboard writes (used by the copy buttons) are granted, and only to an
 * application document. Every other browser permission is denied.
 */
const GRANTED_APP_PERMISSIONS: ReadonlySet<string> = new Set(['clipboard-sanitized-write']);

export interface GuardLogger {
  warn(message: string): void;
}

export interface GuardedSession {
  setPermissionRequestHandler(
    handler: (
      webContents: unknown,
      permission: string,
      callback: (granted: boolean) => void,
      details: { requestingUrl?: string; isMainFrame?: boolean }
    ) => void
  ): void;
  setPermissionCheckHandler(
    handler: (
      webContents: unknown,
      permission: string,
      requestingOrigin: string,
      details: { requestingUrl?: string; isMainFrame?: boolean }
    ) => boolean
  ): void;
  setDevicePermissionHandler?(handler: () => boolean): void;
}

export interface GuardedWebContents {
  setWindowOpenHandler(handler: (details: { url: string }) => { action: 'deny' }): void;
  on(event: string, listener: (...args: never[]) => void): unknown;
  session?: GuardedSession | null;
}

export interface WebContentsGuardOptions {
  policy: AppOriginPolicy;
  openExternal(url: string): Promise<void>;
  log: GuardLogger;
}

/** Why a URL never reaches the system browser, without echoing the URL. */
function externalRefusalReason(error: unknown): string {
  return error instanceof Error ? error.message : 'Invalid URL';
}

/**
 * Hand a renderer-requested URL to the system browser after validation.
 * Returns false (with a logged reason) when the URL is refused.
 */
export function openValidatedExternal(
  rawUrl: unknown,
  options: Pick<WebContentsGuardOptions, 'openExternal' | 'log'>
): boolean {
  let url: string;
  try {
    url = validateExternalUrl(rawUrl);
  } catch (error) {
    options.log.warn(`Refused external URL: ${externalRefusalReason(error)}`);
    return false;
  }
  void options.openExternal(url).catch((error: unknown) => {
    options.log.warn(`System browser could not open URL: ${externalRefusalReason(error)}`);
  });
  return true;
}

const guardedSessions = new WeakSet<object>();

export function installSessionGuards(session: GuardedSession, policy: AppOriginPolicy, log: GuardLogger): void {
  if (guardedSessions.has(session)) return;
  guardedSessions.add(session);
  const granted = (permission: string, details: { requestingUrl?: string; isMainFrame?: boolean }) =>
    GRANTED_APP_PERMISSIONS.has(permission)
    && details.isMainFrame !== false
    && policy.isAppUrl(details.requestingUrl);
  session.setPermissionRequestHandler((_contents, permission, callback, details) => {
    const allow = granted(permission, details ?? {});
    if (!allow) log.warn(`Denied renderer permission request: ${permission}`);
    callback(allow);
  });
  session.setPermissionCheckHandler((_contents, permission, _origin, details) =>
    granted(permission, details ?? {}));
  session.setDevicePermissionHandler?.(() => false);
}

const guardedContents = new WeakSet<object>();

/**
 * Deny child windows, navigation and redirects off the application, webview
 * attachment, and browser permissions for one renderer. Idempotent.
 */
export function installWebContentsGuards(
  contents: GuardedWebContents,
  options: WebContentsGuardOptions
): void {
  if (guardedContents.has(contents)) return;
  guardedContents.add(contents);
  const { policy, log } = options;

  contents.setWindowOpenHandler(({ url }) => {
    if (!policy.isAppUrl(url)) openValidatedExternal(url, options);
    else log.warn('Denied application child window');
    return { action: 'deny' };
  });

  const denyOffAppNavigation = (kind: string) => (event: { preventDefault(): void }, url: unknown) => {
    if (policy.isAppUrl(url)) return;
    event.preventDefault();
    log.warn(`Denied renderer ${kind} away from the application`);
  };
  contents.on('will-navigate', denyOffAppNavigation('navigation'));
  contents.on('will-redirect', denyOffAppNavigation('redirect'));
  contents.on('will-attach-webview', (event: { preventDefault(): void }) => {
    event.preventDefault();
    log.warn('Denied webview attachment');
  });

  if (contents.session) installSessionGuards(contents.session, policy, log);
}

export interface IpcSenderEvent {
  sender: unknown;
  senderFrame: { url?: string } | null;
}

export interface MainFrameOwner {
  isDestroyed(): boolean;
  webContents: { mainFrame: unknown };
}

/** True only for the current application document in the main window's top frame. */
export function isMainFrameSender(
  event: IpcSenderEvent,
  window: MainFrameOwner | null,
  policy: AppOriginPolicy
): boolean {
  if (!window || window.isDestroyed()) return false;
  const contents = window.webContents;
  return event.sender === contents
    && event.senderFrame !== null
    && event.senderFrame === contents.mainFrame
    && policy.isAppUrl(event.senderFrame.url);
}

export class IpcSenderRefusedError extends Error {
  constructor(channel: string) {
    super(`IPC ${channel} refused: sender is not the application window.`);
    this.name = 'IpcSenderRefusedError';
  }
}
