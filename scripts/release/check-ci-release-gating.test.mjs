import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const workflow = fs.readFileSync(path.join(root, '.github/workflows/build.yml'), 'utf8');
const versionTagGuard = "github.ref_type == 'tag' && startsWith(github.ref_name, 'v')";
const escapedVersionTagGuard = versionTagGuard.replaceAll(/[.*+?^${}()|[\]\\]/g, '\\$&');

test('ONNX graph gate rejects transitive download unification and linked runtime mode', () => {
  const check = spawnSync('python', ['-c', String.raw`
import importlib.util
spec = importlib.util.spec_from_file_location('features', 'scripts/release/check-dependency-features.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
for graph in (
    'ort v2.0.0-rc.12|api-24,load-dynamic,download-binaries',
    'ort-sys v2.0.0-rc.12|api-24,disable-linking,download-binaries',
    'ort v2.0.0-rc.12|api-24',
):
    try:
        module.check_ort_features(graph, 'transitive regression')
    except RuntimeError:
        pass
    else:
        raise AssertionError('unsafe graph accepted')
module.check_ort_features('ort v2.0.0-rc.12|api-24,load-dynamic\n\nort-sys v2.0.0-rc.12|api-24,disable-linking', 'accepted')
`], { cwd: root, encoding: 'utf8', timeout: 10_000 });
  assert.equal(check.status, 0, check.stderr || String(check.error));
});

test('Build runs for pull requests against every review base', () => {
  assert.match(workflow, /^  pull_request: \{\}$/m);
});

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
    'headless-inference',
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

test('review inference candidate is bounded, source-pinned and never publishes a release', () => {
  const review = job('review-inference-linux');
  assert.match(review, /^    if: github\.event_name == 'pull_request' \|\| github\.event_name == 'workflow_dispatch'$/m);
  assert.match(review, /^    needs: lint-workflows$/m);
  assert.match(review, /^    runs-on: ubuntu-24\.04$/m);
  assert.match(review, /^    timeout-minutes: 60$/m);
  assert.match(review, /ref: \$\{\{ github\.event\.pull_request\.head\.sha \|\| github\.sha \}\}/);
  assert.match(review, /persist-credentials: false/);
  assert.match(review, /EXPECTED_SOURCE_SHA: \$\{\{ github\.event\.pull_request\.head\.sha \|\| github\.sha \}\}/);
  assert.match(review, /test "\$\(git rev-parse HEAD\)" = "\$EXPECTED_SOURCE_SHA"/);
  assert.match(review, /CARGO_BUILD_JOBS: '1'/);
  assert.match(review, /ORT_SKIP_DOWNLOAD: '1'/);
  assert.match(review, /cargo fetch --locked --manifest-path rust\/Cargo\.toml/);
  assert.match(review, /onnx_runtime_stage\.py --target linux-x86_64 --download/);
  assert.match(review, /onnx_runtime_probe\.py --target linux-x86_64/);
  assert.match(review, /headless_inference_ci\.py --target linux-x86_64/);
  assert.match(review, /check-artifacts\.mjs .* headless-inference-linux/);
  assert.match(review, /name: review-inference-linux-x86_64/);
  assert.match(review, /name: Preserve bounded review build and startup evidence\n        if: always\(\)/);
  assert.match(review, /name: review-inference-evidence-linux-x86_64/);
  assert.equal((review.match(/retention-days: 7/g) ?? []).length, 2);
  assert.doesNotMatch(review, /secrets\.|permissions:|pull_request_target|gh release|git push|CARGO_PROFILE_RELEASE_|RUSTFLAGS/);
  assert.doesNotMatch(job('release-candidate'), /review-inference/);
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


test('native workspace custody and cleanup run on every native QA platform', () => {
  const native = job('torch-quality');
  assert.match(native, /os: \[ubuntu-24\.04, windows-2025, macos-15\]/);
  for (const [name, command] of [
    ['Test reserved acquisition workspace custody', 'cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library acquisition::workspace::tests'],
    ['Test native acquisition workspace cleanup', 'cargo test --locked --manifest-path rust/Cargo.toml -p pumas-app-manager native_'],
  ]) {
    assert.ok(native.includes(`      - name: ${name}\n        run: ${command}\n`),
      `${name} must run without a platform-specific skip`);
  }
});
test('headless owner identity checks include native Windows on ordinary PRs', () => {
  const native = job('torch-quality');
  assert.match(native, /os: \[ubuntu-24\.04, windows-2025, macos-15\]/);
  assert.doesNotMatch(native, /^    if:/m);
  assert.ok(native.includes(
    '      - name: Test native headless owner root identity\n'
    + '        working-directory: scripts/release\n'
    + '        run: python -m unittest -v test_headless_inference.OwnerContractTests\n',
  ), 'owner identity tests must run without a platform or release skip');
});

// PowerShell's final native exit code must not mask a preceding failing test.
test('native baseline gates keep each cargo command in its own step', () => {
  const native = job('torch-quality');
  for (const name of [
    'Test native import integrity',
    'Test native shard completeness',
    'Test native registry startup ownership',
    'Test native failed-start claim release',
  ]) {
    assert.ok(native.includes(`      - name: ${name}\n        run: cargo test --locked `), name);
  }
});

test('completed RPC qualification has no persistent PR-specific publication gate', () => {
  assert.doesNotMatch(workflow, /github\.event\.pull_request\.number == 25/);
  assert.doesNotMatch(job('headless'), /package-rpc-qualification\.mjs|rpc-qualification\//);
});


test('cross-owner acquisition gates reject zero matches on every native host', () => {
  const native = job('torch-quality');
  for (const args of [
    '--package pumas-library --minimum 5',
    '--package pumas-library --no-default-features --features hf-client --minimum 5',
    '--package pumas-rpc --features test-support --minimum 2',
    '--package pumas-rpc --no-default-features --features test-support --minimum 1',
  ]) {
    assert.ok(native.includes(`        run: python scripts/release/test-acquisition-integration.py ${args}\n`));
  }
  assert.ok(job('headless').includes('-p pumas-library --no-default-features --features hf-client'));
});

test('release build commands never enable test-support or all-features', () => {
  for (const name of ['headless-rpc', 'build-rust', 'windows-release', 'windows-no-inference']) {
    for (const line of job(name).split('\n').filter(line => line.includes('cargo build'))) {
      assert.doesNotMatch(line, /--all-features|test-support/);
    }
  }
  const manager = fs.readFileSync(path.join(root, 'rust/crates/pumas-app-manager/Cargo.toml'), 'utf8');
  const rpc = fs.readFileSync(path.join(root, 'rust/crates/pumas-rpc/Cargo.toml'), 'utf8');
  assert.match(manager, /^default = \[\]$/m);
  assert.match(rpc, /^test-support = \["pumas-app-manager\?\/test-support"\]$/m);
  assert.match(rpc, /^default = \["inference-plugins"\]$/m);
});
