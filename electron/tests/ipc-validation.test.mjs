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
  assert.ok(ALLOWED_RPC_METHODS.includes('get_serving_status'));
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

test('validateApiCallPayload enforces method request schemas', () => {
  assert.deepEqual(validateApiCallPayload('install_version', {
    tag: 'v1.2.3',
    app_id: 'comfyui',
  }), {
    method: 'install_version',
    params: {
      tag: 'v1.2.3',
      app_id: 'comfyui',
    },
  });
  assert.deepEqual(validateApiCallPayload('set_default_version', {
    tag: null,
    app_id: undefined,
  }), {
    method: 'set_default_version',
    params: {
      tag: null,
      app_id: undefined,
    },
  });
  assert.deepEqual(validateApiCallPayload('call_plugin_endpoint', {
    app_id: 'ollama',
    endpoint_name: 'loadModel',
    params: { model_name: 'llama3' },
  }), {
    method: 'call_plugin_endpoint',
    params: {
      app_id: 'ollama',
      endpoint_name: 'loadModel',
      params: { model_name: 'llama3' },
    },
  });
  assert.deepEqual(validateApiCallPayload('validate_model_serving_config', {
    request: {
      model_id: 'models/example',
      config: {
        provider: 'ollama',
        profile_id: 'ollama-default',
      },
    },
  }), {
    method: 'validate_model_serving_config',
    params: {
      request: {
        model_id: 'models/example',
        config: {
          provider: 'ollama',
          profile_id: 'ollama-default',
        },
      },
    },
  });

  assert.throws(
    () => validateApiCallPayload('install_version', { app_id: 'comfyui' }),
    /Missing required API param/
  );
  assert.throws(
    () => validateApiCallPayload('install_version', { tag: '' }),
    /Invalid API param/
  );
  assert.throws(
    () => validateApiCallPayload('get_installed_versions', { app_id: 42 }),
    /Invalid API param/
  );
  assert.throws(
    () => validateApiCallPayload('get_installed_versions', { app_id: null }),
    /Invalid API param/
  );
  assert.throws(
    () => validateApiCallPayload('call_plugin_endpoint', {
      app_id: 'ollama',
      endpoint_name: 'loadModel',
      params: { limit: 10 },
    }),
    /Invalid API param/
  );
  assert.throws(
    () => validateApiCallPayload('get_installed_versions', {
      app_id: 'comfyui',
      extra: true,
    }),
    /Unexpected API param/
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
