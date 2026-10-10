import { StrictMode } from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { APIError } from '../errors';
import {
  LauncherRootRecoveryProvider,
  useLauncherRootRecovery,
} from './useLauncherRootRecovery';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

let presentationTimeoutHandler: (() => void) | undefined;
let presentationTimeoutListeners = 0;
let maximumPresentationTimeoutListeners = 0;

function installElectronBridge(
  getRootState: () => Promise<unknown>,
  selectRoot: () => Promise<unknown> = async () => ({ status: 'cancelled' })
): NonNullable<typeof window.electronAPI> {
  const bridge = {
    get_launcher_root_bootstrap: vi.fn(() => ({ status: 'initializing' })),
    get_launcher_root_state: vi.fn(getRootState),
    select_launcher_root: vi.fn(selectRoot),
    notify_launcher_root_presentation_committed: vi.fn().mockResolvedValue(undefined),
    onLauncherRootPresentationTimeout: vi.fn((callback: () => void) => {
      presentationTimeoutHandler = callback;
      presentationTimeoutListeners += 1;
      maximumPresentationTimeoutListeners = Math.max(
        maximumPresentationTimeoutListeners,
        presentationTimeoutListeners
      );
      return () => {
        if (presentationTimeoutHandler === callback) {
          presentationTimeoutHandler = undefined;
        }
        presentationTimeoutListeners -= 1;
      };
    }),
    minimizeWindow: vi.fn().mockResolvedValue(undefined),
    close_window: vi.fn().mockResolvedValue({ success: true }),
  } as unknown as NonNullable<typeof window.electronAPI>;
  window.electronAPI = bridge;
  return bridge;
}

function LibraryContent() {
  const { chooseLibraryRoot } = useLauncherRootRecovery();
  return (
    <button type="button" onClick={() => { void chooseLibraryRoot(); }}>
      Change Library
    </button>
  );
}

function DoubleChooseContent() {
  const { chooseLibraryRoot } = useLauncherRootRecovery();
  return (
    <button
      type="button"
      onClick={() => {
        void chooseLibraryRoot();
        void chooseLibraryRoot();
      }}
    >
      Change Library Twice
    </button>
  );
}

