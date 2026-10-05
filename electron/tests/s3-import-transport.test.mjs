import assert from 'node:assert/strict';
import test from 'node:test';
import { createServer } from 'node:http';
import { once } from 'node:events';
import log from 'electron-log';
import { PythonBridge } from '../dist/python-bridge.js';
import { receiveS3ImportRpc, S3_RPC_FAILURE, S3_RPC_RESPONSE_LIMIT } from '../dist/s3-import-rpc.js';
  const secret = 'synthetic-transport-secret-DO-NOT-LOG';
const id = 'c3f7d104-1234-4321-abcd-aaaaaaaaaaaa';

test('S3 transport contains reflected diagnostics, bounds responses and correlates envelopes', async () => {
  const fileLevel = log.transports.file.level;
  log.transports.file.level = false;
  let mode = 'idle';
  const server = createServer((request, response) => {
    let body = '';
    request.on('data', chunk => { body += chunk; });
    request.on('end', () => {
      const call = JSON.parse(body);
      response.writeHead(200, { 'Content-Type': 'application/json' });
      if (mode === 'invalid') return response.end(secret);
      if (mode === 'oversized') return response.end(secret.repeat(Math.ceil(S3_RPC_RESPONSE_LIMIT / secret.length) + 1));
      if (mode === 'error') return response.end(JSON.stringify({ jsonrpc: '2.0', id: call.id, error: { code: -32603, message: secret, data: secret } }));
      if (mode === 'wrong-id') return response.end(JSON.stringify({ jsonrpc: '2.0', id: call.id + 1, result: { status: 'idle' } }));
      if (mode === 'extra') return response.end(JSON.stringify({ jsonrpc: '2.0', id: call.id, result: { status: 'idle', credentials: secret } }));
      const error = { code: -32003, class: 'conflict', message: secret };
      const result = mode === 'failed'
        ? { status: 'finished', operation_id: id, result: { status: 'failed', error, retained_work: true, published_model_id: null } }
        : { status: 'idle' };
      response.end(JSON.stringify({ jsonrpc: '2.0', id: call.id, result }));
    });
  });
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  const bridge = new PythonBridge({ port: server.address().port, debug: true,
    rustBinaryPath: process.execPath, launcherRoot: process.cwd() });
  bridge.port = server.address().port; bridge.process = {};
  try {
    assert.deepEqual(JSON.parse(JSON.stringify(await bridge.call('get_s3_model_import', {}))), { status: 'idle' });
    for (mode of ['invalid', 'oversized', 'error', 'wrong-id', 'extra']) {
      await assert.rejects(bridge.call('get_s3_model_import', {}), error => {
        assert.equal(error.message, S3_RPC_FAILURE);
        assert.ok(!String(error.stack).includes(secret));
        assert.equal(error.cause, undefined);
        return true;
      });
    }
    mode = 'failed';
    const failed = await bridge.call('get_s3_model_import', {});
    assert.equal(failed.result.retained_work, true);
    assert.equal(failed.result.error.class, 'conflict');
    assert.ok(!JSON.stringify(failed).includes(secret));
    await assert.rejects(bridge.call('get_s3_model_import', { toJSON() { throw new Error(secret); } }),
      error => error.message === S3_RPC_FAILURE && !String(error.stack).includes(secret));
  } finally {
    server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
    log.transports.file.level = fileLevel;
  }
});

test('privileged S3 IPC receiving boundary never returns malformed values or exception detail', async () => {
  for (const invoke of [async () => ({ status: 'idle', secret }), async () => { throw new Error(secret); }]) {
    await assert.rejects(receiveS3ImportRpc('get_s3_model_import', invoke), error =>
      error.message === S3_RPC_FAILURE && error.cause === undefined && !String(error.stack).includes(secret));
  }
  const value = await receiveS3ImportRpc('cancel_s3_model_import', async () => ({ accepted: false,
    outcome: { status: 'rejected', error: { code: -32003, class: 'conflict', message: secret } } }));
  assert.equal(value.accepted, false);
  assert.equal(value.outcome.error.class, 'conflict');
  assert.ok(!JSON.stringify(value).includes(secret));
});
