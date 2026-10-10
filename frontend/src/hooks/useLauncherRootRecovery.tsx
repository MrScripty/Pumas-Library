import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { getElectronAPI } from '../api/adapter';
import { LauncherRootRecoveryView } from '../components/LauncherRootRecoveryView';
import { ValidationError } from '../errors';
import type {
  LauncherRootSelectionResult,
  LauncherRootStartupState,
} from '../types/api-window';
import type { LauncherRootCommittedPresentation } from '../types/api-bridge-utilities';

export interface LauncherRootRecoveryActions {
  chooseLibraryRoot: () => Promise<void>;
}

const LauncherRootRecoveryContext = createContext<LauncherRootRecoveryActions | null>(null);
const LibraryScopeContext = createContext<string | null>(null);

/** Opaque display-cache scope, never filesystem or recovery authority. */
export function useLibraryScopeId(): string | null {
  return useContext(LibraryScopeContext);
}

type RecoveryRequiredStartupState = Extract<
  LauncherRootStartupState,
  { status: 'recovery-required' }
>;

type LauncherRootPresentation =
  | { kind: 'checking' }
  | { kind: 'bridge-unavailable' }
  | { kind: 'content' }
  | { kind: 'explicit-guidance' }
  | {
      kind: 'selection-result';
      result: Exclude<LauncherRootSelectionResult, { status: 'cancelled' }>;
      canReturnToLibrary: boolean;
    }
  | { kind: 'selecting'; prior: LauncherRootPresentation }
  | { kind: 'startup-recovery'; state: RecoveryRequiredStartupState };

const STARTUP_POLL_INTERVAL_MS = 100;

function isElectronRenderer(): boolean {
  return typeof navigator !== 'undefined' && /\bElectron\/\d/.test(navigator.userAgent);
}

