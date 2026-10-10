import assert from 'node:assert/strict';
import test from 'node:test';
import {
  createLauncherRootSelectionHandler,
  isLauncherRootSelectionAvailable,
  launcherRootSelectionInvalid,
  launcherRootSelectionPersistenceUnavailable,
  projectLauncherRootStartupState,
} from '../dist/launcher-root-recovery.js';
import {
  LauncherRootPersistenceError,
  LauncherRootSelectionError,
} from '../dist/launcher-root.js';
import {
  BackendInitializationRecoveryRequiredError,
  LauncherRootRecoveryRequiredError,
  classifyBackendInitializationOutcome,
  isLauncherRootRecoveryRequiredError,
} from '../dist/startup-task.js';
import { BackendStartupError } from '../dist/backend-startup.js';

test('backend recovery preserves launcher authority and remains fatal to release smoke', () => {
  for (const source of ['persisted', 'packaged-default', 'environment', 'argument']) {
    const error = new BackendInitializationRecoveryRequiredError({
      status: 'resolved', launcherRoot: '/private/library', source, persistedState: 'valid',
    }, new BackendStartupError('migration-required'));
    const explicit = source === 'environment' || source === 'argument';
    assert.deepEqual(error.recoveryState, {
      status: 'recovery-required', reason: 'migration-required',
      authoritySource: explicit ? source : source === 'persisted' ? 'persisted' : 'default',
      action: explicit ? 'correct-launch-input' : 'select-library',
    });
    assert.equal(isLauncherRootSelectionAvailable(error.recoveryState), !explicit);
    assert.deepEqual(classifyBackendInitializationOutcome({ status: 'rejected', error }, 'desktop'), {
      status: 'recovery-required', recoveryState: error.recoveryState,
    });
    assert.deepEqual(classifyBackendInitializationOutcome({ status: 'rejected', error }, 'release-smoke'), {
      status: 'fatal', error,
    });
    assert.equal(JSON.stringify(error.recoveryState).includes('/private'), false);
  }
});

test('startup projection strips paths and selects only valid recovery actions', () => {
  assert.deepEqual(projectLauncherRootStartupState({
    status: 'resolved',
    launcherRoot: '/sensitive/library',
    source: 'persisted',
    persistedState: 'valid',
  }), { status: 'ready', selectionAction: 'select-library', libraryScopeId: null });

  for (const source of ['environment', 'argument']) {
    assert.deepEqual(projectLauncherRootStartupState({
      status: 'resolved',
      launcherRoot: '/sensitive/explicit-library',
      source,
      persistedState: 'not-consulted',
    }), {
      status: 'ready',
      selectionAction: 'correct-launch-input',
      libraryScopeId: null,
    });
  }

  assert.deepEqual(projectLauncherRootStartupState({
    status: 'recovery-required',
    code: 'launcher_root_invalid',
    authoritySource: 'persisted',
    persistedState: 'invalid',
    message: 'path-free producer message',
  }), {
    status: 'recovery-required',
    reason: 'invalid',
    authoritySource: 'persisted',
    action: 'select-library',
  });

  for (const authoritySource of ['environment', 'argument']) {
    assert.deepEqual(projectLauncherRootStartupState({
      status: 'recovery-required',
      code: 'launcher_root_unavailable',
      authoritySource,
      persistedState: 'not-consulted',
      message: 'path-free producer message',
    }), {
      status: 'recovery-required',
      reason: 'unavailable',
      authoritySource,
      action: 'correct-launch-input',
    });
  }
});

test('only non-explicit ready and persisted recovery permit the native chooser', () => {
  assert.equal(isLauncherRootSelectionAvailable({ status: 'initializing' }), false);
  assert.equal(isLauncherRootSelectionAvailable({
    status: 'ready',
    selectionAction: 'select-library',
  }), true);
  assert.equal(isLauncherRootSelectionAvailable({
    status: 'ready',
    selectionAction: 'correct-launch-input',
  }), false);
  assert.equal(isLauncherRootSelectionAvailable({
    status: 'recovery-required',
    reason: 'invalid',
    authoritySource: 'persisted',
    action: 'select-library',
  }), true);
  assert.equal(isLauncherRootSelectionAvailable({
    status: 'recovery-required',
    reason: 'invalid',
    authoritySource: 'argument',
    action: 'correct-launch-input',
  }), false);
});

test('selection projection preserves only renderer-owned terminal distinctions', () => {
  assert.deepEqual(launcherRootSelectionInvalid(), {
    status: 'recovery-required',
    reason: 'invalid-selection',
    authorityState: 'unchanged',
  });

  for (const authorityState of [
    'unchanged',
    'replacement-visibility-unknown',
    'published-durability-unavailable',
  ]) {
    assert.deepEqual(launcherRootSelectionPersistenceUnavailable(authorityState), {
      status: 'recovery-required',
      reason: 'persistence-unavailable',
      authorityState,
    });
  }
});