describe('LauncherRootRecoveryProvider', () => {
  const browserUserAgent = window.navigator.userAgent;

  beforeEach(() => {
    window.electronAPI = undefined;
    presentationTimeoutHandler = undefined;
    presentationTimeoutListeners = 0;
    maximumPresentationTimeoutListeners = 0;
    Object.defineProperty(window.navigator, 'userAgent', {
      configurable: true,
      value: browserUserAgent,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it.each([
    ['migration-required', 'Library upgrade required', /older download database/],
    ['backend-unavailable', 'Library could not start', /Another app may already be using it/],
  ] as const)('keeps %s visible and permits choosing another library', async (reason, title, message) => {
    const bridge = installElectronBridge(async () => ({
      status: 'recovery-required', reason, authoritySource: 'persisted', action: 'select-library',
    }));
    render(<LauncherRootRecoveryProvider><div>Library content</div></LauncherRootRecoveryProvider>);
    expect(await screen.findByRole('heading', { name: title })).toBeVisible();
    expect(screen.getByText(message)).toBeVisible();
    expect(screen.queryByText('Library content')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Select Library' }));
    await waitFor(() => expect(bridge.select_launcher_root).toHaveBeenCalledTimes(1));
    expect(screen.getByRole('heading', { name: title })).toBeVisible();
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledWith('recovery-required');
  });

  it.each(['environment', 'argument'] as const)('preserves %s authority after backend failure', async (authoritySource) => {
    const bridge = installElectronBridge(async () => ({
      status: 'recovery-required', reason: 'backend-unavailable', authoritySource, action: 'correct-launch-input',
    }));
    render(<LauncherRootRecoveryProvider><div>Library content</div></LauncherRootRecoveryProvider>);
    expect(await screen.findByRole('heading', { name: 'Library could not start' })).toBeVisible();
    expect(screen.getByRole('status')).toHaveTextContent(`correct the ${authoritySource} launch input`);
    expect(screen.queryByRole('button', { name: 'Select Library' })).not.toBeInTheDocument();
    expect(bridge.select_launcher_root).not.toHaveBeenCalled();
    expect(screen.queryByText('Library content')).not.toBeInTheDocument();
  });

  it('treats browser mode as not applicable and renders application content', () => {
    render(
      <LauncherRootRecoveryProvider>
        <div>Library content</div>
      </LauncherRootRecoveryProvider>
    );

    expect(screen.getByText('Library content')).toBeVisible();
    expect(screen.queryByText(/checking library/i)).not.toBeInTheDocument();
  });

  it('mounts library content in the first render when the synchronous bootstrap is ready', () => {
    const bridge = installElectronBridge(async () => ({ status: 'initializing' }));
    vi.mocked(bridge.get_launcher_root_bootstrap).mockReturnValue({
      status: 'ready', selectionAction: 'select-library', libraryScopeId: `display-v1:${'a'.repeat(64)}`,
    });
    render(<LauncherRootRecoveryProvider><div>First-frame library</div></LauncherRootRecoveryProvider>);
    expect(screen.getByText('First-frame library')).toBeVisible();
    expect(bridge.get_launcher_root_state).not.toHaveBeenCalled();
  });

  it('fails closed when an Electron renderer has no preload bridge', () => {
    Object.defineProperty(window.navigator, 'userAgent', {
      configurable: true,
      value: `${browserUserAgent} Electron/39.8.6`,
    });
    const windowCloseSpy = vi.spyOn(window, 'close').mockImplementation(() => undefined);

    render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    expect(screen.getByRole('heading', { name: 'Desktop bridge unavailable' })).toBeVisible();
    expect(screen.queryByText('Backend-consuming application')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Minimize' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(windowCloseSpy).toHaveBeenCalledTimes(1);
  });

  it('waits for the immediate Electron root-state result and atomically replaces checking', async () => {
    const rootState = deferred<unknown>();
    const bridge = installElectronBridge(() => rootState.promise);
    let contentWasCommittedAtNotification = false;
    vi.mocked(bridge.notify_launcher_root_presentation_committed).mockImplementation(
      async () => {
        contentWasCommittedAtNotification = screen.queryByText(
          'Backend-consuming application'
        ) !== null;
      }
    );

    render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    expect(screen.getByRole('status')).toHaveTextContent('Checking library');
    expect(screen.queryByText('Backend-consuming application')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Minimize' })).toBeVisible();
    expect(screen.getByRole('button', { name: 'Close' })).toBeVisible();
    expect(bridge.get_launcher_root_state).toHaveBeenCalledTimes(1);
    expect(bridge.notify_launcher_root_presentation_committed).not.toHaveBeenCalled();

    await act(async () => {
      rootState.resolve({ status: 'ready', selectionAction: 'select-library', libraryScopeId: null });
      await rootState.promise;
    });

    expect(await screen.findByText('Backend-consuming application')).toBeVisible();
    expect(screen.queryByText(/checking library/i)).not.toBeInTheDocument();
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(1);
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledWith('ready');
    expect(contentWasCommittedAtNotification).toBe(true);
  });

  it('offers persisted startup recovery and restores it when selection is cancelled', async () => {
    const bridge = installElectronBridge(async () => ({
      status: 'recovery-required',
      reason: 'invalid',
      authoritySource: 'persisted',
      action: 'select-library',
    }));

    render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Select Library' }));

    await waitFor(() => {
      expect(bridge.select_launcher_root).toHaveBeenCalledTimes(1);
    });
    expect(await screen.findByRole('button', { name: 'Select Library' })).toBeVisible();
    expect(screen.queryByText('Backend-consuming application')).not.toBeInTheDocument();
    await waitFor(() => {
      expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledWith(
        'recovery-required'
      );
    });
  });

  it('explains explicit startup recovery without offering or invoking selection', async () => {
    const bridge = installElectronBridge(async () => ({
      status: 'recovery-required',
      reason: 'unavailable',
      authoritySource: 'environment',
      action: 'correct-launch-input',
    }));

    render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    expect(await screen.findByRole('heading', { name: 'Correct launcher input' })).toBeVisible();
    expect(screen.getByRole('status')).toHaveTextContent('environment');
    expect(screen.queryByRole('button', { name: 'Select Library' })).not.toBeInTheDocument();
    expect(bridge.select_launcher_root).not.toHaveBeenCalled();
    expect(screen.queryByText('Backend-consuming application')).not.toBeInTheDocument();
  });

  it('turns a ready explicit-authority change request into dismissible guidance', async () => {
    const bridge = installElectronBridge(async () => ({
      status: 'ready',
      libraryScopeId: null,
      selectionAction: 'correct-launch-input',
    }));

    render(
      <LauncherRootRecoveryProvider>
        <LibraryContent />
      </LauncherRootRecoveryProvider>
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Change Library' }));

    expect(await screen.findByRole('heading', { name: 'Correct launcher input' })).toBeVisible();
    expect(bridge.select_launcher_root).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Back to Library' }));
    expect(await screen.findByRole('button', { name: 'Change Library' })).toBeVisible();
    await waitFor(() => {
      expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(1);
    });
  });

  it('shares the immediate root-state read across the StrictMode effect replay', async () => {
    const rootState = deferred<unknown>();
    const bridge = installElectronBridge(() => rootState.promise);

    render(
      <StrictMode>
        <LauncherRootRecoveryProvider>
          <div>Backend-consuming application</div>
        </LauncherRootRecoveryProvider>
      </StrictMode>
    );

    expect(bridge.get_launcher_root_state).toHaveBeenCalledTimes(1);

    await act(async () => {
      rootState.resolve({ status: 'ready', selectionAction: 'select-library', libraryScopeId: null });
      await rootState.promise;
    });

    expect(await screen.findByText('Backend-consuming application')).toBeVisible();
    await waitFor(() => {
      expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(1);
    });
    expect(maximumPresentationTimeoutListeners).toBe(1);
  });

  it('polls one initializing startup lifecycle sequentially until it becomes ready', async () => {
    vi.useFakeTimers();
    const states = [
      { status: 'initializing' },
      { status: 'initializing' },
      { status: 'ready', selectionAction: 'select-library', libraryScopeId: null },
    ];
    const bridge = installElectronBridge(async () => states.shift());

    render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    await act(async () => undefined);
    expect(bridge.get_launcher_root_state).toHaveBeenCalledTimes(1);
    expect(bridge.notify_launcher_root_presentation_committed).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });

    expect(bridge.get_launcher_root_state).toHaveBeenCalledTimes(3);
    expect(screen.getByText('Backend-consuming application')).toBeVisible();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(50);
    });
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(1);
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledWith('ready');
  });

  it('keeps initializing hidden from the main-process visibility owner', async () => {
    vi.useFakeTimers();
    const bridge = installElectronBridge(async () => ({ status: 'initializing' }));

    render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_000);
    });

    expect(bridge.get_launcher_root_state).toHaveBeenCalledTimes(11);
    expect(bridge.notify_launcher_root_presentation_committed).not.toHaveBeenCalled();
    expect(screen.getByRole('heading', { name: 'Checking library' })).toBeVisible();
    expect(screen.queryByText('Backend-consuming application')).not.toBeInTheDocument();
  });

  it('latches the main-process timeout before ignoring a late ready result', async () => {
    const rootState = deferred<unknown>();
    const bridge = installElectronBridge(() => rootState.promise);

    render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    expect(presentationTimeoutHandler).toBeTypeOf('function');
    act(() => {
      presentationTimeoutHandler?.();
    });

    expect(bridge.get_launcher_root_state).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('heading', { name: 'Desktop bridge unavailable' })).toBeVisible();
    await waitFor(() => {
      expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(1);
      expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledWith(
        'bridge-unavailable'
      );
    });

    await act(async () => {
      rootState.resolve({ status: 'ready', selectionAction: 'select-library', libraryScopeId: null });
      await rootState.promise;
    });

    expect(screen.getByRole('heading', { name: 'Desktop bridge unavailable' })).toBeVisible();
    expect(screen.queryByText('Backend-consuming application')).not.toBeInTheDocument();
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(1);
  });

  it('supersedes a committed ready presentation when the main timeout wins', async () => {
    const bridge = installElectronBridge(async () => ({
      status: 'ready',
      libraryScopeId: null,
      selectionAction: 'select-library',
    }));

    render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    expect(await screen.findByText('Backend-consuming application')).toBeVisible();
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(1);
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenLastCalledWith('ready');

    act(() => {
      presentationTimeoutHandler?.();
    });
    expect(screen.getByRole('heading', { name: 'Desktop bridge unavailable' })).toBeVisible();
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(2);
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenLastCalledWith(
      'bridge-unavailable'
    );
  });

  it('supersedes an in-flight ready acknowledgement when the main timeout wins', async () => {
    const readyAcknowledgement = deferred<undefined>();
    const bridge = installElectronBridge(async () => ({
      status: 'ready',
      libraryScopeId: null,
      selectionAction: 'select-library',
    }));
    vi.mocked(bridge.notify_launcher_root_presentation_committed)
      .mockImplementationOnce(() => readyAcknowledgement.promise)
      .mockResolvedValueOnce(undefined);

    render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    expect(await screen.findByText('Backend-consuming application')).toBeVisible();
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(1);
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenLastCalledWith('ready');

    act(() => {
      presentationTimeoutHandler?.();
    });
    expect(screen.getByRole('heading', { name: 'Desktop bridge unavailable' })).toBeVisible();

    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(2);
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenLastCalledWith(
      'bridge-unavailable'
    );
    await act(async () => {
      readyAcknowledgement.resolve(undefined);
      await readyAcknowledgement.promise;
    });
    expect(screen.getByRole('heading', { name: 'Desktop bridge unavailable' })).toBeVisible();
    expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(2);
  });

  it('suppresses a late presentation result after the provider unmounts', async () => {
    const rootState = deferred<unknown>();
    const bridge = installElectronBridge(() => rootState.promise);
    const rendered = render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    expect(screen.getByRole('heading', { name: 'Checking library' })).toBeVisible();
    rendered.unmount();
    await act(async () => {
      rootState.resolve({ status: 'ready', selectionAction: 'select-library', libraryScopeId: null });
      await rootState.promise;
    });

    expect(bridge.notify_launcher_root_presentation_committed).not.toHaveBeenCalled();
  });

  it('turns a rejected root-state invocation into path-free terminal UI', async () => {
    const bridge = installElectronBridge(async () => {
      throw new APIError('/private/library/root leaked from the bridge', 'get_launcher_root_state');
    });

    render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    expect(await screen.findByRole('heading', { name: 'Desktop bridge unavailable' })).toBeVisible();
    expect(screen.queryByText(/private\/library/)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /retry/i })).not.toBeInTheDocument();
    await waitFor(() => {
      expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledWith(
        'bridge-unavailable'
      );
    });
  });

  it('leaves the committed state stable when the visibility acknowledgement rejects', async () => {
    const bridge = installElectronBridge(async () => ({
      status: 'ready',
      libraryScopeId: null,
      selectionAction: 'select-library',
    }));
    vi.mocked(bridge.notify_launcher_root_presentation_committed).mockRejectedValueOnce(
      new Error('main visibility owner unavailable')
    );

    render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    expect(await screen.findByText('Backend-consuming application')).toBeVisible();
    await waitFor(() => {
      expect(bridge.notify_launcher_root_presentation_committed).toHaveBeenCalledTimes(1);
    });
    expect(screen.queryByRole('heading', { name: 'Desktop bridge unavailable' }))
      .not.toBeInTheDocument();
  });

  it('cleans up initializing polling when its renderer owner unmounts', async () => {
    vi.useFakeTimers();
    const bridge = installElectronBridge(async () => ({ status: 'initializing' }));
    const rendered = render(
      <LauncherRootRecoveryProvider>
        <div>Backend-consuming application</div>
      </LauncherRootRecoveryProvider>
    );

    await act(async () => undefined);
    rendered.unmount();
    await vi.advanceTimersByTimeAsync(1_000);

    expect(bridge.get_launcher_root_state).toHaveBeenCalledTimes(1);
    expect(presentationTimeoutListeners).toBe(0);
  });

  it.each([
    [
      'invalid selection',
      {
        status: 'recovery-required',
        reason: 'invalid-selection',
        authorityState: 'unchanged',
      },
      'Selected library is not valid',
    ],
    [
      'unavailable chooser',
      {
        status: 'recovery-required',
        reason: 'chooser-unavailable',
        authorityState: 'unchanged',
      },
      'Library chooser unavailable',
    ],
    [
      'unchanged persistence',
      {
        status: 'recovery-required',
        reason: 'persistence-unavailable',
        authorityState: 'unchanged',
      },
      'Library change was not saved',
    ],
  ])('shows %s as retryable and lets a ready user return to content', async (
    _label,
    selectionResult,
    expectedTitle
  ) => {
    const bridge = installElectronBridge(
      async () => ({ status: 'ready', selectionAction: 'select-library', libraryScopeId: null }),
      async () => selectionResult
    );

    render(
      <LauncherRootRecoveryProvider>
        <LibraryContent />
      </LauncherRootRecoveryProvider>
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Change Library' }));

    expect(await screen.findByRole('heading', { name: expectedTitle })).toBeVisible();
    expect(screen.getByRole('button', { name: 'Try Again' })).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: 'Back to Library' }));
    expect(await screen.findByRole('button', { name: 'Change Library' })).toBeVisible();
    expect(bridge.select_launcher_root).toHaveBeenCalledTimes(1);
  });

  it('keeps startup recovery blocking while allowing retry after an unchanged result', async () => {
    const results = [
      {
        status: 'recovery-required',
        reason: 'invalid-selection',
        authorityState: 'unchanged',
      },
      { status: 'cancelled' },
    ];
    const bridge = installElectronBridge(
      async () => ({
        status: 'recovery-required',
        reason: 'invalid',
        authoritySource: 'persisted',
        action: 'select-library',
      }),
      async () => results.shift()
    );

    render(
      <LauncherRootRecoveryProvider>
        <LibraryContent />
      </LauncherRootRecoveryProvider>
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Select Library' }));
    expect(await screen.findByRole('heading', { name: 'Selected library is not valid' })).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Back to Library' })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Try Again' }));
    expect(await screen.findByRole('heading', { name: 'Selected library is not valid' })).toBeVisible();
    expect(bridge.select_launcher_root).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole('button', { name: 'Back to Library' })).not.toBeInTheDocument();
  });

  it.each([
    [
      'ambiguous replacement',
      {
        status: 'recovery-required',
        reason: 'persistence-unavailable',
        authorityState: 'replacement-visibility-unknown',
      },
      'Library change needs confirmation',
    ],
    [
      'published durability failure',
      {
        status: 'recovery-required',
        reason: 'persistence-unavailable',
        authorityState: 'published-durability-unavailable',
      },
      'Library save may be incomplete',
    ],
    [
      'published restart failure',
      {
        status: 'recovery-required',
        reason: 'restart-unavailable',
        authorityState: 'published',
      },
      'Restart required',
    ],
    ['accepted restart', { status: 'restarting' }, 'Restarting Pumas Library'],
  ])('blocks content and retry after %s', async (_label, selectionResult, expectedTitle) => {
    installElectronBridge(
      async () => ({ status: 'ready', selectionAction: 'select-library', libraryScopeId: null }),
      async () => selectionResult
    );

    render(
      <LauncherRootRecoveryProvider>
        <LibraryContent />
      </LauncherRootRecoveryProvider>
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Change Library' }));

    expect(await screen.findByRole('heading', { name: expectedTitle })).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Try Again' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Back to Library' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Change Library' })).not.toBeInTheDocument();
  });

  it('turns a rejected selection invocation into path-free non-retryable UI', async () => {
    installElectronBridge(
      async () => ({ status: 'ready', selectionAction: 'select-library', libraryScopeId: null }),
      async () => {
        throw new APIError('/private/library/root leaked from selection', 'select_launcher_root');
      }
    );

    render(
      <LauncherRootRecoveryProvider>
        <LibraryContent />
      </LauncherRootRecoveryProvider>
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Change Library' }));

    expect(await screen.findByRole('heading', { name: 'Desktop bridge unavailable' })).toBeVisible();
    expect(screen.queryByText(/private\/library/)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /try again|back to library/i })).not.toBeInTheDocument();
  });

  it('shares one renderer selection attempt across overlapping action calls', async () => {
    const selection = deferred<unknown>();
    const bridge = installElectronBridge(
      async () => ({ status: 'ready', selectionAction: 'select-library', libraryScopeId: null }),
      () => selection.promise
    );

    render(
      <LauncherRootRecoveryProvider>
        <DoubleChooseContent />
      </LauncherRootRecoveryProvider>
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Change Library Twice' }));
    expect(bridge.select_launcher_root).toHaveBeenCalledTimes(1);

    await act(async () => {
      selection.resolve({ status: 'cancelled' });
      await selection.promise;
    });

    expect(await screen.findByRole('button', { name: 'Change Library Twice' })).toBeVisible();
  });

  it('presents a producer not-selectable result as dismissible correction guidance', async () => {
    const bridge = installElectronBridge(
      async () => ({ status: 'ready', selectionAction: 'select-library', libraryScopeId: null }),
      async () => ({ status: 'not-selectable', action: 'correct-launch-input' })
    );

    render(
      <LauncherRootRecoveryProvider>
        <LibraryContent />
      </LauncherRootRecoveryProvider>
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Change Library' }));
    expect(await screen.findByRole('heading', { name: 'Correct launcher input' })).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Try Again' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Back to Library' }));

    expect(await screen.findByRole('button', { name: 'Change Library' })).toBeVisible();
    expect(bridge.select_launcher_root).toHaveBeenCalledTimes(1);
  });
});
