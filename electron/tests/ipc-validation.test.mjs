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
  assert.ok(ALLOWED_RPC_METHODS.includes('serve_model'));
  assert.ok(ALLOWED_RPC_METHODS.includes('torch_configure'));
});

test('backend setup IPC validates exact backend and retry identity before forwarding', () => {
  const token = '2e038924-e0e3-4266-95ef-f7a02997b7b6';
  for (const backend of ['python_conversion', 'llama_cpp', 'nvfp4', 'sherry']) {
    for (const params of [{backend}, {backend, expected_previous_operation_id:null}, {backend, expected_previous_operation_id:token}]) {
      const decoded = validateApiCallPayload('start_backend_setup', params);
      assert.deepEqual(JSON.parse(JSON.stringify(decoded.params)), params);
      assert.notEqual(decoded.params, params);
      assert.ok(Object.isFrozen(decoded.params));
    }
    assert.equal(validateApiCallPayload('get_backend_setup', {backend}).params.backend, backend);
    assert.throws(() => validateApiCallPayload('get_backend_setup', {backend, expected_previous_operation_id:token}));
  }
  for (const method of ['start_backend_setup', 'get_backend_setup']) {
    for (const params of [undefined, null, {}, [], {backend:null}, {backend:42}, {backend:'LlamaCpp'}, {backend:'unknown'}, {backend:'llama_cpp', force:true}]) {
      assert.throws(() => validateApiCallPayload(method, params), /Invalid API params/);
    }
  }
  for (const params of [
    {backend:'llama_cpp', expectedPreviousOperationId:token},
    ...['', token.toUpperCase(), `${token}\n`, 42, false].map(expected_previous_operation_id => ({backend:'llama_cpp', expected_previous_operation_id})),
  ]) assert.throws(() => validateApiCallPayload('start_backend_setup', params), /Invalid API params/);
});

