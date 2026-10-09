import assert from 'node:assert/strict';
import test from 'node:test';
import { validateApiCallPayload } from '../dist/ipc-validation.js';
import { decodeS3ImportOutcome, decodeS3ImportCancelOutcome } from '../dist/generated/desktop-contract.js';

const id = 'c3f7d104-1234-4321-abcd-aaaaaaaaaaaa';
const request = { operation_id: id, endpoint: 'https://source.invalid', region: 'fixture-region',
  bucket: 'fixture-bucket', addressing: 'path', key: 'models/exact object.gguf',
  version_id: 'exact+version/id', filename: 'weights.gguf', sha256: 'a'.repeat(64),
  family: 'fixture', official_name: 'Fixture GGUF' };

test('S3 IPC transports qualified model selections without assigning semantics to extensions', () => {
  for (const filename of ['weights.gguf', 'model.safetensors', 'model.onnx', 'unknown.data']) {
    const input = { ...request, filename };
    assert.deepEqual(JSON.parse(JSON.stringify(validateApiCallPayload('start_s3_model_import', input).params)), input);
  }
  const {key,version_id,filename,sha256,...source}=request;
  for (const primary of ['model-00001.safetensors', 'unet/model.safetensors']) {
    const input = { ...source, primary_logical_path: primary, files: [
      {key,version_id,logical_path:primary,sha256},
      {key:'models/second',version_id:'v2',logical_path:'model-00002.safetensors',sha256:'b'.repeat(64)},
      {key:'models/index',version_id:'v3',logical_path:'model.safetensors.index.json',sha256:'c'.repeat(64)},
    ]};
    assert.deepEqual(JSON.parse(JSON.stringify(validateApiCallPayload('start_s3_model_bundle_import', input).params)), input);
  }
  for (const filename of ['../model.safetensors', 'unet//model.safetensors', 'unet/CON.safetensors', 'unet/model.safetensors.', 'x'.repeat(1025)]) {
    assert.throws(() => validateApiCallPayload('start_s3_model_import', {...request,filename}));
  }
});

test('authenticated S3 IPC admits only an explicit closed bounded credential object', () => {
  const credentials = { access_key_id: 'synthetic-ipc-key', secret_access_key: 'synthetic-ipc-secret', session_token: 'synthetic-ipc-token' };
  for (const session_token of [undefined, null, credentials.session_token]) {
    const input = { source: request, credentials: { ...credentials, session_token } };
    if (session_token === undefined) delete input.credentials.session_token;
    const decoded = validateApiCallPayload('start_authenticated_s3_model_import', input);
    assert.deepEqual(JSON.parse(JSON.stringify(decoded.params)), input);
    assert.notEqual(decoded.params.credentials, input.credentials);
  }
  for (const patch of [{ secret_access_key: '' }, { secret_access_key: 'bad\n' }, { session_token: '' },
    { session_token: 'bad\n' }, { access_key_id: 'bad/key' }, { access_key_id: 'bad=key' },
    { secret_access_key: 'x'.repeat(4097) }, { profile: 'synthetic-ipc-secret' }]) {
    assert.throws(() => validateApiCallPayload('start_authenticated_s3_model_import', { source: request, credentials: { ...credentials, ...patch } }),
      error => !String(error).includes('synthetic-ipc-secret'));
  }
  assert.throws(() => validateApiCallPayload('start_authenticated_s3_model_import', { source: request, credentials, saved: true }));
  assert.throws(() => validateApiCallPayload('start_authenticated_s3_model_import', { source: { ...request, version_id: 'null' }, credentials }));
});

