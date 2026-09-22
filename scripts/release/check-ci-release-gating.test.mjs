import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const workflow = fs.readFileSync(path.join(root, '.github/workflows/build.yml'), 'utf8');
const versionTagGuard = "github.ref_type == 'tag' && startsWith(github.ref_name, 'v')";
const escapedVersionTagGuard = versionTagGuard.replaceAll(/[.*+?^${}()|[\]\\]/g, '\\$&');

function job(name) {
  const markers = [...workflow.matchAll(/^  ([a-z][a-z0-9-]+):\n/gm)];
  const index = markers.findIndex(marker => marker[1] === name);
  assert.notEqual(index, -1, `Build workflow is missing job ${name}`);
  const start = markers[index].index;
  const end = markers[index + 1]?.index ?? workflow.length;
  return workflow.slice(start, end);
}

test('release-producing jobs run only for version tags', () => {
  for (const name of [
    'headless-rpc',
    'build-rust',
    'build-electron',
    'build-electron-no-inference',
    'windows-release',
    'windows-no-inference',
    'verify-launcher',
    'release-candidate',
  ]) {
    assert.match(job(name), new RegExp(`^    if: ${escapedVersionTagGuard}$`, 'm'));
  }
});

test('shared quality jobs remain available to pull requests and ordinary pushes', () => {
  for (const name of ['lint-workflows', 'rust-quality', 'headless', 'build-frontend', 'torch-quality']) {
    assert.doesNotMatch(job(name), /^    if:/m, `${name} must not be job-gated to version tags`);
  }
});

test('release builds and frontend artifact upload are guarded inside shared jobs', () => {
  const headless = job('headless');
  assert.match(headless, new RegExp(`Build inference-disabled release backend\\n        if: ${escapedVersionTagGuard}`));
  assert.match(headless, new RegExp(`Verify inference-disabled release backend\\n        if: ${escapedVersionTagGuard}`));

  const frontend = job('build-frontend');
  for (const step of ['Build library-only renderer', 'Build default renderer']) {
    assert.match(frontend, new RegExp(`${step}\\n        if: ${escapedVersionTagGuard}`));
  }
  assert.match(
    frontend,
    new RegExp(`uses: actions/upload-artifact@v7\\.0\\.1\\n        if: ${escapedVersionTagGuard}`),
  );
});