test('typed startup recovery preserves the closed state for lifecycle classification', () => {
  const resolution = {
    status: 'recovery-required',
    code: 'launcher_root_unavailable',
    authoritySource: 'persisted',
    persistedState: 'unavailable',
    message: 'path-free recovery message',
  };
  const error = new LauncherRootRecoveryRequiredError(resolution);

  assert.equal(isLauncherRootRecoveryRequiredError(error), true);
  assert.equal(isLauncherRootRecoveryRequiredError(new Error('backend failed')), false);
  assert.deepEqual(error.recoveryState, {
    status: 'recovery-required',
    reason: 'unavailable',
    authoritySource: 'persisted',
    action: 'select-library',
  });
});

test('normal startup exposes root recovery while release smoke preserves terminal failure', () => {
  const recoveryError = new LauncherRootRecoveryRequiredError({
    status: 'recovery-required',
    code: 'launcher_root_invalid',
    authoritySource: 'persisted',
    persistedState: 'invalid',
    message: 'path-free recovery message',
  });
  const rejected = { status: 'rejected', error: recoveryError };

  assert.deepEqual(classifyBackendInitializationOutcome(rejected, 'desktop'), {
    status: 'recovery-required',
    recoveryState: recoveryError.recoveryState,
  });
  assert.deepEqual(classifyBackendInitializationOutcome(rejected, 'release-smoke'), {
    status: 'fatal',
    error: recoveryError,
  });

  const backendError = new Error('backend failed');
  assert.deepEqual(classifyBackendInitializationOutcome(
    { status: 'rejected', error: backendError },
    'desktop'
  ), {
    status: 'fatal',
    error: backendError,
  });
  assert.deepEqual(classifyBackendInitializationOutcome(
    { status: 'fulfilled' },
    'desktop'
  ), { status: 'ready' });
});

test('selection handler blocks explicit authority and unavailable chooser without persistence', async () => {
  let chooseCalls = 0;
  let persistCalls = 0;
  let restartCalls = 0;
  const handler = createLauncherRootSelectionHandler({
    chooseLibraryRoot: async () => {
      chooseCalls += 1;
      return { status: 'unavailable' };
    },
    persistLauncherRoot: () => {
      persistCalls += 1;
    },
    requestRestart: () => {
      restartCalls += 1;
    },
  });

  assert.deepEqual(await handler({ status: 'initializing' }), {
    status: 'recovery-required',
    reason: 'chooser-unavailable',
    authorityState: 'unchanged',
  });
  assert.deepEqual(await handler({
    status: 'ready',
    selectionAction: 'correct-launch-input',
  }), {
    status: 'not-selectable',
    action: 'correct-launch-input',
  });
  assert.deepEqual(await handler({
    status: 'recovery-required',
    reason: 'invalid',
    authoritySource: 'argument',
    action: 'correct-launch-input',
  }), {
    status: 'not-selectable',
    action: 'correct-launch-input',
  });
  assert.deepEqual(await handler({
    status: 'ready',
    selectionAction: 'select-library',
  }), {
    status: 'recovery-required',
    reason: 'chooser-unavailable',
    authorityState: 'unchanged',
  });
  assert.deepEqual({ chooseCalls, persistCalls, restartCalls }, {
    chooseCalls: 1,
    persistCalls: 0,
    restartCalls: 0,
  });
});

test('selection handler observes dialog rejection and permits a later attempt', async () => {
  let chooseCalls = 0;
  const handler = createLauncherRootSelectionHandler({
    chooseLibraryRoot: async () => {
      chooseCalls += 1;
      if (chooseCalls === 1) {
        throw new Error('native dialog failed with private detail');
      }
      return { status: 'cancelled' };
    },
    persistLauncherRoot: () => assert.fail('cancelled selection must not persist'),
    requestRestart: () => assert.fail('cancelled selection must not restart'),
  });
  const selectable = { status: 'ready', selectionAction: 'select-library' };

  assert.deepEqual(await handler(selectable), {
    status: 'recovery-required',
    reason: 'chooser-unavailable',
    authorityState: 'unchanged',
  });
  assert.deepEqual(await handler(selectable), { status: 'cancelled' });
  assert.equal(chooseCalls, 2);
});

