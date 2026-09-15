import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import {
  closeSync,
  fsyncSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import {
  LauncherRootPersistenceError,
  launcherRootOverrideConfigPath,
  persistLauncherRootOverride,
  resolveLauncherRoot,
} from '../dist/launcher-root.js';
import {
  LauncherRootRecoveryRequiredError,
  observeBackendInitialization,
  projectBackendInitializationFailure,
} from '../dist/startup-task.js';

function createLauncherRoot(root) {
  mkdirSync(join(root, 'shared-resources', 'models'), { recursive: true });
}

const LAUNCHER_ROOT_TEMP_PREFIX = 'launcher-root.json.tmp-';

function persistenceFailure(stage, code = 'EIO') {
  const error = new Error(`injected ${stage} failure`);
  error.code = code;
  return error;
}

function createPersistenceAdapter({ failureStage, events = [] } = {}) {
  return {
    ensureDirectory(directoryPath) {
      events.push('ensure-directory');
      if (failureStage === 'directory-ensure') {
        throw persistenceFailure(failureStage);
      }
      mkdirSync(directoryPath, { recursive: true });
    },
    openParentDirectory(directoryPath) {
      events.push('open-parent');
      if (failureStage === 'parent-open') {
        throw persistenceFailure(failureStage);
      }
      return openSync(directoryPath, process.platform === 'win32' ? 'r+' : 'r');
    },
    createTemporaryName(authorityFilename) {
      events.push('create-temporary-name');
      if (failureStage === 'temporary-name') {
        throw persistenceFailure(failureStage);
      }
      return `${authorityFilename}.tmp-test-${process.pid}`;
    },
    openTemporaryFile(temporaryPath) {
      events.push('open-temporary');
      if (failureStage === 'temporary-open') {
        throw persistenceFailure(failureStage);
      }
      return openSync(temporaryPath, 'wx', 0o600);
    },
    writeTemporaryFile(descriptor, serializedConfig) {
      events.push('write-temporary');
      if (failureStage === 'partial-write') {
        writeFileSync(descriptor, serializedConfig.slice(0, 7), 'utf8');
        throw persistenceFailure(failureStage);
      }
      writeFileSync(descriptor, serializedConfig, 'utf8');
      if (failureStage === 'full-write') {
        throw persistenceFailure(failureStage);
      }
    },
    syncTemporaryFile(descriptor) {
      events.push('sync-temporary');
      if (failureStage === 'temporary-sync') {
        throw persistenceFailure(failureStage);
      }
      fsyncSync(descriptor);
    },
    closeTemporaryFile(descriptor) {
      events.push('close-temporary');
      closeSync(descriptor);
      if (failureStage === 'temporary-close') {
        throw persistenceFailure(failureStage);
      }
    },
    replaceAuthority(temporaryPath, authorityPath) {
      events.push('replace-authority');
      if (failureStage === 'replace') {
        throw persistenceFailure(failureStage, 'EXDEV');
      }
      renameSync(temporaryPath, authorityPath);
    },
    syncParentDirectory(descriptor) {
      events.push('sync-parent');
      if (failureStage === 'parent-sync') {
        throw persistenceFailure(failureStage);
      }
      fsyncSync(descriptor);
    },
    closeParentDirectory(descriptor) {
      events.push('close-parent');
      closeSync(descriptor);
      if (failureStage === 'parent-close') {
        throw persistenceFailure(failureStage);
      }
    },
    removeTemporaryFile(temporaryPath) {
      events.push('remove-temporary');
      if (failureStage === 'cleanup') {
        throw persistenceFailure(failureStage);
      }
      unlinkSync(temporaryPath);
    },
  };
}

function launcherRootResolutionOptions(userDataPath) {
  return {
    argv: [],
    devRoot: join(userDataPath, 'development-default'),
    env: {},
    execPath: join(userDataPath, 'pumas-library'),
    isPackaged: true,
    userDataPath,
  };
}

function assertPersistedRoot(userDataPath, launcherRoot) {
  assert.deepEqual(resolveLauncherRoot(launcherRootResolutionOptions(userDataPath)), {
    status: 'resolved',
    launcherRoot,
    source: 'persisted',
    persistedState: 'valid',
  });
}

function launcherRootTemporaryFiles(userDataPath) {
  return readdirSync(userDataPath).filter((entry) => entry.startsWith(LAUNCHER_ROOT_TEMP_PREFIX));
}

for (const interruption of [
  {
    failureStage: 'directory-ensure',
    expectedStage: 'directory-ensure',
    cleanupState: 'not-required',
  },
  { failureStage: 'parent-open', expectedStage: 'parent-open', cleanupState: 'not-required' },
  { failureStage: 'temporary-name', expectedStage: 'temporary-name', cleanupState: 'complete' },
  { failureStage: 'temporary-open', expectedStage: 'temporary-open', cleanupState: 'complete' },
  { failureStage: 'partial-write', expectedStage: 'temporary-write', cleanupState: 'complete' },
  { failureStage: 'full-write', expectedStage: 'temporary-write', cleanupState: 'complete' },
  { failureStage: 'temporary-sync', expectedStage: 'temporary-sync', cleanupState: 'complete' },
  { failureStage: 'temporary-close', expectedStage: 'temporary-close', cleanupState: 'incomplete' },
]) {
  test(`pre-publication ${interruption.failureStage} failure preserves authority`, () => {
    const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-persist-'));

    try {
      const oldRoot = join(fixtureRoot, 'old-root');
      const newRoot = join(fixtureRoot, 'new-root');
      const userDataPath = join(fixtureRoot, 'user-data');
      createLauncherRoot(oldRoot);
      createLauncherRoot(newRoot);
      mkdirSync(userDataPath, { recursive: true });
      const authorityPath = launcherRootOverrideConfigPath(userDataPath);
      const oldBytes = `${JSON.stringify({ launcherRoot: oldRoot, updatedAt: 'old' })}\n`;
      writeFileSync(authorityPath, oldBytes, 'utf8');

      assert.throws(
        () => persistLauncherRootOverride(
          userDataPath,
          newRoot,
          createPersistenceAdapter({ failureStage: interruption.failureStage })
        ),
        (error) => {
          assert.ok(error instanceof LauncherRootPersistenceError);
          assert.equal(error.code, 'launcher_root_persistence_unavailable');
          assert.equal(error.stage, interruption.expectedStage);
          assert.equal(error.authorityState, 'unchanged');
          assert.equal(error.cleanupState, interruption.cleanupState);
          assert.equal(error.message, 'Unable to persist launcher root selection.');
          return true;
        }
      );

      assert.equal(readFileSync(authorityPath, 'utf8'), oldBytes);
      assertPersistedRoot(userDataPath, oldRoot);
      assert.deepEqual(launcherRootTemporaryFiles(userDataPath), []);
    } finally {
      rmSync(fixtureRoot, { recursive: true, force: true });
    }
  });
}

test('rename failure preserves old bytes but reports replacement visibility unknown', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-persist-'));

  try {
    const oldRoot = join(fixtureRoot, 'old-root');
    const newRoot = join(fixtureRoot, 'new-root');
    const userDataPath = join(fixtureRoot, 'user-data');
    createLauncherRoot(oldRoot);
    createLauncherRoot(newRoot);
    mkdirSync(userDataPath, { recursive: true });
    const authorityPath = launcherRootOverrideConfigPath(userDataPath);
    const oldBytes = `${JSON.stringify({ launcherRoot: oldRoot, updatedAt: 'old' })}\n`;
    writeFileSync(authorityPath, oldBytes, 'utf8');

    assert.throws(
      () => persistLauncherRootOverride(
        userDataPath,
        newRoot,
        createPersistenceAdapter({ failureStage: 'replace' })
      ),
      (error) => {
        assert.ok(error instanceof LauncherRootPersistenceError);
        assert.equal(error.stage, 'replace');
        assert.equal(error.authorityState, 'replacement-visibility-unknown');
        assert.equal(error.cleanupState, 'complete');
        assert.equal(error.cause.code, 'EXDEV');
        return true;
      }
    );

    assert.equal(readFileSync(authorityPath, 'utf8'), oldBytes);
    assertPersistedRoot(userDataPath, oldRoot);
    assert.deepEqual(launcherRootTemporaryFiles(userDataPath), []);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

for (const failureStage of ['parent-sync', 'parent-close']) {
  test(`${failureStage} failure reports published authority with unavailable durability`, () => {
    const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-persist-'));

    try {
      const oldRoot = join(fixtureRoot, 'old-root');
      const newRoot = join(fixtureRoot, 'new-root');
      const userDataPath = join(fixtureRoot, 'user-data');
      createLauncherRoot(oldRoot);
      createLauncherRoot(newRoot);
      mkdirSync(userDataPath, { recursive: true });
      const authorityPath = launcherRootOverrideConfigPath(userDataPath);
      writeFileSync(
        authorityPath,
        `${JSON.stringify({ launcherRoot: oldRoot, updatedAt: 'old' })}\n`,
        'utf8'
      );

      assert.throws(
        () => persistLauncherRootOverride(
          userDataPath,
          newRoot,
          createPersistenceAdapter({ failureStage })
        ),
        (error) => {
          assert.ok(error instanceof LauncherRootPersistenceError);
          assert.equal(error.stage, failureStage);
          assert.equal(error.authorityState, 'published-durability-unavailable');
          assert.equal(
            error.cleanupState,
            failureStage === 'parent-close' ? 'incomplete' : 'complete'
          );
          return true;
        }
      );

      const published = JSON.parse(readFileSync(authorityPath, 'utf8'));
      assert.equal(published.launcherRoot, newRoot);
      assertPersistedRoot(userDataPath, newRoot);
      assert.deepEqual(launcherRootTemporaryFiles(userDataPath), []);
    } finally {
      rmSync(fixtureRoot, { recursive: true, force: true });
    }
  });
}

test('cleanup failure preserves the primary pre-publication cause and old authority', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-persist-'));

  try {
    const oldRoot = join(fixtureRoot, 'old-root');
    const newRoot = join(fixtureRoot, 'new-root');
    const userDataPath = join(fixtureRoot, 'user-data');
    createLauncherRoot(oldRoot);
    createLauncherRoot(newRoot);
    mkdirSync(userDataPath, { recursive: true });
    const authorityPath = launcherRootOverrideConfigPath(userDataPath);
    const oldBytes = `${JSON.stringify({ launcherRoot: oldRoot, updatedAt: 'old' })}\n`;
    writeFileSync(authorityPath, oldBytes, 'utf8');
    const adapter = createPersistenceAdapter({ failureStage: 'partial-write' });
    adapter.removeTemporaryFile = () => {
      const legalNonErrorThrow = null;
      throw legalNonErrorThrow;
    };

    assert.throws(
      () => persistLauncherRootOverride(userDataPath, newRoot, adapter),
      (error) => {
        assert.ok(error instanceof LauncherRootPersistenceError);
        assert.equal(error.stage, 'temporary-write');
        assert.equal(error.authorityState, 'unchanged');
        assert.equal(error.cleanupState, 'incomplete');
        assert.match(String(error.cause), /partial-write/);
        return true;
      }
    );

    assert.equal(readFileSync(authorityPath, 'utf8'), oldBytes);
    assertPersistedRoot(userDataPath, oldRoot);
    assert.equal(launcherRootTemporaryFiles(userDataPath).length, 1);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('successful persistence follows the selected publication order on the native host', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-persist-'));

  try {
    const newRoot = join(fixtureRoot, 'new-root');
    const defaultAdapterRoot = join(fixtureRoot, 'default-adapter-root');
    const userDataPath = join(fixtureRoot, 'user-data');
    const events = [];
    createLauncherRoot(newRoot);
    createLauncherRoot(defaultAdapterRoot);
    mkdirSync(userDataPath, { recursive: true });

    const config = persistLauncherRootOverride(
      userDataPath,
      newRoot,
      createPersistenceAdapter({ events })
    );
    const authorityPath = launcherRootOverrideConfigPath(userDataPath);

    assert.deepEqual(events, [
      'ensure-directory',
      'open-parent',
      'create-temporary-name',
      'open-temporary',
      'write-temporary',
      'sync-temporary',
      'close-temporary',
      'replace-authority',
      'sync-parent',
      'close-parent',
    ]);
    assert.deepEqual(JSON.parse(readFileSync(authorityPath, 'utf8')), config);
    if (process.platform !== 'win32') assert.equal(statSync(authorityPath).mode & 0o777, 0o600);
    assertPersistedRoot(userDataPath, newRoot);
    assert.deepEqual(launcherRootTemporaryFiles(userDataPath), []);

    const defaultConfig = persistLauncherRootOverride(userDataPath, defaultAdapterRoot);
    assert.deepEqual(JSON.parse(readFileSync(authorityPath, 'utf8')), defaultConfig);
    if (process.platform !== 'win32') assert.equal(statSync(authorityPath).mode & 0o777, 0o600);
    assertPersistedRoot(userDataPath, defaultAdapterRoot);
    assert.deepEqual(launcherRootTemporaryFiles(userDataPath), []);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

async function interruptPersistenceAtBarrier(barrier, userDataPath, selectedPath) {
  const moduleUrl = new URL('../dist/launcher-root.js', import.meta.url).href;
  const childSource = `
    import * as fs from 'node:fs';
    const { persistLauncherRootOverride } = await import(${JSON.stringify(moduleUrl)});
    const barrier = ${JSON.stringify(barrier)};
    const adapter = {
      ensureDirectory: (value) => fs.mkdirSync(value, { recursive: true }),
      openParentDirectory: (value) => fs.openSync(value, process.platform === 'win32' ? 'r+' : 'r'),
      createTemporaryName: (value) => value + '.tmp-child-' + process.pid,
      openTemporaryFile: (value) => fs.openSync(value, 'wx', 0o600),
      writeTemporaryFile: (fd, value) => fs.writeFileSync(fd, value, 'utf8'),
      syncTemporaryFile: (fd) => fs.fsyncSync(fd),
      closeTemporaryFile: (fd) => fs.closeSync(fd),
      replaceAuthority: (from, to) => {
        if (barrier === 'before-replace') {
          fs.writeSync(1, 'barrier\\n');
          Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0);
        }
        fs.renameSync(from, to);
      },
      syncParentDirectory: (fd) => {
        if (barrier === 'after-replace') {
          fs.writeSync(1, 'barrier\\n');
          Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0);
        }
        fs.fsyncSync(fd);
      },
      closeParentDirectory: (fd) => fs.closeSync(fd),
      removeTemporaryFile: (value) => fs.unlinkSync(value),
    };
    persistLauncherRootOverride(
      ${JSON.stringify(userDataPath)},
      ${JSON.stringify(selectedPath)},
      adapter
    );
  `;

  await new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ['--input-type=module', '--eval', childSource], {
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stderr = '';
    let interrupted = false;
    const timeout = setTimeout(() => {
      child.kill('SIGKILL');
      reject(new Error(`persistence child did not reach ${barrier}: ${stderr}`));
    }, 5_000);

    child.stderr.setEncoding('utf8');
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', (chunk) => {
      if (!interrupted && chunk.includes('barrier')) {
        interrupted = true;
        child.kill('SIGKILL');
      }
    });
    child.on('error', (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    child.on('exit', (_code, signal) => {
      clearTimeout(timeout);
      if (!interrupted || signal !== 'SIGKILL') {
        reject(new Error(`unexpected persistence child exit at ${barrier}: ${stderr}`));
        return;
      }
      resolve();
    });
  });
}

for (const interruption of [
  { barrier: 'before-replace', expected: 'old' },
  { barrier: 'after-replace', expected: 'new' },
]) {
  test(`SIGKILL ${interruption.barrier} leaves complete ${interruption.expected} authority`, async () => {
    const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-persist-'));

    try {
      const oldRoot = join(fixtureRoot, 'old-root');
      const newRoot = join(fixtureRoot, 'new-root');
      const userDataPath = join(fixtureRoot, 'user-data');
      createLauncherRoot(oldRoot);
      createLauncherRoot(newRoot);
      mkdirSync(userDataPath, { recursive: true });
      const authorityPath = launcherRootOverrideConfigPath(userDataPath);
      writeFileSync(
        authorityPath,
        `${JSON.stringify({ launcherRoot: oldRoot, updatedAt: 'old' })}\n`,
        'utf8'
      );

      await interruptPersistenceAtBarrier(interruption.barrier, userDataPath, newRoot);

      const expectedRoot = interruption.expected === 'old' ? oldRoot : newRoot;
      const persisted = JSON.parse(readFileSync(authorityPath, 'utf8'));
      assert.equal(persisted.launcherRoot, expectedRoot);
      assertPersistedRoot(userDataPath, expectedRoot);
    } finally {
      rmSync(fixtureRoot, { recursive: true, force: true });
    }
  });
}

test('backend initialization rejection is observed while window creation is delayed', async () => {
  const recoveryFailure = new LauncherRootRecoveryRequiredError({
    status: 'recovery-required',
    code: 'launcher_root_invalid',
    authoritySource: 'persisted',
    persistedState: 'invalid',
    message: 'stable recovery required',
  });
  const unhandledRejections = [];
  const onUnhandledRejection = (reason) => {
    unhandledRejections.push(reason);
  };
  process.on('unhandledRejection', onUnhandledRejection);

  try {
    const initialization = observeBackendInitialization(Promise.reject(recoveryFailure));

    // Model createWindow remaining pending beyond the turn in which initialization rejects.
    await new Promise((resolve) => setImmediate(resolve));
    assert.deepEqual(unhandledRejections, []);

    const outcome = await initialization;
    assert.equal(outcome.status, 'rejected');
    assert.equal(outcome.error, recoveryFailure);
    assert.deepEqual(
      projectBackendInitializationFailure('Failed to initialize backend bridge', outcome.error),
      {
        message:
          'Failed to initialize backend bridge: launcher_root_invalid (persisted): stable recovery required',
      }
    );

    // The closed observer preserves one typed terminal failure without raw logging.
    await new Promise((resolve) => setImmediate(resolve));
    assert.deepEqual(unhandledRejections, []);
  } finally {
    process.off('unhandledRejection', onUnhandledRejection);
  }
});

test('persisted authority distinguishes absence and validity and blocks invalid discovery', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-root-'));

  try {
    const discoveryRoot = join(fixtureRoot, 'discovery-root');
    const persistedRoot = join(fixtureRoot, 'persisted-root');
    const userDataPath = join(fixtureRoot, 'user-data');
    createLauncherRoot(discoveryRoot);
    createLauncherRoot(persistedRoot);

    const options = {
      argv: [],
      devRoot: join(fixtureRoot, 'development-default'),
      env: {},
      execPath: join(discoveryRoot, 'bin', 'pumas-library'),
      isPackaged: true,
      userDataPath,
    };

    mkdirSync(userDataPath, { recursive: true });
    writeFileSync(launcherRootOverrideConfigPath(userDataPath), '{\n', 'utf8');
    assert.deepEqual(resolveLauncherRoot(options), {
      status: 'recovery-required',
      code: 'launcher_root_invalid',
      authoritySource: 'persisted',
      persistedState: 'invalid',
      message: 'The saved launcher root is invalid; select an existing Pumas library to recover.',
    });

    unlinkSync(launcherRootOverrideConfigPath(userDataPath));
    assert.deepEqual(resolveLauncherRoot(options), {
      status: 'resolved',
      launcherRoot: discoveryRoot,
      source: 'discovery',
      persistedState: 'absent',
    });

    writeFileSync(
      launcherRootOverrideConfigPath(userDataPath),
      `${JSON.stringify({ launcherRoot: persistedRoot })}\n`,
      'utf8'
    );
    assert.deepEqual(resolveLauncherRoot(options), {
      status: 'resolved',
      launcherRoot: persistedRoot,
      source: 'persisted',
      persistedState: 'valid',
    });

  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('persisted metadata is non-authoritative and the JSON value must be a record', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-root-'));

  try {
    const persistedRoot = join(fixtureRoot, 'persisted-root');
    const userDataPath = join(fixtureRoot, 'user-data');
    createLauncherRoot(persistedRoot);
    mkdirSync(userDataPath, { recursive: true });

    const options = {
      argv: [],
      devRoot: join(fixtureRoot, 'development-default'),
      env: {},
      execPath: join(fixtureRoot, 'pumas-library'),
      isPackaged: true,
      userDataPath,
    };

    writeFileSync(
      launcherRootOverrideConfigPath(userDataPath),
      `${JSON.stringify({
        launcherRoot: persistedRoot,
        selectedPath: 17,
        updatedAt: null,
        futureMetadata: { ignored: true },
      })}\n`,
      'utf8'
    );
    assert.deepEqual(resolveLauncherRoot(options), {
      status: 'resolved',
      launcherRoot: persistedRoot,
      source: 'persisted',
      persistedState: 'valid',
    });

    writeFileSync(
      launcherRootOverrideConfigPath(userDataPath),
      `${JSON.stringify([{ launcherRoot: persistedRoot }])}\n`,
      'utf8'
    );
    assert.deepEqual(resolveLauncherRoot(options), {
      status: 'recovery-required',
      code: 'launcher_root_invalid',
      authoritySource: 'persisted',
      persistedState: 'invalid',
      message: 'The saved launcher root is invalid; select an existing Pumas library to recover.',
    });
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('unavailable persisted authority cannot fall through to discovery', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-root-'));

  try {
    const discoveryRoot = join(fixtureRoot, 'discovery-root');
    const unavailableUserDataPath = join(fixtureRoot, 'user-data-is-a-file');
    createLauncherRoot(discoveryRoot);
    writeFileSync(unavailableUserDataPath, 'not a directory', 'utf8');

    assert.deepEqual(resolveLauncherRoot({
      argv: [],
      devRoot: join(fixtureRoot, 'development-default'),
      env: {},
      execPath: join(discoveryRoot, 'bin', 'pumas-library'),
      isPackaged: true,
      userDataPath: unavailableUserDataPath,
    }), {
      status: 'recovery-required',
      code: 'launcher_root_unavailable',
      authoritySource: 'persisted',
      persistedState: 'unavailable',
      message: 'The saved launcher root is unavailable; restore access or select a Pumas library to recover.',
    });
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('invalid launcher-root argument is rejected before persisted state or discovery', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-root-'));

  try {
    const discoveryRoot = join(fixtureRoot, 'discovery-root');
    const invalidArgumentRoot = join(fixtureRoot, 'missing-argument-root');
    createLauncherRoot(discoveryRoot);

    assert.deepEqual(resolveLauncherRoot({
      argv: ['pumas-library', '--launcher-root', invalidArgumentRoot],
      devRoot: join(fixtureRoot, 'development-default'),
      env: {},
      execPath: join(discoveryRoot, 'bin', 'pumas-library'),
      isPackaged: true,
      userDataPath: join(fixtureRoot, 'user-data'),
    }), {
      status: 'recovery-required',
      code: 'launcher_root_invalid',
      authoritySource: 'argument',
      persistedState: 'not-consulted',
      message: 'The selected launcher root is invalid; select an existing Pumas library to recover.',
    });
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('invalid launcher-root environment override is rejected before argument and discovery', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-root-'));

  try {
    const discoveryRoot = join(fixtureRoot, 'discovery-root');
    const validArgumentRoot = join(fixtureRoot, 'argument-root');
    const invalidEnvironmentRoot = join(fixtureRoot, 'missing-environment-root');
    createLauncherRoot(discoveryRoot);
    createLauncherRoot(validArgumentRoot);

    assert.deepEqual(resolveLauncherRoot({
      argv: ['pumas-library', `--launcher-root=${validArgumentRoot}`],
      devRoot: join(fixtureRoot, 'development-default'),
      env: { PUMAS_LAUNCHER_ROOT: invalidEnvironmentRoot },
      execPath: join(discoveryRoot, 'bin', 'pumas-library'),
      isPackaged: true,
      userDataPath: join(fixtureRoot, 'user-data'),
    }), {
      status: 'recovery-required',
      code: 'launcher_root_invalid',
      authoritySource: 'environment',
      persistedState: 'not-consulted',
      message: 'The selected launcher root is invalid; select an existing Pumas library to recover.',
    });
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('valid explicit roots are normalized and skip persisted authority', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-root-'));

  try {
    const argumentRoot = join(fixtureRoot, 'argument-root');
    const environmentRoot = join(fixtureRoot, 'environment-root');
    createLauncherRoot(argumentRoot);
    createLauncherRoot(environmentRoot);

    const baseOptions = {
      devRoot: join(fixtureRoot, 'development-default'),
      execPath: join(fixtureRoot, 'pumas-library'),
      isPackaged: true,
      userDataPath: join(fixtureRoot, 'user-data'),
    };

    assert.deepEqual(resolveLauncherRoot({
      ...baseOptions,
      argv: [
        'pumas-library',
        '--launcher-root',
        join(argumentRoot, 'shared-resources', 'models'),
      ],
      env: {},
    }), {
      status: 'resolved',
      launcherRoot: argumentRoot,
      source: 'argument',
      persistedState: 'not-consulted',
    });

    assert.deepEqual(resolveLauncherRoot({
      ...baseOptions,
      argv: ['pumas-library', `--launcher-root=${argumentRoot}`],
      env: { PUMAS_LAUNCHER_ROOT: join(environmentRoot, 'shared-resources') },
    }), {
      status: 'resolved',
      launcherRoot: environmentRoot,
      source: 'environment',
      persistedState: 'not-consulted',
    });
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('launcher-root markers must be directories', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-root-'));

  try {
    const invalidRoot = join(fixtureRoot, 'file-marker-root');
    mkdirSync(join(invalidRoot, 'shared-resources'), { recursive: true });
    writeFileSync(join(invalidRoot, 'shared-resources', 'models'), 'not a directory', 'utf8');

    assert.deepEqual(resolveLauncherRoot({
      argv: ['pumas-library', '--launcher-root', invalidRoot],
      devRoot: join(fixtureRoot, 'development-default'),
      env: {},
      execPath: join(fixtureRoot, 'pumas-library'),
      isPackaged: true,
      userDataPath: join(fixtureRoot, 'user-data'),
    }), {
      status: 'recovery-required',
      code: 'launcher_root_invalid',
      authoritySource: 'argument',
      persistedState: 'not-consulted',
      message: 'The selected launcher root is invalid; select an existing Pumas library to recover.',
    });
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('authoritative root validation reports deterministic I/O failure as unavailable', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-root-'));

  try {
    const launcherRoot = join(fixtureRoot, 'launcher-root');
    const userDataPath = join(fixtureRoot, 'user-data');
    createLauncherRoot(launcherRoot);
    mkdirSync(userDataPath, { recursive: true });
    writeFileSync(
      launcherRootOverrideConfigPath(userDataPath),
      `${JSON.stringify({ launcherRoot })}\n`,
      'utf8'
    );

    const unavailableFileSystem = {
      readFileSync,
      statSync() {
        const error = new Error('injected launcher-root validation failure');
        error.code = 'EIO';
        throw error;
      },
    };
    const baseOptions = {
      devRoot: join(fixtureRoot, 'development-default'),
      execPath: join(fixtureRoot, 'pumas-library'),
      isPackaged: true,
      userDataPath,
    };

    for (const authority of [
      {
        authoritySource: 'argument',
        argv: ['pumas-library', '--launcher-root', launcherRoot],
        env: {},
        persistedState: 'not-consulted',
      },
      {
        authoritySource: 'environment',
        argv: [],
        env: { PUMAS_LAUNCHER_ROOT: launcherRoot },
        persistedState: 'not-consulted',
      },
      {
        authoritySource: 'persisted',
        argv: [],
        env: {},
        persistedState: 'unavailable',
      },
    ]) {
      assert.deepEqual(resolveLauncherRoot({
        ...baseOptions,
        argv: authority.argv,
        env: authority.env,
      }, unavailableFileSystem), {
        status: 'recovery-required',
        code: 'launcher_root_unavailable',
        authoritySource: authority.authoritySource,
        persistedState: authority.persistedState,
        message: authority.authoritySource === 'persisted'
          ? 'The saved launcher root is unavailable; restore access or select a Pumas library to recover.'
          : 'The selected launcher root is unavailable; restore access or select a Pumas library to recover.',
      });
    }
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('malformed explicit and persisted path values are invalid, not unavailable', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-root-'));

  try {
    const userDataPath = join(fixtureRoot, 'user-data');
    mkdirSync(userDataPath, { recursive: true });
    const baseOptions = {
      devRoot: join(fixtureRoot, 'development-default'),
      execPath: join(fixtureRoot, 'pumas-library'),
      isPackaged: true,
      userDataPath,
    };

    assert.deepEqual(resolveLauncherRoot({
      ...baseOptions,
      argv: ['pumas-library', '--launcher-root', 'invalid\0argument'],
      env: {},
    }), {
      status: 'recovery-required',
      code: 'launcher_root_invalid',
      authoritySource: 'argument',
      persistedState: 'not-consulted',
      message: 'The selected launcher root is invalid; select an existing Pumas library to recover.',
    });

    writeFileSync(
      launcherRootOverrideConfigPath(userDataPath),
      `${JSON.stringify({ launcherRoot: 'invalid\0persisted' })}\n`,
      'utf8'
    );
    assert.deepEqual(resolveLauncherRoot({
      ...baseOptions,
      argv: [],
      env: {},
    }), {
      status: 'recovery-required',
      code: 'launcher_root_invalid',
      authoritySource: 'persisted',
      persistedState: 'invalid',
      message: 'The saved launcher root is invalid; select an existing Pumas library to recover.',
    });
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('authoritative arbitrary descendants cannot normalize to a launcher-root ancestor', () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-root-'));

  try {
    const launcherRoot = join(fixtureRoot, 'launcher-root');
    const invalidDescendant = join(launcherRoot, 'not-an-authority-selection');
    const userDataPath = join(fixtureRoot, 'user-data');
    createLauncherRoot(launcherRoot);
    mkdirSync(userDataPath, { recursive: true });

    const baseOptions = {
      devRoot: join(fixtureRoot, 'development-default'),
      execPath: join(launcherRoot, 'bin', 'pumas-library'),
      isPackaged: true,
      userDataPath,
    };
    const cases = [
      {
        authoritySource: 'argument',
        argv: ['pumas-library', '--launcher-root', invalidDescendant],
        env: {},
        persistedState: 'not-consulted',
        message: 'The selected launcher root is invalid; select an existing Pumas library to recover.',
      },
      {
        authoritySource: 'environment',
        argv: [],
        env: { PUMAS_LAUNCHER_ROOT: invalidDescendant },
        persistedState: 'not-consulted',
        message: 'The selected launcher root is invalid; select an existing Pumas library to recover.',
      },
      {
        authoritySource: 'persisted',
        argv: [],
        env: {},
        persistedState: 'invalid',
        message: 'The saved launcher root is invalid; select an existing Pumas library to recover.',
      },
    ];

    for (const authority of cases) {
      if (authority.authoritySource === 'persisted') {
        writeFileSync(
          launcherRootOverrideConfigPath(userDataPath),
          `${JSON.stringify({ launcherRoot: invalidDescendant })}\n`,
          'utf8'
        );
      }

      assert.deepEqual(resolveLauncherRoot({
        ...baseOptions,
        argv: authority.argv,
        env: authority.env,
      }), {
        status: 'recovery-required',
        code: 'launcher_root_invalid',
        authoritySource: authority.authoritySource,
        persistedState: authority.persistedState,
        message: authority.message,
      });
    }
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

for (const explicitCase of [
  {
    name: 'missing argument value',
    argv: ['pumas-library', '--launcher-root'],
    env: {},
    authoritySource: 'argument',
  },
  {
    name: 'argument value replaced by another flag',
    argv: ['pumas-library', '--launcher-root', '--debug'],
    env: {},
    authoritySource: 'argument',
  },
  {
    name: 'blank inline argument value',
    argv: ['pumas-library', '--launcher-root=   '],
    env: {},
    authoritySource: 'argument',
  },
  {
    name: 'blank environment value',
    argv: [],
    env: { PUMAS_LAUNCHER_ROOT: '   ' },
    authoritySource: 'environment',
  },
]) {
  test(`present ${explicitCase.name} is invalid instead of absent`, () => {
    const fixtureRoot = mkdtempSync(join(tmpdir(), 'pumas-launcher-root-'));

    try {
      const discoveryRoot = join(fixtureRoot, 'discovery-root');
      createLauncherRoot(discoveryRoot);

      assert.deepEqual(resolveLauncherRoot({
        argv: explicitCase.argv,
        devRoot: join(fixtureRoot, 'development-default'),
        env: explicitCase.env,
        execPath: join(discoveryRoot, 'bin', 'pumas-library'),
        isPackaged: true,
        userDataPath: join(fixtureRoot, 'user-data'),
      }), {
        status: 'recovery-required',
        code: 'launcher_root_invalid',
        authoritySource: explicitCase.authoritySource,
        persistedState: 'not-consulted',
        message: 'The selected launcher root is invalid; select an existing Pumas library to recover.',
      });
    } finally {
      rmSync(fixtureRoot, { recursive: true, force: true });
    }
  });
}
