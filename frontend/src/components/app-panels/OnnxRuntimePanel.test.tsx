import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ModelManagerProps } from '../ModelManager';
import { UNSUPPORTED_VERSION_STATE, type AppVersionState } from '../../utils/appVersionState';
import { OnnxRuntimePanel } from './OnnxRuntimePanel';

const onnxRuntimeModelLibrarySectionMock = vi.hoisted(() => vi.fn());

vi.mock('./sections/OnnxRuntimeModelLibrarySection', () => ({
  OnnxRuntimeModelLibrarySection: (props: unknown) => {
    onnxRuntimeModelLibrarySectionMock(props);
    return <div>onnx-runtime-library</div>;
  },
}));

vi.mock('./VersionManagementPanel', () => ({
  VersionManagementPanel: ({ appDisplayName, showManager }: {
    appDisplayName: string;
    showManager: boolean;
  }) => <div>{`versions-${appDisplayName}-${showManager ? 'open' : 'closed'}`}</div>,
}));

vi.mock('./sections/RuntimeProfileSettingsSection', () => ({
  RuntimeProfileSettingsSection: ({ provider }: { provider: string }) => (
    <div>{`profiles-${provider}`}</div>
  ),
}));

describe('OnnxRuntimePanel', () => {
  it('renders ONNX profile settings and delegates model groups to the ONNX library section', () => {
    const modelManagerProps: ModelManagerProps = {
      libraryLoadStatus: 'ready',
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
      servingEndpoint: {
        endpoint_mode: 'pumas_gateway',
        endpoint_url: 'http://127.0.0.1:3456/v1',
        model_count: 1,
      },
      servedModels: [
        {
          model_id: 'onnx-primary',
          provider: 'onnx_runtime',
          profile_id: 'onnx-cpu',
          load_state: 'loaded',
          device_mode: 'cpu',
          keep_loaded: true,
        },
      ],
      starredModels: new Set(),
    };

    render(
      <OnnxRuntimePanel
        appDisplayName="ONNX Runtime"
        versions={onnxRuntimeVersions}
        showVersionManager={false}
        onShowVersionManager={vi.fn()}
        diskSpacePercent={0}
        modelManagerProps={modelManagerProps}
      />
    );

    expect(screen.getByText('versions-ONNX Runtime-closed')).toBeInTheDocument();
    expect(screen.getByText('profiles-onnx_runtime')).toBeInTheDocument();
    expect(screen.getByText('onnx-runtime-library')).toBeInTheDocument();
    expect(onnxRuntimeModelLibrarySectionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        modelGroups: modelManagerProps.modelGroups,
        excludedModels: modelManagerProps.excludedModels,
        servingEndpoint: modelManagerProps.servingEndpoint,
        servedModels: modelManagerProps.servedModels,
        starredModels: modelManagerProps.starredModels,
        onToggleLink: modelManagerProps.onToggleLink,
        onToggleStar: modelManagerProps.onToggleStar,
      })
    );
  });
});

const onnxRuntimeVersions: AppVersionState = {
  ...UNSUPPORTED_VERSION_STATE,
  appId: 'onnx-runtime',
  isSupported: true,
  installedVersions: ['v1.24.2+cpu'],
  activeVersion: 'v1.24.2+cpu',
};

describe('OnnxRuntimePanel version manager', () => {
  it('shows only the version manager while it is open, like llama.cpp', () => {
    render(
      <OnnxRuntimePanel
        appDisplayName="ONNX Runtime"
        versions={onnxRuntimeVersions}
        showVersionManager
        onShowVersionManager={vi.fn()}
        diskSpacePercent={0}
        modelManagerProps={{
          libraryLoadStatus: 'ready',
          excludedModels: new Set(),
          modelGroups: [],
          onToggleLink: vi.fn(),
          onToggleStar: vi.fn(),
          selectedAppId: 'onnx-runtime',
          starredModels: new Set(),
        }}
      />
    );

    expect(screen.getByText('versions-ONNX Runtime-open')).toBeInTheDocument();
    expect(screen.queryByText('profiles-onnx_runtime')).not.toBeInTheDocument();
    expect(screen.queryByText('onnx-runtime-library')).not.toBeInTheDocument();
  });
});
