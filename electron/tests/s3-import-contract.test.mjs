import assert from 'node:assert/strict';
import test from 'node:test';
import { validateApiCallPayload } from '../dist/ipc-validation.js';
import { decodeS3ImportOutcome, decodeS3ImportCancelOutcome } from '../dist/generated/desktop-contract.js';

const id = 'c3f7d104-1234-4321-abcd-aaaaaaaaaaaa';
const request = { operation_id: id, endpoint: 'https://source.invalid', region: 'fixture-region',
  bucket: 'fixture-bucket', addressing: 'path', key: 'models/exact object.gguf',
  version_id: 'exact+version/id', filename: 'weights.gguf', sha256: 'a'.repeat(64),
  family: 'fixture', official_name: 'Fixture GGUF' };

test('S3 IPC admits a closed anonymous request with exact source pins and copies it', () => {
  const value = validateApiCallPayload('start_s3_model_import', request);
  assert.deepEqual(JSON.parse(JSON.stringify(value.params)), request);
  assert.notEqual(value.params, request);
  assert.ok(Object.isFrozen(value.params));
  for (const patch of [{ credentials: 'synthetic-secret' }, { session_token: 'synthetic-token' },
    { allow_http: true }, { sha256: 'bad' }, { version_id: '' }, { addressing: 'auto' },
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
