import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { INFERENCE_ONLY_MARKERS, checkFrontendVariant } from './check-frontend-variant.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

function bundle(t, body) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-frontend-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.mkdirSync(path.join(directory, 'assets'));
  fs.writeFileSync(path.join(directory, 'index.html'), '<script type="module" crossorigin src="./assets/index.js"></script>');
  fs.writeFileSync(path.join(directory, 'assets', 'index.js'), body);
  return directory;
}

test('each marker exists only in its replaced inference module', () => {
  const vite = fs.readFileSync(path.join(root, 'frontend/vite.config.ts'), 'utf8');
  for (const marker of INFERENCE_ONLY_MARKERS) {
    assert.ok(fs.readFileSync(path.join(root, marker.source), 'utf8').includes(marker.text), marker.text);
    // The module is one the library-only mode swaps for a stub.
    assert.ok(vite.includes(`'./src/components/${path.basename(marker.source)}'`), marker.source);
  }
});

test('library-only rejects inference code; default requires it', t => {
  const withInference = bundle(t, INFERENCE_ONLY_MARKERS.map(marker => `"${marker.text}"`).join(';'));
  const without = bundle(t, 'console.log("library")');
  assert.equal(checkFrontendVariant(without, 'library-only'), INFERENCE_ONLY_MARKERS.length);
  assert.throws(() => checkFrontendVariant(withInference, 'library-only'), /contains inference-only code/);
  assert.equal(checkFrontendVariant(withInference, 'default'), INFERENCE_ONLY_MARKERS.length);
  assert.throws(() => checkFrontendVariant(without, 'default'), /lacks inference-plugin code/);
  assert.throws(() => checkFrontendVariant(without, 'other'), /Unknown frontend variant/);
});

test('a bundle must load the scripts it references', t => {
  const directory = bundle(t, '');
  fs.rmSync(path.join(directory, 'assets', 'index.js'));
  assert.throws(() => checkFrontendVariant(directory, 'library-only'), /references missing assets\/index\.js/);
  fs.writeFileSync(path.join(directory, 'index.html'), '<html></html>');
  assert.throws(() => checkFrontendVariant(directory, 'library-only'), /references no relative script/);
  fs.rmSync(path.join(directory, 'index.html'));
  assert.throws(() => checkFrontendVariant(directory, 'library-only'), /no index\.html/);
});
