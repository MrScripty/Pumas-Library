import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { runBoundedCommand } from './commands.mjs';
import { createPlatformService } from './platform-service.mjs';

const processTree = createPlatformService().processTree;

test('runBoundedCommand succeeds when the process stays alive for the smoke window', async () => {
  await runBoundedCommand(
    process.execPath,
    ['-e', 'setTimeout(() => process.exit(0), 150)'],
    {
      minUptimeMs: 100,
      maxUptimeMs: 2_000,
      processTree,
    }
  );
});

test('runBoundedCommand rejects when the process exits before the minimum smoke window', async () => {
  await assert.rejects(
    runBoundedCommand(
      process.execPath,
      ['-e', 'process.exit(0)'],
      {
        // Node startup can exceed 100 ms on a loaded qualification host.
        minUptimeMs: 1_000,
        maxUptimeMs: 2_000,
        processTree,
      }
    ),
    /minimum smoke window/
  );
});

test('runBoundedCommand maps spawn failure to one launcher error', async () => {
  await assert.rejects(
    runBoundedCommand(
      `missing-pumas-launcher-command-${process.pid}`,
      [],
      { maxUptimeMs: 200, processTree }
    ),
    /failed to start/
  );
});

test('runBoundedCommand requires the platform process-tree adapter', async () => {
  await assert.rejects(
    runBoundedCommand(process.execPath, ['-e', 'process.exit(0)']),
    /requires a platform process-tree adapter/
  );
});

test('runBoundedCommand rejects invalid deadline relationships before spawn', async () => {
  await assert.rejects(
    runBoundedCommand(
      process.execPath,
      ['-e', 'process.exit(0)'],
      {
        minUptimeMs: 300,
        maxUptimeMs: 200,
        processTree,
      }
    ),
    /invalid bounded command deadlines/
  );
});

test('runBoundedCommand forcibly closes a process that ignores graceful termination', async () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-launcher-force-'));
  const selfExitMarker = path.join(tempRoot, 'self-exit');
  const terminationRequests = [];
  const observedProcessTree = {
    ...processTree,
    terminate(child, request) {
      terminationRequests.push(request.force);
      if (request.force) {
        child.kill('SIGKILL');
      }
    },
  };

  try {
    await assert.rejects(
      runBoundedCommand(
        process.execPath,
        [
          '-e',
          [
            "const fs = require('node:fs');",
            `setTimeout(() => { fs.writeFileSync(${JSON.stringify(selfExitMarker)}, 'self-exit'); process.exit(0); }, 1_200);`,
          ].join(''),
        ],
        {
          maxUptimeMs: 200,
          terminationGraceMs: 100,
          forceCloseMs: 500,
          processTree: observedProcessTree,
        }
      ),
      /exceeded smoke window/
    );

    assert.deepEqual(terminationRequests, [false, true]);
    assert.equal(fs.existsSync(selfExitMarker), false);
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});

test('runBoundedCommand observes graceful termination before escalation', async () => {
  const terminationRequests = [];
  const observedProcessTree = {
    ...processTree,
    async terminate(child, request) {
      terminationRequests.push(request.force);
      await processTree.terminate(child, request);
    },
  };

  await assert.rejects(
    runBoundedCommand(
      process.execPath,
      ['-e', 'setTimeout(() => process.exit(0), 5_000)'],
      {
        maxUptimeMs: 200,
        terminationGraceMs: 300,
        forceCloseMs: 500,
        processTree: observedProcessTree,
      }
    ),
    /exceeded smoke window/
  );

  assert.deepEqual(
    terminationRequests,
    process.platform === 'win32' ? [false, true] : [false]
  );
});

test('runBoundedCommand joins termination work after the app child closes', async () => {
  let terminationHelperFinished = false;
  const delayedProcessTree = {
    ...processTree,
    terminate(child, { force }) {
      if (!force) {
        return;
      }

      child.kill('SIGKILL');
      return new Promise((resolve) => {
        setTimeout(() => {
          terminationHelperFinished = true;
          resolve();
        }, 100);
      });
    },
  };

  await assert.rejects(
    runBoundedCommand(
      process.execPath,
      ['-e', 'setTimeout(() => process.exit(0), 5_000)'],
      {
        maxUptimeMs: 200,
        terminationGraceMs: 100,
        forceCloseMs: 500,
        processTree: delayedProcessTree,
      }
    ),
    (error) => /exceeded smoke window/.test(error.message)
      && terminationHelperFinished
  );
});

