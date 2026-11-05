import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { ValidationError } from '../errors';
import { devServerCspPlugin, widenConnectSrcForDevServer } from './devServerCsp';

const indexHtml = readFileSync(path.resolve(__dirname, '../../index.html'), 'utf8');

function contentSecurityPolicy(html: string): string {
  const policy = /<meta\s+http-equiv="Content-Security-Policy"\s+content="([^"]+)"/.exec(html)?.[1];
  expect(policy).toBeDefined();
  return policy ?? '';
}

function directive(policy: string, name: string): string | undefined {
  return policy.split(';').map((part) => part.trim()).find((part) => part.startsWith(`${name} `));
}

describe('renderer content security policy', () => {
  it('ships connect-src limited to self, with no loopback HTTP or WebSocket grant', () => {
    const policy = contentSecurityPolicy(indexHtml);
    expect(directive(policy, 'connect-src')).toBe("connect-src 'self'");
    expect(policy).not.toContain('127.0.0.1');
    expect(policy).not.toContain('localhost');
  });

  it('adds only the dev-server HMR WebSocket when served by Vite', () => {
    const plugin = devServerCspPlugin('127.0.0.1', 3000);
    plugin.configResolved({ server: { host: '127.0.0.1', port: 3100 } });

    expect(plugin.apply).toBe('serve');
    const policy = contentSecurityPolicy(plugin.transformIndexHtml(indexHtml));
    expect(directive(policy, 'connect-src')).toBe("connect-src 'self' ws://127.0.0.1:3100");
    expect(directive(policy, 'script-src')).toBe("script-src 'self'");
  });

  it('refuses to transform a policy whose connect-src it cannot find', () => {
    expect(() => widenConnectSrcForDevServer('<html></html>', '127.0.0.1', 3000)).toThrow(ValidationError);
  });
});