test('partial recovery admits model tickets and rejects retired recovery requests', () => {
  const params = { modelId: 'llm/example', recoveryToken: `v1:${'a'.repeat(64)}` };
  assert.deepEqual(validateApiCallPayload('resume_partial_download', params), {
    method: 'resume_partial_download', params,
  });
  assert.throws(() => validateApiCallPayload('resume_partial_download', {
    repo_id: 'org/example', dest_dir: '/models/example',
  }));
  for (const method of ['recover_download', 'list_interrupted_downloads']) {
    assert.throws(() => validateApiCallPayload(method, {}), /Unknown API method/);
  }
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
  assert.deepEqual(validateApiCallPayload('get_installed_versions', { app_id: 'ollama' }), {
    method: 'get_installed_versions',
    params: { app_id: 'ollama' },
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

test('default selection IPC uses the generated request contract', () => {
  for (const appKey of ['app_id', 'appId']) {
    for (const appId of ['', ' runtime λ ']) {
      for (const params of [
        { [appKey]: appId },
        { [appKey]: appId, tag: null },
        { [appKey]: appId, tag: '' },
        { [appKey]: appId, tag: ' \n\t ' },
        { [appKey]: appId, tag: ' vλ.1 ' },
      ]) {
        const decoded = validateApiCallPayload('set_default_version', params);
        assert.deepEqual(JSON.parse(JSON.stringify(decoded.params)), params);
        assert.notEqual(decoded.params, params);
        assert.ok(Object.isFrozen(decoded.params));
      }
    }
  }

  for (const params of [
    undefined,
    null,
    {},
    [],
    true,
    42,
    'runtime',
    { tag: 'v1' },
    { app_id: null },
    { app_id: 42 },
    { app_id: 'runtime', tag: true },
    { app_id: 'runtime', tag: 42 },
    { app_id: 'runtime', tag: [] },
    { app_id: 'runtime', tag: {} },
    { app_id: 'runtime', extra: true },
    { app_id: 'a', appId: 'a' },
  ]) {
    assert.throws(
      () => validateApiCallPayload('set_default_version', params),
      /Invalid API params/
    );
  }
});

test('installation-start IPC uses the generated request contract', () => {
  for (const appKey of ['app_id', 'appId']) {
    for (const value of ['', ' \n\t ', ' runtime λ ']) {
      const params = { [appKey]: value, tag: value };
      const decoded = validateApiCallPayload('install_version', params);
      assert.deepEqual(JSON.parse(JSON.stringify(decoded.params)), params);
      assert.notEqual(decoded.params, params);
      assert.ok(Object.isFrozen(decoded.params));
    }
  }
  for (const params of [undefined, null, {}, [], true, 42, 'runtime', { tag: 'v1' },
    { app_id: 'runtime' }, { app_id: null, tag: 'v1' }, { app_id: 42, tag: 'v1' },
    { app_id: 'runtime', tag: null }, { app_id: 'runtime', tag: true },
    { app_id: 'runtime', tag: [] }, { app_id: 'runtime', tag: {} },
    { app_id: 'runtime', tag: 'v1', extra: true },
    { app_id: 'a', appId: 'a', tag: 'v1' }]) {
    assert.throws(() => validateApiCallPayload('install_version', params), /Invalid API params/);
  }
});

test('dependency-check IPC uses the generated request contract with a required app id', () => {
  for (const appKey of ['app_id', 'appId']) {
    for (const value of ['', ' \n\t ', ' runtime λ ']) {
      const params = { [appKey]: value, tag: value };
      const decoded = validateApiCallPayload('check_version_dependencies', params);
      assert.deepEqual(JSON.parse(JSON.stringify(decoded.params)), params);
      assert.notEqual(decoded.params, params);
      assert.ok(Object.isFrozen(decoded.params));
    }
  }
  for (const params of [undefined, null, {}, [], true, 42, 'runtime',
    { tag: 'v1' }, { app_id: null, tag: 'v1' }, { app_id: 42, tag: 'v1' },
    { app_id: 'runtime' }, { app_id: 'runtime', tag: null },
    { app_id: 'runtime', tag: true }, { app_id: 'runtime', tag: [] },
    { app_id: 'runtime', tag: {} }, { app_id: 'runtime', tag: 'v1', extra: true },
    { app_id: 'a', appId: 'a', tag: 'v1' }]) {
    assert.throws(
      () => validateApiCallPayload('check_version_dependencies', params),
      /Invalid API params/
    );
  }
});

test('release-dependency IPC uses the generated request contract with a required app id', () => {
  for (const appKey of ['app_id', 'appId']) {
    for (const value of ['', ' \n\t ', ' runtime λ ']) {
      const params = { [appKey]: value, tag: value };
      const decoded = validateApiCallPayload('get_release_dependencies', params);
      assert.deepEqual(JSON.parse(JSON.stringify(decoded.params)), params);
      assert.notEqual(decoded.params, params);
      assert.ok(Object.isFrozen(decoded.params));
    }
  }
  for (const params of [undefined, null, {}, [], true, 42, 'runtime',
    { tag: 'v1' }, { app_id: null, tag: 'v1' }, { app_id: 42, tag: 'v1' },
    { app_id: 'runtime' }, { app_id: 'runtime', tag: null },
    { app_id: 'runtime', tag: true }, { app_id: 'runtime', tag: [] },
    { app_id: 'runtime', tag: {} }, { app_id: 'runtime', tag: 'v1', top_n: 5 },
    { app_id: 'runtime', tag: 'v1', extra: true },
    { app_id: 'a', appId: 'a', tag: 'v1' }]) {
    assert.throws(
      () => validateApiCallPayload('get_release_dependencies', params),
      /Invalid API params/
    );
  }
});

test('validateApiCallPayload enforces method request schemas', () => {
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
  assert.deepEqual(validateApiCallPayload('serve_model', {
    request: {
      model_id: 'models/example',
      config: {
        provider: 'ollama',
        profile_id: 'ollama-default',
      },
    },
  }), {
    method: 'serve_model',
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
      app_id: 'ollama',
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
