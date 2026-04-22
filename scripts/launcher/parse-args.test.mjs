import test from 'node:test';
import assert from 'node:assert/strict';
import { parseArgs } from './parse-args.mjs';
import { buildUsage } from './contract.mjs';
import { createPlatformService } from './platform-service.mjs';

test('parseArgs accepts install action without forwarded args', () => {
  const result = parseArgs(['--install']);
  assert.deepEqual(result, { action: '--install', forwardedArgs: [] });
});

test('parseArgs accepts release smoke action without forwarded args', () => {
  const result = parseArgs(['--release-smoke']);
  assert.deepEqual(result, { action: '--release-smoke', forwardedArgs: [] });
});

test('parseArgs preserves forwarded args for run action', () => {
  const result = parseArgs(['--run', '--', '--devtools', 'value with spaces']);
  assert.deepEqual(result, {
    action: '--run',
    forwardedArgs: ['--devtools', 'value with spaces'],
  });
});

test('parseArgs rejects positional arguments', () => {
  assert.throws(
    () => parseArgs(['run']),
    /unknown argument: run/
  );
});

test('parseArgs rejects multiple action flags', () => {
  assert.throws(
    () => parseArgs(['--build', '--run']),
    /only one action flag is allowed/
  );
});

test('parseArgs rejects passthrough delimiter for non-runtime actions', () => {
  assert.throws(
    () => parseArgs(['--build', '--', '--debug']),
    /only valid with --run or --run-release/
  );
});

test('buildUsage advertises the shared launcher contract', () => {
  const usage = buildUsage('./launcher.sh');
  assert.match(usage, /--build-release/);
  assert.match(usage, /--run-release/);
  assert.match(usage, /--test/);
  assert.match(usage, /--release-smoke/);
});

test('platform factory resolves the Windows Corepack command separately', () => {
  const windows = createPlatformService('win32');
  const linux = createPlatformService('linux');

  assert.equal(windows.corepackCommand, 'corepack.cmd');
  assert.equal(linux.corepackCommand, 'corepack');
});

test('platform factory resolves Python module invocations per host', () => {
  const windows = createPlatformService('win32');
  const linux = createPlatformService('linux');

  assert.equal(windows.pythonCommand, 'py.exe');
  assert.deepEqual(windows.pythonModuleArgs('unittest', ['discover']), ['-3', '-m', 'unittest', 'discover']);
  assert.equal(linux.pythonCommand, 'python3');
  assert.deepEqual(linux.pythonModuleArgs('unittest', ['discover']), ['-m', 'unittest', 'discover']);
});
