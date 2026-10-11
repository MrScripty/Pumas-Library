import test from 'node:test';
import assert from 'node:assert/strict';
import { ADAPTERS, validateRequest, decodeResult, decodeCapabilities, capabilityRows, admit, template } from '../contract.mjs';
import { backendUrl, redact } from '../transport.mjs';
import { capabilities, PNG } from './fixture.mjs';

export function request(capability = 'text_generation') {
  const value = template(capability, 'fixture/model', 'fixture-profile', 'test:1');
  if (capability === 'audio_transcription') Object.assign(value.input, { sample_count: 2, data_base64: 'AAAAAA==' });
  if (capability === 'image_to_text') value.input.messages[0].content[1].data_base64 = PNG;
  return value;
}
test('public v1 request subsets match documented shapes for six adapters', () => {
  for (const name of ADAPTERS) {
    const value = request(name); assert.equal(validateRequest(value), value); admit(value, capabilities());
  }
  const audio = request('audio_transcription');
  assert.deepEqual(audio.input, { kind: 'audio', encoding: 'pcm_s16le', sample_rate_hz: 16000, channels: 1, sample_count: 2, data_base64: 'AAAAAA==' });
  assert.equal(request('image_generation').options.width, 512);
});
test('closed requests reject invalid variants, unsafe resource requests and input metadata', () => {
  const cases = [
    r => { r.extra = true; }, r => { r.stream = true; }, r => { r.options.max_tokens = 513; },
    r => { r.options.temperature = NaN; }, r => { r.request_id = 'bad id'; },
    r => { r.profile = ''; }, r => { r.capability = 'generation_3d'; }, r => { r.input.kind = 'audio'; },
    r => { r.output = 'png_base64'; }, r => { r.model = ' model '; },
  ];
  for (const change of cases) { const value = request(); change(value); assert.throws(() => validateRequest(value)); }
  const audio = request('audio_transcription'); audio.input.sample_count = 3;
  assert.throws(() => validateRequest(audio), /frames/);
  const floats = request('audio_transcription'); Object.assign(floats.input, { encoding: 'pcm_f32le', sample_count: 1, data_base64: 'AADAfw==' });
  assert.throws(() => validateRequest(floats), /number/);
  const image = request('image_generation'); image.options.width = 2048;
  image.options.height = 2048; assert.throws(() => validateRequest(image), /pixel/);
  const vision = request('image_to_text'); vision.input.messages[0].content[1].data_base64 = 'AAAA';
  assert.throws(() => validateRequest(vision), /signature/);
});
test('8 MiB attachments validate without regex stack overflow; padding and decoded byte overflow fail', () => {
  const value = request('image_to_text');
  const data = Buffer.alloc(8 * 1024 * 1024);
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).copy(data);
  value.input.messages[0].content[1].data_base64 = data.toString('base64');
  assert.equal(validateRequest(value), value);
  value.input.messages[0].content[1].data_base64 = Buffer.concat([data, Buffer.from([0])]).toString('base64');
  assert.throws(() => validateRequest(value), e => e.code === 'request_limit');
  value.input.messages[0].content[1].data_base64 = 'AA=A';
  assert.throws(() => validateRequest(value), /padding/);
});
test('capability support never implies inference coverage; new modalities require reviewed adapters', () => {
  const cap = capabilities(); decodeCapabilities(cap, 'fixture/model', 'fixture-profile');
  const rows = capabilityRows(cap);
  assert.equal(rows.find(r => r.capability === 'text_generation').support, 'supported');
  assert.ok(rows.every(r => r.execution === 'untested'));
  for (const name of ['audio_generation', 'image_editing', 'generation_3d', 'audio_classification']) assert.equal(rows.find(r => r.capability === name).support, 'unavailable');
  assert.ok(capabilityRows(null).every(r => r.support === 'untested'));
  cap.capabilities.push({ ...cap.capabilities[0], capability: 'future_capability' });
  assert.equal(capabilityRows(cap).at(-1).support, 'unavailable');
  cap.capabilities[1].option_bounds = [{ option: 'max_tokens', minimum: 1, maximum: 32 }];
  assert.throws(() => admit(request(), cap), /32/);
  cap.capabilities[1].availability = { state: 'unavailable', reason: 'runtime_unavailable' };
  assert.throws(() => admit(request(), cap), e => e.code === 'capability_unavailable');
});
test('malformed discovery and incoherent results are not successful inference', () => {
  const cap = capabilities(); cap.capabilities[0].availability = {};
  assert.throws(() => decodeCapabilities(cap, 'fixture/model'), /descriptor/);
  const badLimit = capabilities(); badLimit.max_request_bytes = '33554432';
  assert.throws(() => decodeCapabilities(badLimit, 'fixture/model'), e => e.code === 'invalid_response');
  const value = { contract_version: 1, request_id: 'test:1', result: { kind: 'text', text: 'controlled', finish_reason: 'stop' } };
  assert.equal(decodeResult(value, request()).state, 'succeeded');
  assert.deepEqual(decodeResult({ ...value, result: { ...value.result, provider_private: 'never projected' } }, request()).result, value.result);
  assert.throws(() => decodeResult({ ...value, request_id: 'different' }, request()), /correlation/);
  assert.throws(() => decodeResult({ ...value, result: { ...value.result, finish_reason: 'unknown' } }, request()), /text result/);
  const embedding = request('text_embedding');
  assert.throws(() => decodeResult({ ...value, result: { kind: 'embeddings', vectors: [[Infinity]] } }, embedding), /embedding/);
  assert.deepEqual(decodeResult({ contract_version: 1, request_id: 'test:1', error: { code: 'provider_failure', outcome: 'unknown' } }, request()), { state: 'error', error: { code: 'provider_failure', outcome: 'unknown' } });
});
test('endpoint containment and receipts redact selected secrets and binary data', () => {
  assert.equal(backendUrl('http://127.0.0.1:8080'), 'http://127.0.0.1:8080');
  for (const url of ['https://127.0.0.1', 'http://localhost', 'http://example.com', 'http://127.0.0.1@evil.example', 'http://127.0.0.1/?token=secret', 'http://127.0.0.1/path']) assert.throws(() => backendUrl(url));
  const safe = JSON.stringify(redact({ api_key: 'private', text: 'Bearer abc123 hf_abcdefgh1234 literal-secret https://u:p@local/a?token=no', data_base64: PNG, max_tokens: 32 }, ['literal-secret']));
  for (const forbidden of ['private', 'abc123', 'abcdefgh1234', 'literal-secret', 'u:p', 'token=no', PNG]) assert.ok(!safe.includes(forbidden));
  assert.ok(safe.includes('"max_tokens":32'));
});
