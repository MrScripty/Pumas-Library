import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ModelManagerProps } from '../ModelManager';
import { OnnxRuntimePanel } from './OnnxRuntimePanel';

const onnxRuntimeModelLibrarySectionMock = vi.hoisted(() => vi.fn());

vi.mock('./sections/OnnxRuntimeModelLibrarySection', () => ({
  OnnxRuntimeModelLibrarySection: (props: unknown) => {
    onnxRuntimeModelLibrarySectionMock(props);
    return <div>onnx-runtime-library</div>;
  },
}));

vi.mock('./sections/RuntimeProfileSettingsSection', () => ({
  RuntimeProfileSettingsSection: ({ provider }: { provider: string }) => (
    <div>{`profiles-${provider}`}</div>
  ),
}));

describe('OnnxRuntimePanel', () => {
  it('renders ONNX profile settings and delegates model groups to the ONNX library section', () => {
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
    expect(screen.getByText('onnx-runtime-library')).toBeInTheDocument();
    expect(onnxRuntimeModelLibrarySectionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        modelGroups: modelManagerProps.modelGroups,
        excludedModels: modelManagerProps.excludedModels,
        starredModels: modelManagerProps.starredModels,
        onToggleLink: modelManagerProps.onToggleLink,
        onToggleStar: modelManagerProps.onToggleStar,
      })
    );
  });
});