export function LauncherRootRecoveryProvider({ children }: { children: ReactNode }) {
  const bridgeRef = useRef(getElectronAPI());
  const bridge = bridgeRef.current;
  const [bootstrap] = useState<LauncherRootStartupState | null>(() => {
    if (!bridge) return null;
    try {
      return bridge.get_launcher_root_bootstrap();
    } catch {
      return null;
    }
  });
  const mountedRef = useRef(true);
  const startupReadRef = useRef<Promise<LauncherRootStartupState> | null>(null);
  const selectionAttemptRef = useRef<Promise<void> | null>(null);
  const committedPresentationRef = useRef<LauncherRootCommittedPresentation | null>(null);
  const presentationCommitGenerationRef = useRef(0);
  const [presentation, setPresentation] = useState<LauncherRootPresentation>(
    bridge
      ? bootstrap?.status === 'ready'
        ? { kind: 'content' }
        : bootstrap?.status === 'recovery-required'
          ? { kind: 'startup-recovery', state: bootstrap }
          : bootstrap?.status === 'initializing'
            ? { kind: 'checking' }
            : { kind: 'bridge-unavailable' }
      : isElectronRenderer()
        ? { kind: 'bridge-unavailable' }
        : { kind: 'content' }
  );
  const [startupState, setStartupState] = useState<LauncherRootStartupState | null>(bootstrap);
  const [upgradePhase, setUpgradePhase] = useState<'idle' | 'confirm' | 'running' | 'failed' | 'upgraded'>('idle');
  const [oldWritersStopped, setOldWritersStopped] = useState(false);
  const [noDowngradeAccepted, setNoDowngradeAccepted] = useState(false);
  const upgradeAttemptRef = useRef<Promise<void> | null>(null);
  const [upgradeFailureStage, setUpgradeFailureStage] = useState<'upgrade' | 'open' | 'unknown'>('unknown');


  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    if (!bridge || !bootstrap) {
      return;
    }

    let disposed = false;
    let pollTimer: number | undefined;
    let terminal = bootstrap.status !== 'initializing';
    let visibilityTimedOut = false;

    const completeStartup = (nextPresentation: LauncherRootPresentation) => {
      if (disposed || terminal) {
        return;
      }
      terminal = true;
      if (pollTimer !== undefined) {
        window.clearTimeout(pollTimer);
      }
      setPresentation(nextPresentation);
    };

    const removeTimeoutListener = bridge.onLauncherRootPresentationTimeout(() => {
      if (disposed || visibilityTimedOut) {
        return;
      }
      visibilityTimedOut = true;
      terminal = true;
      if (pollTimer !== undefined) {
        window.clearTimeout(pollTimer);
      }
      presentationCommitGenerationRef.current += 1;
      committedPresentationRef.current = null;
      setPresentation({ kind: 'bridge-unavailable' });
    });

    const readStartupState = () => {
      if (disposed || terminal) {
        return;
      }

      let request = startupReadRef.current;
      if (!request) {
        request = bridge.get_launcher_root_state();
        startupReadRef.current = request;
        const clearRequest = () => {
          if (startupReadRef.current === request) {
            startupReadRef.current = null;
          }
        };
        void request.then(clearRequest, clearRequest);
      }

      void request.then(
        (state) => {
          if (disposed || terminal) {
            return;
          }
          setStartupState(state);
          if (state.status === 'ready') {
            completeStartup({ kind: 'content' });
          } else if (state.status === 'recovery-required') {
            completeStartup({ kind: 'startup-recovery', state });
          } else {
            pollTimer = window.setTimeout(readStartupState, STARTUP_POLL_INTERVAL_MS);
          }
        },
        () => {
          completeStartup({ kind: 'bridge-unavailable' });
        }
      );
    };

    readStartupState();

    return () => {
      disposed = true;
      if (pollTimer !== undefined) {
        window.clearTimeout(pollTimer);
      }
      removeTimeoutListener();
    };
  }, [bridge, bootstrap]);

  useLayoutEffect(() => {
    if (!bridge || committedPresentationRef.current) {
      return;
    }

    let committedPresentation: LauncherRootCommittedPresentation | undefined;
    if (presentation.kind === 'content') {
      committedPresentation = 'ready';
    } else if (presentation.kind === 'startup-recovery') {
      committedPresentation = 'recovery-required';
    } else if (presentation.kind === 'bridge-unavailable') {
      committedPresentation = 'bridge-unavailable';
    }

    if (!committedPresentation) {
      return;
    }

    const generation = presentationCommitGenerationRef.current + 1;
    presentationCommitGenerationRef.current = generation;
    committedPresentationRef.current = committedPresentation;
    void bridge.notify_launcher_root_presentation_committed(committedPresentation).catch(() => {
      // Main owns the bounded native failure path when the semantic commit cannot be observed.
    });

    return () => {
      if (presentationCommitGenerationRef.current === generation) {
        presentationCommitGenerationRef.current += 1;
      }
    };
  }, [bridge, presentation]);

  const chooseLibraryRoot = useCallback((): Promise<void> => {
    if (selectionAttemptRef.current) {
      return selectionAttemptRef.current;
    }
    if (!bridge || !startupState || startupState.status === 'initializing') {
      return Promise.resolve();
    }
    if (
      (startupState.status === 'ready' && startupState.selectionAction !== 'select-library') ||
      (startupState.status === 'recovery-required' && startupState.action !== 'select-library')
    ) {
      if (startupState.status === 'ready') {
        setPresentation({ kind: 'explicit-guidance' });
      }
      return Promise.resolve();
    }

    const prior = presentation;
    setPresentation({ kind: 'selecting', prior });
    const attempt = (async () => {
      let result: LauncherRootSelectionResult;
      try {
        result = await bridge.select_launcher_root();
      } catch {
        if (mountedRef.current) {
          setPresentation({ kind: 'bridge-unavailable' });
        }
        return;
      }
      if (!mountedRef.current) {
        return;
      }
      if (result.status === 'cancelled') {
        setPresentation(prior);
      } else {
        setPresentation({
          kind: 'selection-result',
          result,
          canReturnToLibrary: startupState.status === 'ready',
        });
      }
    })();
    selectionAttemptRef.current = attempt;
    const clearAttempt = () => {
      if (selectionAttemptRef.current === attempt) {
        selectionAttemptRef.current = null;
      }
    };
    void attempt.then(clearAttempt, clearAttempt);
    return attempt;
  }, [bridge, presentation, startupState]);

  const upgradeLibrary = useCallback((): Promise<void> => {
    if (upgradeAttemptRef.current) return upgradeAttemptRef.current;
    if (!bridge || !oldWritersStopped || !noDowngradeAccepted ||
        startupState?.status !== 'recovery-required' || startupState.reason !== 'migration-required') {
      return Promise.resolve();
    }
    setUpgradePhase('running');
    const attempt = (async () => {
      try {
        const result = await bridge.upgrade_launcher_library({ oldWritersStopped: true, noDowngradeAccepted: true });
        if (!mountedRef.current) return;
        if (result.status === 'ready') {
          setStartupState(result.state);
          setPresentation({ kind: 'content' });
          setUpgradePhase('idle');
        } else if (result.status === 'upgraded') {
          setUpgradePhase('upgraded');
        } else {
          setUpgradeFailureStage(result.status === 'failed' ? result.stage : 'unknown');
          setUpgradePhase('failed');
        }
      } catch {
        if (mountedRef.current) {
          setUpgradeFailureStage('unknown');
          setUpgradePhase('failed');
        }
      }
    })();
    upgradeAttemptRef.current = attempt;
    void attempt.finally(() => {
      if (upgradeAttemptRef.current === attempt) upgradeAttemptRef.current = null;
    });
    return attempt;
  }, [bridge, oldWritersStopped, noDowngradeAccepted, startupState]);

  const actions = useMemo<LauncherRootRecoveryActions>(() => ({
    chooseLibraryRoot,
  }), [chooseLibraryRoot]);

  const minimize = useCallback(() => {
    if (bridge) {
      void bridge.minimizeWindow().catch(() => {
        if (mountedRef.current) {
          setPresentation({ kind: 'bridge-unavailable' });
        }
      });
    }
  }, [bridge]);

  const close = useCallback(() => {
    if (bridge) {
      void bridge.close_window().catch(() => {
        if (mountedRef.current) {
          setPresentation({ kind: 'bridge-unavailable' });
        }
      });
    } else {
      window.close();
    }
  }, [bridge]);

  let content: ReactNode;
  if (presentation.kind === 'content') {
    content = children;
  } else if (presentation.kind === 'bridge-unavailable') {
    content = (
      <LauncherRootRecoveryView
        title="Desktop bridge unavailable"
        message="Pumas Library could not confirm the current library state. Close and reopen the desktop app before trying again."
        onClose={close}
        onMinimize={bridge ? minimize : undefined}
      />
    );
  } else if (presentation.kind === 'selection-result') {
    const { result } = presentation;
    const retryable = result.status === 'recovery-required' &&
      result.authorityState === 'unchanged';
    let title: string;
    let message: string;

    if (result.status === 'restarting') {
      title = 'Restarting Pumas Library';
      message = 'The library was saved. Pumas Library is restarting to use it.';
    } else if (result.status === 'not-selectable') {
      title = 'Correct launcher input';
      message = 'A launcher environment value or command argument controls this library. Correct that launch input and reopen Pumas Library to change it.';
    } else if (result.reason === 'invalid-selection') {
      title = 'Selected library is not valid';
      message = 'Choose a launcher root, shared-resources directory, or shared-resources/models directory.';
    } else if (result.reason === 'chooser-unavailable') {
      title = 'Library chooser unavailable';
      message = 'The desktop library chooser could not open. You can try again.';
    } else if (result.reason === 'restart-unavailable') {
      title = 'Restart required';
      message = 'The library was saved, but Pumas Library could not restart. Close and reopen the app to use the saved library.';
    } else if (result.authorityState === 'unchanged') {
      title = 'Library change was not saved';
      message = 'The current library is unchanged. You can try selecting it again.';
    } else if (result.authorityState === 'replacement-visibility-unknown') {
      title = 'Library change needs confirmation';
      message = 'Pumas Library cannot confirm which saved library is visible. Close and reopen the app before trying again.';
    } else {
      title = 'Library save may be incomplete';
      message = 'The new library was published, but its durability could not be confirmed. Close and reopen Pumas Library.';
    }

    content = (
      <LauncherRootRecoveryView
        title={title}
        message={message}
        primaryAction={retryable
          ? { label: 'Try Again', onAction: () => { void chooseLibraryRoot(); } }
          : undefined}
        secondaryAction={presentation.canReturnToLibrary &&
          (retryable || result.status === 'not-selectable')
          ? { label: 'Back to Library', onAction: () => { setPresentation({ kind: 'content' }); } }
          : undefined}
        onClose={close}
        onMinimize={minimize}
      />
    );
  } else if (presentation.kind === 'explicit-guidance') {
    content = (
      <LauncherRootRecoveryView
        title="Correct launcher input"
        message="A launcher environment value or command argument controls this library. Correct that launch input and reopen Pumas Library to change it."
        secondaryAction={{ label: 'Back to Library', onAction: () => { setPresentation({ kind: 'content' }); } }}
        onClose={close}
        onMinimize={minimize}
      />
    );
  } else if (presentation.kind === 'startup-recovery') {
    const canSelectLibrary = presentation.state.action === 'select-library';
    const reason = presentation.state.reason;
    const title = reason === 'migration-required' ? 'Library upgrade required'
      : reason === 'backend-unavailable' ? 'Library could not start'
      : canSelectLibrary ? 'Library needs attention' : 'Correct launcher input';
    const message = reason === 'migration-required'
      ? 'This library uses an older download database. Choose Upgrade Library to back up and upgrade its metadata, then open it. Model files stay in place.'
      : reason === 'backend-unavailable'
        ? 'Pumas Library could not open this library. Another app may already be using it. ' +
          (canSelectLibrary
            ? 'Close other apps using the library and reopen Pumas Library, or select another library.'
            : `Close other apps using the library or correct the ${presentation.state.authoritySource} launch input, then reopen Pumas Library.`)
        : canSelectLibrary
          ? reason === 'invalid'
            ? 'The saved Pumas library is not valid. Select an existing library to continue.'
            : 'The saved Pumas library is unavailable. Restore access or select another library.'
          : `The ${presentation.state.authoritySource} launcher input controls this library. Correct that launch input and reopen Pumas Library.`;
    if (reason === 'migration-required' && upgradePhase !== 'idle') {
      const confirming = upgradePhase === 'confirm';
      const running = upgradePhase === 'running';
      content = (
        <LauncherRootRecoveryView
          title={confirming ? 'Upgrade library metadata' : running ? 'Upgrading library metadata'
            : upgradePhase === 'upgraded' || upgradeFailureStage === 'open' ? 'Library metadata upgraded' : 'Library upgrade needs attention'}
          message={confirming
            ? 'Only launcher-data/downloads.json is backed up and updated. Model files are not copied or rewritten. The metadata backup is saved beside downloads.json as downloads.pre-schema7-<id>.json.'
            : running ? 'Backing up download metadata, upgrading it, and opening this library. Closing the app will wait for this operation to finish.'
              : upgradeFailureStage === 'open'
                ? 'The metadata upgrade completed, but the library backend could not start. Your metadata backup has been kept. Check the startup error in the app\'s main log and close other apps using this library before reopening.'
                : upgradePhase === 'upgraded'
                ? 'The metadata upgrade completed. Close and reopen Pumas Library to open this library. Your metadata backup has been kept.'
                : 'The upgrade could not be confirmed. Any metadata backup has been kept. Check library permissions and free space, then close and reopen Pumas Library before trying again.'}
          primaryAction={confirming ? {
            label: 'Upgrade Metadata and Open Library',
            disabled: !oldWritersStopped || !noDowngradeAccepted,
            onAction: () => { void upgradeLibrary(); },
          } : undefined}
          secondaryAction={confirming ? { label: 'Cancel', onAction: () => {
            setUpgradePhase('idle'); setOldWritersStopped(false); setNoDowngradeAccepted(false);
          } } : undefined}
          onClose={close}
          onMinimize={minimize}
        >
          {confirming && (
            <div className="mt-5 flex flex-col gap-3 text-sm">
              <label className="flex items-start gap-2">
                <input type="checkbox" checked={oldWritersStopped} onChange={(event) => setOldWritersStopped(event.target.checked)} />
                I have closed every other app, backend process, and tool using this library.
              </label>
              <label className="flex items-start gap-2">
                <input type="checkbox" checked={noDowngradeAccepted} onChange={(event) => setNoDowngradeAccepted(event.target.checked)} />
                I understand that older app versions cannot use the upgraded metadata. There is no automatic downgrade.
              </label>
            </div>
          )}
        </LauncherRootRecoveryView>
      );
    } else content = (
      <LauncherRootRecoveryView
        title={title}
        message={message}
        primaryAction={reason === 'migration-required'
          ? { label: 'Upgrade Library', onAction: () => { setUpgradePhase('confirm'); } }
          : canSelectLibrary
          ? { label: 'Select Library', onAction: () => { void chooseLibraryRoot(); } }
          : undefined}
        secondaryAction={reason === 'migration-required' && canSelectLibrary
          ? { label: 'Select Library', onAction: () => { void chooseLibraryRoot(); } } : undefined}
        onClose={close}
        onMinimize={minimize}
      />
    );
  } else {
    content = (
      <LauncherRootRecoveryView
        title={presentation.kind === 'selecting' ? 'Selecting library' : 'Checking library'}
        message={presentation.kind === 'selecting'
          ? 'Waiting for library selection…'
          : 'Checking library…'}
        onClose={close}
        onMinimize={minimize}
      />
    );
  }

  return (
    <LauncherRootRecoveryContext.Provider value={actions}>
      <LibraryScopeContext.Provider value={startupState?.status === 'ready' ? startupState.libraryScopeId : null}>
        {content}
      </LibraryScopeContext.Provider>
    </LauncherRootRecoveryContext.Provider>
  );
}

export function useLauncherRootRecovery(): LauncherRootRecoveryActions {
  const context = useContext(LauncherRootRecoveryContext);
  if (!context) {
    throw new ValidationError(
      'useLauncherRootRecovery must be used within LauncherRootRecoveryProvider.',
      'LauncherRootRecoveryProvider'
    );
  }
  return context;
}