test('selection handler shares one active attempt and locks successful restart', async () => {
  let resolveChoice;
  const choice = new Promise((resolve) => {
    resolveChoice = resolve;
  });
  const calls = { choose: 0, persist: 0, restart: 0 };
  const handler = createLauncherRootSelectionHandler({
    chooseLibraryRoot: () => {
      calls.choose += 1;
      return choice;
    },
    persistLauncherRoot: (selectedPath) => {
      calls.persist += 1;
      assert.equal(selectedPath, '/selected/library');
    },
    requestRestart: () => {
      calls.restart += 1;
    },
  });
  const selectable = { status: 'ready', selectionAction: 'select-library' };

  const first = handler(selectable);
  const overlapping = handler(selectable);
  assert.equal(first, overlapping);
  assert.deepEqual(calls, { choose: 1, persist: 0, restart: 0 });

  resolveChoice({ status: 'selected', selectedPath: '/selected/library' });
  assert.deepEqual(await first, { status: 'restarting' });
  assert.deepEqual(await overlapping, { status: 'restarting' });
  assert.deepEqual(await handler(selectable), { status: 'restarting' });
  assert.deepEqual(calls, { choose: 1, persist: 1, restart: 1 });
});

test('selection handler permits retry only after unchanged outcomes', async () => {
  const selectedPaths = [
    '/invalid/library',
    '/unchanged/library',
    '/cancelled-after-retry',
  ];
  let chooseCalls = 0;
  const handler = createLauncherRootSelectionHandler({
    chooseLibraryRoot: async () => {
      const selectedPath = selectedPaths[chooseCalls];
      chooseCalls += 1;
      if (selectedPath === '/cancelled-after-retry') {
        return { status: 'cancelled' };
      }
      return { status: 'selected', selectedPath };
    },
    persistLauncherRoot: (selectedPath) => {
      if (selectedPath === '/invalid/library') {
        throw new LauncherRootSelectionError();
      }
      throw new LauncherRootPersistenceError(
        'temporary-write',
        'unchanged',
        'complete',
        new Error('private persistence cause')
      );
    },
    requestRestart: () => assert.fail('failed persistence must not restart'),
  });
  const selectable = { status: 'ready', selectionAction: 'select-library' };

  assert.deepEqual(await handler(selectable), {
    status: 'recovery-required',
    reason: 'invalid-selection',
    authorityState: 'unchanged',
  });
  assert.deepEqual(await handler(selectable), {
    status: 'recovery-required',
    reason: 'persistence-unavailable',
    authorityState: 'unchanged',
  });
  assert.deepEqual(await handler(selectable), { status: 'cancelled' });
  assert.equal(chooseCalls, 3);
});

test('selection handler locks ambiguous and published persistence outcomes', async () => {
  const failures = [
    {
      authorityState: 'replacement-visibility-unknown',
      error: new LauncherRootPersistenceError(
        'replace',
        'replacement-visibility-unknown',
        'complete',
        new Error('private persistence cause')
      ),
    },
    {
      authorityState: 'published-durability-unavailable',
      error: new LauncherRootPersistenceError(
        'parent-sync',
        'published-durability-unavailable',
        'complete',
        new Error('private persistence cause')
      ),
    },
    {
      authorityState: 'replacement-visibility-unknown',
      error: new Error('unexpected persistence failure with private detail'),
    },
  ];

  for (const { authorityState, error } of failures) {
    const calls = { choose: 0, persist: 0, restart: 0 };
    const handler = createLauncherRootSelectionHandler({
      chooseLibraryRoot: async () => {
        calls.choose += 1;
        return { status: 'selected', selectedPath: '/selected/library' };
      },
      persistLauncherRoot: () => {
        calls.persist += 1;
        throw error;
      },
      requestRestart: () => {
        calls.restart += 1;
      },
    });
    const selectable = { status: 'ready', selectionAction: 'select-library' };
    const expected = {
      status: 'recovery-required',
      reason: 'persistence-unavailable',
      authorityState,
    };

    assert.deepEqual(await handler(selectable), expected);
    assert.deepEqual(await handler(selectable), expected);
    assert.deepEqual(calls, { choose: 1, persist: 1, restart: 0 });
  }
});

test('selection handler reports and locks a failed synchronous relaunch request', async () => {
  const calls = { choose: 0, persist: 0, restart: 0 };
  const handler = createLauncherRootSelectionHandler({
    chooseLibraryRoot: async () => {
      calls.choose += 1;
      return { status: 'selected', selectedPath: '/selected/library' };
    },
    persistLauncherRoot: () => {
      calls.persist += 1;
    },
    requestRestart: () => {
      calls.restart += 1;
      throw new Error('relaunch request failed with private detail');
    },
  });
  const selectable = { status: 'ready', selectionAction: 'select-library' };
  const expected = {
    status: 'recovery-required',
    reason: 'restart-unavailable',
    authorityState: 'published',
  };

  assert.deepEqual(await handler(selectable), expected);
  assert.deepEqual(await handler(selectable), expected);
  assert.deepEqual(calls, { choose: 1, persist: 1, restart: 1 });
});