test('runBoundedCommand reports incomplete cleanup within its declared outer bound', async () => {
  let ownedChild;
  const ineffectiveProcessTree = {
    ...processTree,
    terminate(child) {
      ownedChild = child;
    },
  };
  const startedAt = Date.now();

  try {
    await assert.rejects(
      runBoundedCommand(
        process.execPath,
        ['-e', 'setTimeout(() => process.exit(0), 5_000)'],
        {
          maxUptimeMs: 200,
          terminationGraceMs: 100,
          forceCloseMs: 500,
          processTree: ineffectiveProcessTree,
        }
      ),
      /did not close within the forced termination window/
    );

    assert.ok(
      Date.now() - startedAt < 3_000,
      'max + grace + force must settle well before the 5-second child fallback'
    );
  } finally {
    if (ownedChild && isProcessAlive(ownedChild.pid)) {
      ownedChild.kill('SIGKILL');
      await waitUntil(() => !isProcessAlive(ownedChild.pid), 1_000);
    }
  }
});

test('command failures do not disclose forwarded argument values', async () => {
  const secret = 'secret-launcher-token-value';

  await assert.rejects(
    runBoundedCommand(
      process.execPath,
      ['-e', 'process.exit(7)', '--', `--token=${secret}`],
      { maxUptimeMs: 2_000, processTree }
    ),
    (error) => /exited with code 7/.test(error.message)
      && !error.message.includes(secret)
  );
});

test('runBoundedCommand stops a real descendant process with its owned tree', async () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-launcher-tree-'));
  const descendantPidPath = path.join(tempRoot, 'descendant.pid');
  let descendantPid;
  const descendantSource = [
    "process.on('SIGTERM', () => {});",
    'setTimeout(() => process.exit(0), 5_000);',
  ].join('');
  const parentSource = [
    "const { spawn } = require('node:child_process');",
    "const fs = require('node:fs');",
    `const descendant = spawn(process.execPath, ['-e', ${JSON.stringify(descendantSource)}], { stdio: 'ignore' });`,
    `fs.writeFileSync(${JSON.stringify(descendantPidPath)}, String(descendant.pid));`,
    "process.on('SIGTERM', () => {});",
    'setTimeout(() => process.exit(0), 5_000);',
  ].join('');

  try {
    await assert.rejects(
      runBoundedCommand(
        process.execPath,
        ['-e', parentSource],
        {
          maxUptimeMs: 1_000,
          terminationGraceMs: 100,
          forceCloseMs: 1_000,
          processTree,
        }
      ),
      /exceeded smoke window/
    );

    descendantPid = Number.parseInt(fs.readFileSync(descendantPidPath, 'utf8'), 10);
    assert.ok(Number.isInteger(descendantPid));
    assert.equal(
      await waitUntil(() => !isProcessAlive(descendantPid), 1_000),
      true,
      `descendant process ${descendantPid} remained alive after tree termination`
    );
  } finally {
    if (descendantPid && isProcessAlive(descendantPid)) {
      process.kill(descendantPid, 'SIGKILL');
    }
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});

function isProcessAlive(pid) {
  try {
    process.kill(pid, 0);
    if (process.platform === 'linux') {
      // A container's PID 1 may retain an orphan zombie after group SIGKILL.
      // It cannot execute; kill(pid, 0) alone would mistake it for live work.
      const stat = fs.readFileSync(`/proc/${pid}/stat`, 'utf8');
      const state = stat.slice(stat.lastIndexOf(')') + 2).split(' ')[0];
      return state !== 'Z' && state !== 'X';
    }
    return true;
  } catch (error) {
    if (error?.code === 'ESRCH' || error?.code === 'ENOENT') {
      return false;
    }
    throw error;
  }
}

async function waitUntil(predicate, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) {
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  return predicate();
}
