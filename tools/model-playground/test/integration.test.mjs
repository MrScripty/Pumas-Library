import test from 'node:test';
import assert from 'node:assert/strict';
import { once } from 'node:events';
import { Transport } from '../transport.mjs';
import { PumasClient } from '../client.mjs';
import { template, ADAPTERS } from '../contract.mjs';
import { createPlayground } from '../server.mjs';
import { startFixture, FENCE, PNG } from './fixture.mjs';

function request(name = 'text_generation') {
  const value = template(name, 'fixture/model', 'fixture-profile', 'test:1');
  if (name === 'audio_transcription') Object.assign(value.input, { sample_count: 2, data_base64: 'AAAAAA==' });
  if (name === 'image_to_text') value.input.messages[0].content[1].data_base64 = PNG;
  return value;
}
async function setup(t) {
  const fixture = await startFixture(); t.after(() => fixture.close());
  const transport = new Transport(fixture.state.endpoint, 'test-private-token');
  const client = new PumasClient(transport);
  await client.discover(new AbortController().signal, []);
  return { fixture, client, transport };
}
test('loopback HTTP discovery, fencing, plugin inventory and six finite adapters (simulated inference)', async t => {
  const { client, fixture } = await setup(t);
  for (const name of ADAPTERS) {
    const receipts = [];
    const result = await client.run(request(name), FENCE, new AbortController().signal, receipts);
    assert.equal(result.state, 'succeeded');
    assert.equal(receipts.at(-1).outcome, 'completed');
    assert.equal(receipts.at(-1).http_status, 200);
    assert.equal(receipts.at(-1).execution, 'real_http');
    assert.ok(!JSON.stringify(receipts).includes('test-private-token'));
  }
  assert.equal(fixture.state.operations.length, 6);
  assert.ok(fixture.state.requests.slice(1).every(r => r.headers['pumas-service-generation'] === FENCE.service));
});
test('unsupported, stale and malformed capabilities never admit a model operation', async t => {
  const { client, fixture } = await setup(t);
  for (const mode of ['unavailable', 'stale', 'malformed-capabilities']) {
    fixture.state.mode = mode;
    await assert.rejects(client.run(request(), FENCE, new AbortController().signal, []));
    assert.equal(fixture.state.operations.length, 0);
  }
  await assert.rejects(client.run(request(), { ...FENCE, service: 'replaced' }, new AbortController().signal, []), e => e.code === 'stale_discovery');
});
test('real HTTP errors, correlation loss and disconnect preserve outcomes without replay', async t => {
  const { client, fixture } = await setup(t);
  for (const [mode, code, outcome] of [['provider-error', 'provider_failure', 'unknown'], ['not-admitted', 'capability_unavailable', 'not_admitted'],
    ['wrong-id', 'invalid_response', 'unknown'], ['disconnect', 'transport_lost', 'unknown'], ['redirect', 'redirect_refused', 'unknown'], ['not-json', 'invalid_response', 'unknown']]) {
    fixture.state.mode = mode; const before = fixture.state.operations.length;
    const result = await client.run(request(), FENCE, new AbortController().signal, []);
    assert.equal(result.error.code, code); assert.equal(result.error.outcome, outcome);
    assert.equal(fixture.state.operations.length, before + 1);
  }
});
test('explicit cancellation closes a silent operation without claiming provider cessation', async t => {
  const { client, fixture } = await setup(t); fixture.state.mode = 'hang';
  const controller = new AbortController();
  const admitted = once(fixture.server, 'request');
  const result = client.run(request(), FENCE, controller.signal, []);
  await admitted; // capability observation precedes admission
  while (!fixture.state.pending.size) await new Promise(resolve => setTimeout(resolve, 5));
  const closed = once([...fixture.state.pending][0], 'close'); controller.abort();
  assert.equal((await result).error.outcome, 'unknown'); await closed;
  assert.equal(fixture.state.disconnects, 1); assert.equal(fixture.state.operations.length, 1);
});
test('whole response byte bound rejects before parsing', async t => {
  const { transport, fixture } = await setup(t); fixture.state.mode = 'oversize';
  const result = await transport.exchange('/v1/model-operations', { body: request(), fence: FENCE, generation: true, maxBytes: 1024 });
  assert.equal(result.error.code, 'response_limit'); assert.equal(fixture.state.operations.length, 1);
});
test('embedding transport loss remains unknown even with bounded observation transport', async t => {
  const { client, fixture } = await setup(t); fixture.state.mode = 'disconnect';
  const result = await client.run(request('text_embedding'), FENCE, new AbortController().signal, []);
  assert.equal(result.error.outcome, 'unknown'); assert.equal(fixture.state.operations.length, 1);
});
test('browser disconnect and server shutdown close owned upstream work', async t => {
  const fixture = await startFixture(); t.after(() => fixture.close());
  const app = createPlayground({ backend: fixture.state.endpoint });
  await new Promise(resolve => app.server.listen(0, '127.0.0.1', resolve));
  let closed = false; t.after(async () => { if (!closed) await app.close(); });
  const origin = `http://127.0.0.1:${app.server.address().port}`;
  const html = await (await fetch(origin)).text();
  const session = /name="playground-session" content="([^"]+)"/.exec(html)[1];
  const post = (path, input, signal) => fetch(origin + path, { method: 'POST', signal,
    headers: { 'content-type': 'application/json', 'x-playground-session': session }, body: JSON.stringify(input) });
  await (await post('/api/discover', { id: 'discover' })).json(); fixture.state.mode = 'hang';
  const abort = new AbortController();
  const first = post('/api/run', { id: 'first', request: request(), fence: FENCE }, abort.signal).catch(e => e);
  while (!fixture.state.pending.size) await new Promise(resolve => setTimeout(resolve, 5));
  const disconnected = once([...fixture.state.pending][0], 'close'); abort.abort(); await first; await disconnected;
  const second = post('/api/run', { id: 'second', request: request(), fence: FENCE }).catch(e => e);
  while (!fixture.state.pending.size) await new Promise(resolve => setTimeout(resolve, 5));
  const drained = once([...fixture.state.pending][0], 'close');
  await app.close(); closed = true; await second; await drained;
  assert.equal(fixture.state.disconnects, 2); assert.equal(fixture.state.operations.length, 2);
});
test('browser listener rejects cross-origin/missing session, returns receipts and refuses concurrent work', async t => {
  const fixture = await startFixture(); t.after(() => fixture.close());
  const app = createPlayground({ backend: fixture.state.endpoint });
  await new Promise(resolve => app.server.listen(0, '127.0.0.1', resolve)); t.after(() => app.close());
  const origin = `http://127.0.0.1:${app.server.address().port}`;
  const html = await (await fetch(origin)).text();
  const session = /name="playground-session" content="([^"]+)"/.exec(html)[1];
  const post = (path, input, headers = {}) => fetch(origin + path, { method: 'POST', headers: { 'content-type': 'application/json', 'x-playground-session': session, ...headers }, body: JSON.stringify(input) });
  assert.equal((await post('/api/discover', { id: 'bad' }, { origin: 'https://evil.example' })).status, 403);
  assert.equal((await post('/api/discover', { id: 'bad' }, { 'x-playground-session': '' })).status, 403);
  const discovery = await (await post('/api/discover', { id: 'discover' })).json();
  assert.equal(discovery.models[0].id, 'fixture/model');
  assert.equal(discovery.plugins[0].id, 'torch');
  assert.ok(!html.includes('test-private-token'));
  fixture.state.mode = 'hang';
  const pending = post('/api/run', { id: 'running', request: request(), fence: FENCE });
  while (!fixture.state.pending.size) await new Promise(resolve => setTimeout(resolve, 5));
  assert.equal((await post('/api/discover', { id: 'competing' })).status, 409);
  assert.equal((await post('/api/cancel', { id: 'wrong' })).status, 400);
  const cancel = await (await post('/api/cancel', { id: 'running' })).json();
  assert.equal(cancel.outcome, 'unknown');
  const result = await (await pending).json();
  assert.equal(result.error.code, 'cancel_requested'); assert.equal(result.receipts.at(-1).outcome, 'unknown');
});
