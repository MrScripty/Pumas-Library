import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { createRequire } from 'node:module';
import test from 'node:test';

test('display scope is stable, root-specific, and unavailable without valid identity', async (t) => {
  const { readLibraryDisplayScope } = await import('../dist/library-display-scope.js');
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-display-scope-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const createRoot = (name) => {
    const root = path.join(directory, name);
    fs.mkdirSync(path.join(root, 'shared-resources', 'models'), { recursive: true });
    return root;
  };
  const first = createRoot('first');
  const second = createRoot('second');
  const marker = (root) => path.join(root, 'shared-resources', 'models', '.pumas-library-id.json');
  const identity = JSON.stringify({ schema_version: 1, library_id: '714afbd9-b821-4350-a2e2-fd694ac5bf9a' });
  assert.equal(readLibraryDisplayScope(first), null);
  fs.writeFileSync(marker(first), identity);
  fs.writeFileSync(marker(second), identity);
  const scope = readLibraryDisplayScope(first);
  assert.match(scope, /^display-v1:[a-f0-9]{64}$/);
  assert.equal(readLibraryDisplayScope(first), scope);
  assert.notEqual(readLibraryDisplayScope(second), scope);
  fs.writeFileSync(marker(first), identity.replace('714afbd9', '814afbd9'));
  assert.notEqual(readLibraryDisplayScope(first), scope);
  for (const invalid of ['{}', '{', identity.replace(':1', ':2'), identity.replace('714afbd9', 'not-uuid'), `{"schema_version":0,${identity.slice(1)}`, ' '.repeat(1025)]) {
    fs.writeFileSync(marker(first), invalid);
    assert.equal(readLibraryDisplayScope(first), null);
  }
  fs.unlinkSync(marker(first));
  fs.symlinkSync(marker(second), marker(first));
  assert.equal(readLibraryDisplayScope(first), null);
  fs.unlinkSync(marker(first));
  fs.writeFileSync(marker(first), identity);
  const nativeFs = createRequire(import.meta.url)('node:fs');
  const close = nativeFs.closeSync;
  t.mock.method(nativeFs, 'closeSync', (descriptor) => {
    close(descriptor);
    throw new Error('injected cleanup failure');
  });
  assert.equal(readLibraryDisplayScope(first), null);
});