test('S3 IPC admits a closed anonymous request with exact source pins and copies it', () => {
  const value = validateApiCallPayload('start_s3_model_import', request);
  assert.deepEqual(JSON.parse(JSON.stringify(value.params)), request);
  assert.notEqual(value.params, request);
  assert.ok(Object.isFrozen(value.params));
  for (const patch of [{ credentials: 'synthetic-secret' }, { session_token: 'synthetic-token' },
    { allow_http: true }, { sha256: 'bad' }, { version_id: '' }, { version_id: 'null' },
    { key: '../weights.gguf' }, { key: 'models/./weights.gguf' }, { key: 'models//weights.gguf' },
    { key: '/weights.gguf' }, { key: 'weights.gguf/' }, { addressing: 'auto' },
    { endpoint: 'http://source.invalid' }, { endpoint: 'https://user:synthetic-secret@source.invalid' },
    { filename: '../weights.gguf' }, { operation_id: id.toUpperCase() }]) {
    assert.throws(() => validateApiCallPayload('start_s3_model_import', { ...request, ...patch }),
      error => error.message === 'Invalid API params for method: start_s3_model_import');
  }
  assert.equal(validateApiCallPayload('get_s3_model_import', { operation_id: null }).params.operation_id, null);
  assert.equal(validateApiCallPayload('cancel_s3_model_import', { operation_id: id }).params.operation_id, id);
  for (const method of ['get_s3_model_import', 'cancel_s3_model_import']) {
    assert.throws(() => validateApiCallPayload(method, { operation_id: id, token: 'synthetic-secret' }));
  }
});

test('S3 response decoders retain exact u64 progress and typed custody outcomes', () => {
  const running = { status: 'running', operation_id: id,
    progress: { phase: 'acquiring', downloaded_for_current_file: '18446744073709551615' } };
  const error = { code: -32603, class: 'internal', message: 'Internal error' };
  for (const outcome of [running, { status: 'unavailable' }, { status: 'idle' },
    { status: 'not_found', operation_id: id }, { status: 'rejected', error },
    { status: 'finished', operation_id: id, result: { status: 'completed', model_id: 'fixture/model' } },
    { status: 'finished', operation_id: id, result: { status: 'cancelled', retained_work: true } },
    { status: 'finished', operation_id: id, result: { status: 'failed', error, retained_work: true, published_model_id: 'fixture/model' } }]) {
    const decoded = decodeS3ImportOutcome(outcome);
    assert.equal(decoded.status, 'valid');
    assert.deepEqual(JSON.parse(JSON.stringify(decoded.value)), outcome);
    assert.equal(decodeS3ImportCancelOutcome({ accepted: false, outcome }).status, 'valid');
  }
  for (const progress of [0, 18446744073709551615, '-1', '01', '1e20', '9'.repeat(21)]) {
    assert.equal(decodeS3ImportOutcome({ ...running,
      progress: { ...running.progress, downloaded_for_current_file: progress } }).status, 'invalid');
  }
  assert.equal(decodeS3ImportOutcome({ ...running, token: 'synthetic-secret' }).status, 'invalid');
});

test('bundle IPC requires a bounded complete per-file pin set and preserves ephemeral authenticated construction', () => {
  const {key,version_id,filename,sha256,...source}=request;
  const bundle={...source,primary_logical_path:filename,files:[{key,version_id,logical_path:filename,sha256},{key,version_id:'other-version',logical_path:'config/data.json',sha256:'b'.repeat(64)}]};
  assert.deepEqual(JSON.parse(JSON.stringify(validateApiCallPayload('start_s3_model_bundle_import',bundle).params)),bundle);
  const credentials={access_key_id:'synthetic-bundle-ipc-key',secret_access_key:'synthetic-bundle-ipc-secret',session_token:null};
  const decoded=validateApiCallPayload('start_authenticated_s3_model_bundle_import',{source:bundle,credentials});
  assert.deepEqual(JSON.parse(JSON.stringify(decoded.params)),{source:bundle,credentials});assert.notEqual(decoded.params.credentials,credentials);
  for(const patch of [{version_id:'null'},{key:'../bad'},{logical_path:'../data.json'},{logical_path:'CON.json'},{sha256:'bad'},{credentials}]) {
    assert.throws(()=>validateApiCallPayload('start_s3_model_bundle_import',{...bundle,files:[bundle.files[0],{...bundle.files[1],...patch}]}),error=>error.message==='Invalid S3 bundle parameters');
  }
  for(const files of [[],[bundle.files[0]],Array(33).fill(bundle.files[0])]) assert.throws(()=>validateApiCallPayload('start_s3_model_bundle_import',{...bundle,files}));
  assert.throws(()=>validateApiCallPayload('start_authenticated_s3_model_bundle_import',{source:bundle,credentials:{...credentials,profile:credentials.secret_access_key}}),error=>!String(error).includes(credentials.secret_access_key));
  assert.deepEqual(JSON.parse(JSON.stringify(validateApiCallPayload('get_s3_model_bundle_import',{operation_id:id}).params)),{operation_id:id});
});
