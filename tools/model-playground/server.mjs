import http from 'node:http';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { timingSafeEqual, randomBytes } from 'node:crypto';
import { LIMITS, PlaygroundError, assert, object } from './contract.mjs';
import { Transport } from './transport.mjs';
import { PumasClient } from './client.mjs';

const FILES = { '/': ['public/index.html', 'text/html'], '/app.mjs': ['public/app.mjs', 'text/javascript'],
  '/style.css': ['public/style.css', 'text/css'], '/contract.mjs': ['contract.mjs', 'text/javascript'] };
function equal(a, b) { return typeof a === 'string' && Buffer.byteLength(a) === Buffer.byteLength(b) && timingSafeEqual(Buffer.from(a), Buffer.from(b)); }
async function body(req) {
  assert(req.headers['content-type'] === 'application/json', 'Expected application/json.');
  const chunks = []; let size = 0;
  for await (const chunk of req) {
    size += chunk.length; assert(size <= LIMITS.requestBytes + 4096, 'Playground request byte limit exceeded.', 'request_limit'); chunks.push(chunk);
  }
  try { return JSON.parse(Buffer.concat(chunks).toString('utf8')); }
  catch { throw new PlaygroundError('invalid_request', 'Request is not valid JSON.'); }
}
function fields(value, keys) {
  assert(object(value) && keys.every(k => Object.hasOwn(value, k)) && Object.keys(value).every(k => keys.includes(k)), 'Invalid API request fields.');
}
export function createPlayground({ backend, token = '' }) {
  const transport = new Transport(backend, token);
  const client = new PumasClient(transport);
  const browserToken = randomBytes(32).toString('hex');
  let active = null, closing = false;
  const server = http.createServer(async (req, res) => {
    const origin = `http://127.0.0.1:${server.address().port}`;
    res.setHeader('cache-control', 'no-store'); res.setHeader('x-content-type-options', 'nosniff');
    res.setHeader('content-security-policy', "default-src 'self'; img-src 'self' blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'");
    const send = (status, value) => { if (!res.destroyed) { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(value)); } };
    const receipts = [];
    try {
      assert(req.headers.host === origin.slice(7) && (!req.headers.origin || req.headers.origin === origin) &&
        !['cross-site', 'same-site'].includes(req.headers['sec-fetch-site']), 'Only this playground origin may access the listener.', 'origin_refused');
      assert(!closing, 'Playground is shutting down.', 'unavailable');
      if (req.method === 'GET' && Object.hasOwn(FILES, req.url)) {
        const [path, type] = FILES[req.url];
        let content = await readFile(new URL(path, import.meta.url));
        if (req.url === '/') content = Buffer.from(content.toString().replace('__SESSION__', browserToken));
        res.writeHead(200, { 'content-type': `${type}; charset=utf-8` }); res.end(content); return;
      }
      assert(req.method === 'POST' && ['/api/discover', '/api/inspect', '/api/run', '/api/cancel'].includes(req.url), 'Unknown route.', 'not_found');
      assert(equal(req.headers['x-playground-session'], browserToken), 'Missing playground session.', 'origin_refused');
      const input = await body(req);
      if (req.url === '/api/cancel') {
        fields(input, ['id']);
        assert(active && active.id === input.id, 'No matching active request.', 'not_found');
        active.controller.abort();
        send(200, { state: 'cancel_requested', outcome: 'unknown', message: 'Connection cancellation requested; provider cessation is unconfirmed.' }); return;
      }
      assert(!active, 'One request may run at a time. Cancel or wait for the current request.', 'busy');
      fields(input, req.url === '/api/discover' ? ['id'] : req.url === '/api/inspect' ? ['id', 'model', 'profile', 'fence'] : ['id', 'request', 'fence']);
      assert(typeof input.id === 'string' && /^[A-Za-z0-9_.:-]{1,128}$/.test(input.id), 'Invalid invocation ID.');
      const invocation = { id: input.id, controller: new AbortController(), done: null };
      active = invocation;
      const lost = () => { if (!res.writableEnded) invocation.controller.abort(); };
      res.once('close', lost);
      invocation.done = (async () => {
        try {
          const signal = invocation.controller.signal;
          const result = req.url === '/api/discover' ? await client.discover(signal, receipts) :
            req.url === '/api/inspect' ? await client.inspect(input.model, input.profile, input.fence, signal, receipts) :
              await client.run(input.request, input.fence, signal, receipts);
          send(200, { id: input.id, ...result, receipts, limits: LIMITS });
        } catch (error) {
          const cancelled = invocation.controller.signal.aborted;
          const known = error instanceof PlaygroundError;
          send(400, { id: input.id, state: 'error', error: { code: cancelled ? 'cancel_requested' : known ? error.code : 'internal_error',
            message: cancelled ? 'Cancellation requested; provider cessation is unconfirmed.' : known ? error.message : 'Playground could not complete the request.',
            outcome: cancelled ? (receipts.some(r => r.path === '/v1/model-operations' && r.outcome === 'unknown') ? 'unknown' : 'not_admitted') : known ? error.outcome : 'unknown' }, receipts });
        } finally { res.removeListener('close', lost); if (active === invocation) active = null; }
      })();
      await invocation.done;
    } catch (error) {
      send(error.code === 'busy' ? 409 : error.code === 'origin_refused' ? 403 : 400,
        { state: 'error', error: { code: error.code ?? 'invalid_request', message: error instanceof PlaygroundError ? error.message : 'Invalid request.', outcome: 'not_admitted' }, receipts });
    }
  });
  server.maxConnections = 16; server.requestTimeout = 30000; server.headersTimeout = 10000;
  server.keepAliveTimeout = 5000;
  return { server, async close() {
    closing = true; const pending = active; pending?.controller.abort();
    const closed = new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
    server.closeAllConnections(); await pending?.done; await closed;
  } };
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const { values } = parseArgs({ options: { backend: { type: 'string' }, port: { type: 'string', default: '8787' }, help: { type: 'boolean' } } });
    if (values.help) {
      process.stdout.write('Usage: node tools/model-playground/server.mjs --backend http://127.0.0.1:8080 [--port 8787]\nOptional PUMAS_PLAYGROUND_TOKEN stays server-side. No models are loaded or downloaded.\n');
    } else {
      assert(values.backend, 'An explicit --backend loopback URL is required.');
      assert(/^\d+$/.test(values.port) && Number(values.port) > 0 && Number(values.port) <= 65535, 'Invalid port.');
      const app = createPlayground({ backend: values.backend, token: process.env.PUMAS_PLAYGROUND_TOKEN });
      app.server.on('error', () => { process.stderr.write('Playground listener failed. Check the selected port.\n'); process.exitCode = 1; });
      app.server.listen(Number(values.port), '127.0.0.1', () => process.stdout.write(`Pumas playground: http://127.0.0.1:${values.port}\n`));
      let stopping = false;
      const stop = async () => { if (stopping) return; stopping = true; await app.close(); };
      process.once('SIGINT', stop); process.once('SIGTERM', stop);
    }
  } catch (error) { process.stderr.write(`${error instanceof PlaygroundError ? error.message : 'Invalid launch arguments.'}\n`); process.exitCode = 1; }
}
