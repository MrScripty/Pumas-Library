import assert from 'node:assert/strict';
import test from 'node:test';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { spawn } from 'node:child_process';
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

test('S3 local transport stays direct despite a proxy-configured global agent and never follows redirects', async () => {
  let forwarded = 0;
  const proxy = createServer((_request, response) => { forwarded += 1; response.writeHead(502); response.end(); });
  let calls = 0;
  const direct = createServer((request, response) => {
    let body = ''; request.on('data', chunk => { body += chunk; });
    request.on('end', () => {
      calls += 1; const call = JSON.parse(body);
      response.writeHead(calls === 1 ? 200 : 302, { 'Content-Type': 'application/json', Location: `http://127.0.0.1:${proxy.address().port}/redirect` });
      response.end(JSON.stringify({ jsonrpc: '2.0', id: call.id, result: { status: 'idle' } }));
    });
  });
  proxy.listen(0, '127.0.0.1'); direct.listen(0, '127.0.0.1');
  await Promise.all([once(proxy, 'listening'), once(direct, 'listening')]);
  const script = `
    const assert = require('node:assert/strict');
    const http = require('node:http');
    const log = require('electron-log'); log.transports.file.level = false;
    http.globalAgent = new http.Agent({proxyEnv:{HTTP_PROXY:'http://127.0.0.1:${proxy.address().port}'}});
    const {PythonBridge} = require(${JSON.stringify(new URL('../dist/python-bridge.js', import.meta.url).pathname)});
    const bridge = new PythonBridge({port:${direct.address().port},debug:false,rustBinaryPath:process.execPath,launcherRoot:process.cwd()});
    bridge.port=${direct.address().port}; bridge.process={};
    (async () => {
      assert.equal((await bridge.call('start_authenticated_s3_model_import', {})).status,'idle');
      await assert.rejects(bridge.call('get_s3_model_import', {}));
    })().catch(() => {process.exitCode=1;});
  `;
  try {
    const child = spawn(process.execPath, ['-e', script], { cwd: new URL('..', import.meta.url), stdio: ['ignore', 'ignore', 'ignore'] });
    const [code] = await once(child, 'close');
    assert.equal(code, 0); assert.equal(calls, 2); assert.equal(forwarded, 0);
  } finally {
    direct.closeAllConnections(); proxy.closeAllConnections();
    await Promise.all([new Promise(resolve => direct.close(resolve)), new Promise(resolve => proxy.close(resolve))]);
  }
});

test('bundle observation receiving boundary projects reflected errors before IPC', async () => {
  const observation={outcome:{status:'finished',operation_id:'c3f7d104-1234-4321-abcd-aaaaaaaaaaaa',result:{status:'failed',error:{code:-32603,class:'internal',message:'synthetic-bundle-reflected-secret'},retained_work:true,published_model_id:'fixture/model'}},bundle_progress:null};
  const safe=await receiveS3ImportRpc('get_s3_model_bundle_import',async()=>observation);
  assert.equal(safe.outcome.result.published_model_id,'fixture/model');assert.ok(!JSON.stringify(safe).includes('synthetic-bundle-reflected-secret'));
  await assert.rejects(()=>receiveS3ImportRpc('get_s3_model_bundle_import',async()=>({...observation,credentials:'synthetic-bundle-reflected-secret'})),error=>String(error)===`Error: ${S3_RPC_FAILURE}`);
});
