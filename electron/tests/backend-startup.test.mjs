import assert from 'node:assert/strict';
import test from 'node:test';
import { BackendStartupFailureReader } from '../dist/backend-startup.js';

const frame = 'PUMAS_STARTUP_FAILURE={"version":1,"reason":"migration-required"}\n';

test('startup diagnostic survives every stdout chunk boundary', () => {
  for (let split = 0; split <= frame.length; split += 1) {
    const reader = new BackendStartupFailureReader();
    reader.push(`ordinary log\n${frame.slice(0, split)}`);
    reader.push(frame.slice(split));
    assert.equal(reader.reason, 'migration-required');
  }
});

test('startup diagnostic rejects malformed, unsupported, private and oversized frames', () => {
  for (const line of [
    'PUMAS_STARTUP_FAILURE=not json\n',
    'PUMAS_STARTUP_FAILURE={"version":2,"reason":"migration-required"}\n',
    'PUMAS_STARTUP_FAILURE={"version":1,"reason":"unknown"}\n',
    'PUMAS_STARTUP_FAILURE={"version":1,"reason":"migration-required","path":"/private"}\n',
    `${'x'.repeat(1025)}${frame}`,
    frame.trimEnd(),
  ]) {
    const reader = new BackendStartupFailureReader();
    reader.push(line);
    assert.equal(reader.reason, 'backend-unavailable');
    reader.push(`\n${frame}`);
    assert.equal(reader.reason, 'migration-required');
  }
});
