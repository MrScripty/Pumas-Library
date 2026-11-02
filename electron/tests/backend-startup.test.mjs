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

test('startup diagnostic ignores malformed, unsupported-version and oversized frames', () => {
  for (const line of [
    'PUMAS_STARTUP_FAILURE=not json\n',
    'PUMAS_STARTUP_FAILURE={"version":2,"reason":"migration-required"}\n',
    'PUMAS_STARTUP_FAILURE={"reason":"migration-required"}\n',
    `${'x'.repeat(1025)}${frame}`,
    frame.trimEnd(),
  ]) {
    const reader = new BackendStartupFailureReader();
    reader.push(line);
    assert.equal(reader.reason, 'backend-unavailable');
    assert.equal(reader.detail, undefined);
    reader.push(`\n${frame}`);
    assert.equal(reader.reason, 'migration-required');
  }
});

test('startup diagnostic carries the backend explanation', () => {
  const reader = new BackendStartupFailureReader();
  reader.push('PUMAS_STARTUP_FAILURE={"version":1,"reason":"backend-unavailable","detail":"unknown field `hf_model_partition_version`"}\n');
  assert.equal(reader.reason, 'backend-unavailable');
  assert.equal(reader.detail, 'unknown field `hf_model_partition_version`');
});

test('a backend from another build is read tolerantly', () => {
  // Unknown fields are ignored and never surfaced.
  const extra = new BackendStartupFailureReader();
  extra.push('PUMAS_STARTUP_FAILURE={"version":1,"reason":"migration-required","path":"/private"}\n');
  assert.equal(extra.reason, 'migration-required');
  assert.equal(extra.detail, undefined);
  // An unknown reason still reports the failure and its explanation.
  const unknown = new BackendStartupFailureReader();
  unknown.push('PUMAS_STARTUP_FAILURE={"version":1,"reason":"store-locked","detail":"locked by pid 7"}\n');
  assert.equal(unknown.reason, 'backend-unavailable');
  assert.equal(unknown.detail, 'locked by pid 7');
});
