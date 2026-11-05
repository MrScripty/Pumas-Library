/**
 * Dev-server-only Content-Security-Policy widening.
 *
 * The shipped renderer talks to the backend only through the Electron preload
 * IPC bridge, so frontend/index.html grants `connect-src 'self'` and nothing
 * on loopback. The Vite dev server additionally needs its HMR WebSocket, which
 * this transform adds for `vite serve` only; production builds never run it.
 */

import { ValidationError } from '../errors';

const CONNECT_SRC_SELF = "connect-src 'self'";

export function widenConnectSrcForDevServer(html: string, host: string, port: number): string {
  if (!html.includes(CONNECT_SRC_SELF)) {
    throw new ValidationError(
      `index.html CSP must contain "${CONNECT_SRC_SELF}" for the dev-server transform`,
      'Content-Security-Policy'
    );
  }
  return html.replace(CONNECT_SRC_SELF, `${CONNECT_SRC_SELF} ws://${host}:${port}`);
}

interface DevServerCspPlugin {
  name: string;
  apply: 'serve';
  configResolved: (config: { server: { host?: string | boolean; port?: number } }) => void;
  transformIndexHtml: (html: string) => string;
}

export function devServerCspPlugin(defaultHost: string, defaultPort: number): DevServerCspPlugin {
  let host = defaultHost;
  let port = defaultPort;
  return {
    name: 'pumas-dev-server-csp',
    apply: 'serve',
    configResolved(config) {
      if (typeof config.server.host === 'string') host = config.server.host;
      if (typeof config.server.port === 'number') port = config.server.port;
    },
    transformIndexHtml(html) {
      return widenConnectSrcForDevServer(html, host, port);
    },
  };
}
