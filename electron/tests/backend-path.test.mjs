import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';
import {
  backendBinaryName,
  resolveBackendBinaryPath,
} from '../dist/backend-path.js';

test('backendBinaryName keeps the platform executable convention explicit', () => {
  assert.equal(backendBinaryName('linux'), 'pumas-rpc');
  assert.equal(backendBinaryName('darwin'), 'pumas-rpc');
  assert.equal(backendBinaryName('win32'), 'pumas-rpc.exe');
});

test('resolveBackendBinaryPath honors a launcher-provided override', () => {
  const overridePath = path.join('/repo', 'rust', 'target', 'debug', backendBinaryName('linux'));

  assert.equal(
    resolveBackendBinaryPath({
      defaultBuildProfile: 'release',
      isPackaged: false,
      overridePath,
      platform: 'linux',
      resourcesPath: '/resources',
      sourceRoot: '/repo',
    }),
    overridePath
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
