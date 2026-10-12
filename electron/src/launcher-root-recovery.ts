import {
  LauncherRootPersistenceError,
  LauncherRootSelectionError,
  type LauncherRootAuthorityState,
  type LauncherRootResolution,
} from './launcher-root';

type LauncherRootRecoveryRequired = Extract<
  LauncherRootStartupState,
  { status: 'recovery-required' }
>;

export type LauncherRootSelectionChoice =
  | { status: 'cancelled' }
  | { status: 'unavailable' }
  | { status: 'selected'; selectedPath: string };

export interface LauncherRootSelectionAdapter {
  chooseLibraryRoot(): Promise<LauncherRootSelectionChoice>;
  persistLauncherRoot(selectedPath: string): void;
  requestRestart(): void;
}

export type LauncherRootSelectionHandler = (
  startupState: LauncherRootStartupState
) => Promise<LauncherRootSelectionResult>;

export type LauncherRootStartupState =
  | { status: 'initializing' }
  | {
      status: 'ready';
      selectionAction: 'select-library' | 'correct-launch-input';
      libraryScopeId: string | null;
    }
  | {
      status: 'recovery-required';
      reason: 'invalid' | 'unavailable' | 'migration-required' | 'backend-unavailable';
      authoritySource: 'persisted' | 'environment' | 'argument' | 'default';
      action: 'select-library' | 'correct-launch-input';
    };

export type LauncherRootSelectionResult =
  | { status: 'cancelled' }
  | { status: 'restarting' }
  | { status: 'not-selectable'; action: 'correct-launch-input' }
  | {
      status: 'recovery-required';
      reason: 'invalid-selection' | 'chooser-unavailable';
      authorityState: 'unchanged';
    }
  | {
      status: 'recovery-required';
      reason: 'persistence-unavailable';
      authorityState: LauncherRootAuthorityState;
    }
  | {
      status: 'recovery-required';
      reason: 'restart-unavailable';
      authorityState: 'published';
    };

export function projectLauncherRootStartupState(
  resolution: Extract<LauncherRootResolution, { status: 'recovery-required' }>
): LauncherRootRecoveryRequired;
export function projectLauncherRootStartupState(
  resolution: LauncherRootResolution,
  libraryScopeId?: string | null
): LauncherRootStartupState;
export function projectLauncherRootStartupState(
  resolution: LauncherRootResolution,
  libraryScopeId: string | null = null
): LauncherRootStartupState {
  if (resolution.status === 'resolved') {
    return {
      status: 'ready',
      libraryScopeId,
      selectionAction:
        resolution.source === 'environment' || resolution.source === 'argument'
          ? 'correct-launch-input'
          : 'select-library',
    };
  }

  return {
    status: 'recovery-required',
    reason: resolution.code === 'launcher_root_invalid' ? 'invalid' : 'unavailable',
    authoritySource: resolution.authoritySource,
    action: resolution.authoritySource === 'persisted'
      ? 'select-library'
      : 'correct-launch-input',
  };
}

export function launcherRootSelectionInvalid(): LauncherRootSelectionResult {
  return {
    status: 'recovery-required',
    reason: 'invalid-selection',
    authorityState: 'unchanged',
  };
}

export function launcherRootSelectionPersistenceUnavailable(
  authorityState: LauncherRootAuthorityState
): LauncherRootSelectionResult {
  return {
    status: 'recovery-required',
    reason: 'persistence-unavailable',
    authorityState,
  };
}

export function isLauncherRootSelectionAvailable(
  startupState: LauncherRootStartupState
): boolean {
  return (startupState.status === 'ready' &&
      startupState.selectionAction === 'select-library') ||
    (startupState.status === 'recovery-required' &&
      startupState.action === 'select-library');
}

export function createLauncherRootSelectionHandler(
  adapter: LauncherRootSelectionAdapter
): LauncherRootSelectionHandler {
  let activeAttempt: Promise<LauncherRootSelectionResult> | undefined;
  let terminalResult: LauncherRootSelectionResult | undefined;

  return (startupState) => {
    if (terminalResult) {
      return Promise.resolve(terminalResult);
    }
    if (activeAttempt) {
      return activeAttempt;
    }
    if (startupState.status === 'initializing') {
      return Promise.resolve(launcherRootChooserUnavailable());
    }
    if (!isLauncherRootSelectionAvailable(startupState)) {
      return Promise.resolve({
        status: 'not-selectable',
        action: 'correct-launch-input',
      });
    }

    const ownedAttempt = executeLauncherRootSelection(adapter)
      .then((result) => {
        if (locksLauncherRootSelection(result)) {
          terminalResult = result;
        }
        return result;
      })
      .finally(() => {
        if (activeAttempt === ownedAttempt) {
          activeAttempt = undefined;
        }
      });
    activeAttempt = ownedAttempt;
    return ownedAttempt;
  };
}

async function executeLauncherRootSelection(
  adapter: LauncherRootSelectionAdapter
): Promise<LauncherRootSelectionResult> {
  let choice: LauncherRootSelectionChoice;
  try {
    choice = await adapter.chooseLibraryRoot();
  } catch {
    return launcherRootChooserUnavailable();
  }

  if (choice.status === 'unavailable') {
    return launcherRootChooserUnavailable();
  }
  if (choice.status === 'cancelled') {
    return { status: 'cancelled' };
  }

  try {
    adapter.persistLauncherRoot(choice.selectedPath);
  } catch (error) {
    if (error instanceof LauncherRootSelectionError) {
      return launcherRootSelectionInvalid();
    }
    if (error instanceof LauncherRootPersistenceError) {
      return launcherRootSelectionPersistenceUnavailable(error.authorityState);
    }
    return launcherRootSelectionPersistenceUnavailable(
      'replacement-visibility-unknown'
    );
  }

  try {
    adapter.requestRestart();
  } catch {
    return {
      status: 'recovery-required',
      reason: 'restart-unavailable',
      authorityState: 'published',
    };
  }

  return { status: 'restarting' };
}

function launcherRootChooserUnavailable(): LauncherRootSelectionResult {
  return {
    status: 'recovery-required',
    reason: 'chooser-unavailable',
    authorityState: 'unchanged',
  };
}

function locksLauncherRootSelection(result: LauncherRootSelectionResult): boolean {
  return result.status === 'restarting' ||
    (result.status === 'recovery-required' && result.authorityState !== 'unchanged');
}
