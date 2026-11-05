import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';
import {
  backendBinaryName,
  isIgnoredPackagedOverride,
  resolveBackendBinaryPath,
} from '../dist/backend-path.js';

test('backendBinaryName keeps the platform executable convention explicit', () => {
  assert.equal(backendBinaryName('linux'), 'pumas-rpc');
  assert.equal(backendBinaryName('darwin'), 'pumas-rpc');
  assert.equal(backendBinaryName('win32'), 'pumas-rpc.exe');
});

test('resolveBackendBinaryPath honors a launcher-provided override', () => {
  const overridePath = path.join('/repo', 'rust', 'target', 'debug', backendBinaryName('linux'));
  const resolvedOverridePath = path.resolve(overridePath);

  assert.equal(
    resolveBackendBinaryPath({
      defaultBuildProfile: 'release',
      isPackaged: false,
      overridePath,
      platform: 'linux',
      resourcesPath: '/resources',
      sourceRoot: '/repo',
    }),
    resolvedOverridePath
  );
});

test('resolveBackendBinaryPath separates dev and release source builds', () => {
  assert.equal(
    resolveBackendBinaryPath({
      defaultBuildProfile: 'debug',
      isPackaged: false,
      platform: 'linux',
      resourcesPath: '/resources',
      sourceRoot: '/repo',
    }),
    path.join('/repo', 'rust', 'target', 'debug', 'pumas-rpc')
  );

  assert.equal(
    resolveBackendBinaryPath({
      defaultBuildProfile: 'release',
      isPackaged: false,
      platform: 'win32',
      resourcesPath: 'C:\\resources',
      sourceRoot: 'C:\\repo',
    }),
    path.join('C:\\repo', 'rust', 'target', 'release', 'pumas-rpc.exe')
  );
});

test('resolveBackendBinaryPath uses packaged resources for packaged apps', () => {
  assert.equal(
    resolveBackendBinaryPath({
      defaultBuildProfile: 'debug',
      isPackaged: true,
      platform: 'linux',
      resourcesPath: '/opt/Pumas/resources',
      sourceRoot: '/repo',
    }),
    path.join('/opt/Pumas/resources', 'pumas-rpc')
  );
});

test('a packaged app ignores PUMAS_RPC_BINARY and always runs its shipped backend', () => {
  const options = {
    defaultBuildProfile: 'release',
    isPackaged: true,
    overridePath: '/tmp/other-build/pumas-rpc',
    platform: 'linux',
    resourcesPath: '/opt/Pumas/resources',
    sourceRoot: '/repo',
  };
  assert.equal(resolveBackendBinaryPath(options), path.join('/opt/Pumas/resources', 'pumas-rpc'));
  assert.equal(isIgnoredPackagedOverride(options), true);
  assert.equal(isIgnoredPackagedOverride({ ...options, overridePath: '  ' }), false);
  assert.equal(isIgnoredPackagedOverride({ ...options, isPackaged: false }), false);
});
