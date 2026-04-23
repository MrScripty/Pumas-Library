import assert from 'node:assert/strict';
import test from 'node:test';
import {
  ALLOWED_RPC_METHODS,
  sanitizeOpenDialogOptions,
  validateApiCallPayload,
  validateExternalUrl,
} from '../dist/ipc-validation.js';

test('RPC method registry has stable unique method names', () => {
  assert.ok(ALLOWED_RPC_METHODS.length > 100);
  assert.deepEqual(
    [...new Set(ALLOWED_RPC_METHODS)],
    [...ALLOWED_RPC_METHODS]
  );
  assert.ok(ALLOWED_RPC_METHODS.includes('get_status'));
  assert.ok(ALLOWED_RPC_METHODS.includes('torch_configure'));
});

test('validateApiCallPayload rejects unknown methods and non-record params', () => {
  assert.deepEqual(validateApiCallPayload('get_status', undefined), {
    method: 'get_status',
    params: {},
  });
  assert.deepEqual(validateApiCallPayload('get_status', {}), {
    method: 'get_status',
    params: {},
  });
  assert.deepEqual(validateApiCallPayload('get_installed_versions', { app_id: 'comfyui' }), {
    method: 'get_installed_versions',
    params: { app_id: 'comfyui' },
  });

  assert.throws(
    () => validateApiCallPayload('unknown_method', {}),
    /Unknown API method/
  );
  assert.throws(
    () => validateApiCallPayload('get_status', []),
    /Invalid API params payload/
  );
  assert.throws(
    () => validateApiCallPayload('get_status', { injected: true }),
    /Unexpected API params/
  );
});

test('sanitizeOpenDialogOptions keeps only allowed dialog fields', () => {
  const options = sanitizeOpenDialogOptions({
    title: 'Pick a model',
    defaultPath: '/models',
    buttonLabel: 'Choose',
    message: 'Select a model file.',
    securityScopedBookmarks: true,
    properties: ['openFile', 'multiSelections', 'createDirectory', 'badProperty'],
    filters: [
      { name: 'Models', extensions: ['gguf', 'safetensors', 42] },
      { name: 'Empty', extensions: [] },
      { name: 123, extensions: ['zip'] },
    ],
  });

  assert.deepEqual(options, {
    title: 'Pick a model',
    defaultPath: '/models',
    buttonLabel: 'Choose',
    message: 'Select a model file.',
    properties: ['openFile', 'multiSelections', 'createDirectory'],
    filters: [
      { name: 'Models', extensions: ['gguf', 'safetensors'] },
    ],
  });
});

test('validateExternalUrl accepts only http and https URLs', () => {
  assert.equal(validateExternalUrl('https://example.com/path'), 'https://example.com/path');
  assert.throws(() => validateExternalUrl('file:///tmp/model.gguf'), /Only http\/https/);
  assert.throws(() => validateExternalUrl('javascript:alert(1)'), /Only http\/https/);
  assert.throws(() => validateExternalUrl(42), /Invalid URL payload/);
});
