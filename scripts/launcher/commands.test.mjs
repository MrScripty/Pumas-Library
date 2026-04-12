import assert from 'node:assert/strict';
import test from 'node:test';
import { runBoundedCommand } from './commands.mjs';

test('runBoundedCommand succeeds when the process stays alive for the smoke window', async () => {
  await runBoundedCommand(
    process.execPath,
    ['-e', 'setTimeout(() => process.exit(0), 150)'],
    {
      minUptimeMs: 100,
      maxUptimeMs: 2_000,
    }
  );
});

test('runBoundedCommand rejects when the process exits before the minimum smoke window', async () => {
  await assert.rejects(
    runBoundedCommand(
      process.execPath,
      ['-e', 'process.exit(0)'],
      {
        minUptimeMs: 100,
        maxUptimeMs: 2_000,
      }
    ),
    /minimum smoke window/
  );
});
