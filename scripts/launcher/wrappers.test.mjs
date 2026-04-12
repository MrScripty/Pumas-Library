import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const moduleDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(moduleDir, '..', '..');
const launcherSh = path.join(repoRoot, 'launcher.sh');
const launcherPs1 = path.join(repoRoot, 'launcher.ps1');

test('launcher.sh stays a thin wrapper over the shared core', () => {
  const contents = fs.readFileSync(launcherSh, 'utf8');

  assert.match(contents, /PUMAS_LAUNCHER_DISPLAY_NAME='\.\/*launcher\.sh'/);
  assert.match(contents, /exec node "\$LAUNCHER_CORE" "\$@"/);
  assert.doesNotMatch(contents, /cargo build/);
  assert.doesNotMatch(contents, /npm --workspace/);
});

test('launcher.ps1 stays a thin wrapper over the shared core', () => {
  const contents = fs.readFileSync(launcherPs1, 'utf8');

  assert.match(contents, /\$env:PUMAS_LAUNCHER_DISPLAY_NAME = '\.\/launcher\.ps1'/);
  assert.match(contents, /& \$nodeCommand\.Source \$launcherCore @args/);
  assert.doesNotMatch(contents, /cargo build/);
  assert.doesNotMatch(contents, /npm --workspace/);
});
