import http from 'node:http';
import { isIP } from 'node:net';
import { performance } from 'node:perf_hooks';
import { LIMITS, PlaygroundError, assert, object } from './contract.mjs';

export function backendUrl(raw) {
  const url = new URL(raw);
  const host = url.hostname.replace(/^\[|\]$/g, '');
  assert(url.protocol === 'http:' && isIP(host) && (host === '::1' || host.startsWith('127.')) &&
    !url.username && !url.password && !url.search && !url.hash && url.pathname === '/' && url.port !== '0',
  'Backend must be a numeric loopback HTTP base URL.');
  return url.origin;
}
export function redact(value, secrets = [], depth = 0) {
  if (depth > 20) return '[depth omitted]';
  if (typeof value === 'string') {
    let safe = value;
    for (const secret of secrets.filter(Boolean)) safe = safe.replaceAll(secret, '[REDACTED]');
    safe = safe.replace(/\b(?:Bearer\s+)[A-Za-z0-9._~+/=-]+/gi, 'Bearer [REDACTED]')
      .replace(/\b(?:sk-[A-Za-z0-9_-]{8,}|hf_[A-Za-z0-9]{8,})/g, '[REDACTED]')
      .replace(/\b(https?:\/\/)[^\s"<>]+/gi, match => {
        try { const url = new URL(match); url.username = ''; url.password = ''; url.search = ''; url.hash = ''; return url.toString(); }
        catch { return '[URL omitted]'; }
      });
    return safe.length > 16000 ? `${safe.slice(0, 16000)} [truncated]` : safe;
  }
  if (Array.isArray(value)) return value.slice(0, 128).map(item => redact(item, secrets, depth + 1));
  if (object(value)) return Object.fromEntries(Object.entries(value).slice(0, 128).map(([key, item]) => [key,
    /(?:token|secret|password|authorization|cookie|api[_-]?key|credential)/i.test(key) && !['max_tokens', 'max_output_tokens'].includes(key)
      ? '[REDACTED]' : /(?:base64|b64_json)/i.test(key) ? `[binary omitted: ${typeof item === 'string' ? item.length : 0} encoded characters]` : redact(item, secrets, depth + 1)]));
  return value;
}
export class Transport {
  constructor(endpoint, token = '') { this.endpoint = backendUrl(endpoint); this.token = token; }
  async exchange(path, { body, signal, fence, generation = false, maxBytes = LIMITS.responseBytes } = {}) {
    assert(path.startsWith('/') && !path.startsWith('//'), 'Invalid route.');
    const started = performance.now();
    const receipt = { method: body === undefined ? 'GET' : 'POST', path,
      request: body === undefined ? null : redact(body, [this.token]), started_at: new Date().toISOString(),
      execution: 'real_http', http_status: null, response_bytes: 0, outcome: 'not_admitted' };
    const payload = body === undefined ? undefined : JSON.stringify(body);
    const inference = path === '/v1/model-operations';
    let dispatched = false;
    try {
      const result = await new Promise((resolve, reject) => {
        const headers = { accept: 'application/json', ...(payload ? { 'content-type': 'application/json', 'content-length': Buffer.byteLength(payload) } : {}),
          ...(this.token ? { authorization: `Bearer ${this.token}` } : {}),
          ...(fence ? { 'pumas-instance-generation': fence.instance, 'pumas-service-generation': fence.service } : {}) };
        const req = http.request(new URL(path, this.endpoint), { method: receipt.method, headers, agent: false, signal }, res => {
          receipt.http_status = res.statusCode;
          if (res.statusCode >= 300 && res.statusCode < 400) { res.destroy(); reject(new PlaygroundError('redirect_refused', 'Redirects are never followed.', dispatched ? 'unknown' : 'not_admitted')); return; }
          const chunks = []; let size = 0;
          res.on('data', chunk => {
            size += chunk.length; receipt.response_bytes = size;
            if (size > maxBytes) { res.destroy(new PlaygroundError('response_limit', 'Response byte limit exceeded.', 'unknown')); return; }
            chunks.push(chunk);
          });
          res.on('error', reject);
          res.on('end', () => {
            try {
              assert(String(res.headers['content-type'] ?? '').split(';')[0] === 'application/json', 'Expected a JSON response.', 'invalid_response');
              const value = JSON.parse(Buffer.concat(chunks).toString('utf8'));
              resolve({ status: res.statusCode, value });
            } catch (error) { reject(error instanceof PlaygroundError ? error : new PlaygroundError('invalid_response', 'Response is not valid JSON.', 'unknown')); }
          });
        });
        let connectTimer = setTimeout(() => req.destroy(new PlaygroundError('connect_timeout', 'Could not connect within 10 seconds.')), 10000);
        const readTimer = generation ? null : setTimeout(() => req.destroy(new PlaygroundError('observation_timeout', 'Observation exceeded 30 seconds.')), 30000);
        req.once('socket', socket => socket.once('connect', () => { clearTimeout(connectTimer); connectTimer = null; dispatched = true; }));
        req.on('error', reject);
        req.once('close', () => { clearTimeout(connectTimer); clearTimeout(readTimer); });
        req.end(payload);
      });
      receipt.response = redact(result.value, [this.token]);
      receipt.outcome = inference ? 'unknown' : 'observed';
      return { ...result, receipt };
    } catch (error) {
      const code = signal?.aborted ? 'cancel_requested' : error instanceof PlaygroundError ? error.code : 'transport_lost';
      receipt.outcome = dispatched && inference ? 'unknown' : 'not_admitted';
      receipt.error = { code, outcome: receipt.outcome, message: error instanceof PlaygroundError ? error.message :
        signal?.aborted ? 'Connection closed; provider cessation is unconfirmed.' : 'Local transport failed. Verify the selected Pumas endpoint.' };
      return { status: null, value: null, receipt, error: receipt.error };
    } finally { receipt.elapsed_ms = Math.round(performance.now() - started); }
  }
}
