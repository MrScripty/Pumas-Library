import { describe, expect, it } from 'vitest';
import { DEFAULT_APPS, getAppById } from './apps';

describe('DEFAULT_APPS', () => {
  it('registers ONNX Runtime as an in-process app without a connection URL', () => {
    const onnxRuntime = getAppById('onnx-runtime');

    expect(onnxRuntime).toBeDefined();
    expect(onnxRuntime?.displayName).toBe('ONNX Runtime');
    expect(onnxRuntime?.status).toBe('idle');
    expect(onnxRuntime?.iconState).toBe('offline');
    expect(onnxRuntime?.connectionUrl).toBeUndefined();
    expect(DEFAULT_APPS.some((app) => app.id === 'onnx-runtime')).toBe(true);
  });
});
