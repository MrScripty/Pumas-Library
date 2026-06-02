import { describe, expect, it } from 'vitest';
import type { RuntimeProfileStatus } from '../types/api-runtime-profiles';
import type { ServedModelStatus, ServingEndpointStatus } from '../types/api-serving';
import { getLlamaCppConnectionUrl } from './llamaCppConnectionUrl';

const llamaCppProfiles = new Set(['llama-router']);

const loadedLlamaCppModel: ServedModelStatus = {
  model_id: 'llm/qwen',
  model_alias: 'qwen',
  provider: 'llama_cpp',
  profile_id: 'llama-router',
  load_state: 'loaded',
  device_mode: 'gpu',
  keep_loaded: true,
  endpoint_url: 'http://127.0.0.1:46325/',
};

describe('getLlamaCppConnectionUrl', () => {
  it('prefers the live Pumas gateway URL for llama.cpp connection info', () => {
    const endpoint: ServingEndpointStatus = {
      endpoint_mode: 'pumas_gateway',
      endpoint_url: 'http://127.0.0.1:45639/v1',
      model_count: 1,
    };

    expect(
      getLlamaCppConnectionUrl({
        servingEndpoint: endpoint,
        servedModels: [loadedLlamaCppModel],
        runtimeStatuses: [],
        llamaCppProfileIds: llamaCppProfiles,
      })
    ).toBe('http://127.0.0.1:45639/v1');
  });

  it('falls back to the active llama.cpp runtime endpoint when no gateway is available', () => {
    const statuses: RuntimeProfileStatus[] = [
      {
        profile_id: 'llama-router',
        state: 'running',
        endpoint_url: 'http://127.0.0.1:20617/',
      },
    ];

    expect(
      getLlamaCppConnectionUrl({
        servingEndpoint: { endpoint_mode: 'not_configured', model_count: 0 },
        servedModels: [],
        runtimeStatuses: statuses,
        llamaCppProfileIds: llamaCppProfiles,
      })
    ).toBe('http://127.0.0.1:20617/');
  });

  it('ignores the shared gateway when no llama.cpp model is loaded', () => {
    expect(
      getLlamaCppConnectionUrl({
        servingEndpoint: {
          endpoint_mode: 'pumas_gateway',
          endpoint_url: 'http://127.0.0.1:45639/v1',
          model_count: 1,
        },
        servedModels: [
          {
            ...loadedLlamaCppModel,
            provider: 'onnx_runtime',
            profile_id: 'onnx-runtime-default',
            endpoint_url: null,
          },
        ],
        runtimeStatuses: [],
        llamaCppProfileIds: llamaCppProfiles,
      })
    ).toBeUndefined();
  });

  it('falls back to a loaded llama.cpp provider endpoint if serving status lacks a gateway URL', () => {
    expect(
      getLlamaCppConnectionUrl({
        servingEndpoint: { endpoint_mode: 'pumas_gateway', model_count: 1 },
        servedModels: [loadedLlamaCppModel],
        runtimeStatuses: [],
        llamaCppProfileIds: llamaCppProfiles,
      })
    ).toBe('http://127.0.0.1:46325/');
  });
});
