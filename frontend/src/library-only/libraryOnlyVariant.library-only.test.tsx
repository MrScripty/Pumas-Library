/**
 * Library-only variant: runs under vitest.library-only.config.ts, which
 * compiles __FEATURE_INFERENCE_PLUGINS__ out and resolves the same build-time
 * aliases as `vite build --mode library-only`.
 */
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

vi.mock('../api/adapter', () => ({
  api: {},
  getElectronAPI: () => null,
  isAPIAvailable: () => false,
}));

import App from '@app-entry';
import { RuntimeModelServeAction } from '@runtime-model-serve-action';
import { ModelRuntimeRouteEditor } from '@runtime-route-editor';
import { DEFAULT_APPS, getAppById } from '../config/apps';
import { LauncherRootRecoveryProvider } from '../hooks/useLauncherRootRecovery';
import { LocalModelInstalledActions } from '../components/LocalModelInstalledActions';
import { ModelMetadataModalTabs } from '../components/ModelMetadataModalTabs';
import type { LocalModelRowState } from '../components/LocalModelRowState';

const rowState: LocalModelRowState = {
  canPause: false,
  canRecoverPartial: false,
  canResume: false,
  canShowRelated: false,
  hasRetainedProgressRing: false,
  isActiveDownload: false,
  isDownloading: false,
  isExpanded: false,
  isLinked: true,
  isPartialDownload: false,
  isPaused: false,
  isQueued: false,
  isRecoveringPartial: false,
  isStarred: false,
  relatedModels: [],
  relatedStatus: 'idle',
  ringDegrees: 0,
};

describe('library-only renderer variant', () => {
  it('is compiled with inference plugins disabled', () => {
    expect(__FEATURE_INFERENCE_PLUGINS__).toBe(false);
    expect(DEFAULT_APPS).toEqual([]);
    expect(getAppById('ollama')).toBeUndefined();
    expect(getAppById('torch')).toBeUndefined();
  });

  it('mounts the library app without the inference plugin sidebar', () => {
    render(<LauncherRootRecoveryProvider><App /></LauncherRootRecoveryProvider>);

    expect(screen.queryByRole('toolbar', { name: 'Inference plugins' })).not.toBeInTheDocument();
    const alerts = screen.getAllByRole('alert').map((alert) => alert.textContent);
    expect(alerts).toContain('Library unavailable. The desktop backend is not available.');
    expect(alerts).toContain('Download list unavailable: The desktop backend is not available.');
  });

  it('replaces runtime routing and serving controls with empty components', () => {
    const model = { id: 'llm/example', name: 'Example', category: 'llm' };
    const { container: serveContainer } = render(
      <RuntimeModelServeAction model={model} rowState={rowState} onServeModel={vi.fn()} />
    );
    expect(serveContainer).toBeEmptyDOMElement();

    const { container: routeContainer } = render(
      <ModelRuntimeRouteEditor {...({} as Parameters<typeof ModelRuntimeRouteEditor>[0])} />
    );
    expect(routeContainer).toBeEmptyDOMElement();

    render(
      <LocalModelInstalledActions
        model={model}
        rowState={rowState}
        selectedAppId={null}
        onToggleLink={vi.fn()}
        onServeModel={vi.fn()}
      />
    );
    expect(screen.queryByRole('button', { name: 'Serve model' })).not.toBeInTheDocument();
  });

  it('omits the serving tab from model metadata', () => {
    render(
      <ModelMetadataModalTabs
        activeSource="stored"
        embeddedFileType={null}
        embeddedMetadata={null}
        storedMetadata={{}}
        onActiveSourceChange={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: /Stored/ })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Serving/ })).not.toBeInTheDocument();
  });
});
