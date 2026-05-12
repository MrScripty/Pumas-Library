import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ModelManagerProps } from '../ModelManager';
import { OnnxRuntimePanel } from './OnnxRuntimePanel';

const modelManagerMock = vi.hoisted(() => vi.fn());

vi.mock('../ModelManager', () => ({
  ModelManager: (props: ModelManagerProps) => {
    modelManagerMock(props);
    return <div>model-manager</div>;
  },
}));

vi.mock('./sections/RuntimeProfileSettingsSection', () => ({
  RuntimeProfileSettingsSection: ({ provider }: { provider: string }) => (
    <div>{`profiles-${provider}`}</div>
  ),
}));

describe('OnnxRuntimePanel', () => {
  it('renders ONNX profile settings and filters model groups to ONNX artifacts', () => {
    const modelManagerProps: ModelManagerProps = {
      excludedModels: new Set(),
      modelGroups: [
        {
          category: 'embeddings',
          models: [
            { id: 'onnx-primary', name: 'ONNX Primary', category: 'embeddings', primaryFormat: 'onnx' },
            { id: 'onnx-path', name: 'ONNX Path', category: 'embeddings', path: '/models/model.onnx' },
            { id: 'gguf', name: 'GGUF', category: 'embeddings', primaryFormat: 'gguf' },
          ],
        },
      ],
      onToggleLink: vi.fn(),
      onToggleStar: vi.fn(),
      selectedAppId: 'onnx-runtime',
      starredModels: new Set(),
    };

    render(<OnnxRuntimePanel modelManagerProps={modelManagerProps} />);

    expect(screen.getByText('profiles-onnx_runtime')).toBeInTheDocument();
    expect(modelManagerMock).toHaveBeenCalledWith(
      expect.objectContaining({
        modelGroups: [
          {
            category: 'embeddings',
            models: [
              expect.objectContaining({ id: 'onnx-primary' }),
              expect.objectContaining({ id: 'onnx-path' }),
            ],
          },
        ],
      })
    );
  });
});
