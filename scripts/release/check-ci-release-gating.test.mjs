import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const workflow = fs.readFileSync(path.join(root, '.github/workflows/build.yml'), 'utf8');
const versionTagGuard = "(github.ref_type == 'tag' && startsWith(github.ref_name, 'v')) || (github.event_name == 'workflow_dispatch' && inputs.build_candidates)";
const escapedVersionTagGuard = versionTagGuard.replaceAll(/[.*+?^${}()|[\]\\]/g, '\\$&');

test('ONNX graph gate rejects transitive download unification and linked runtime mode', () => {
  const check = spawnSync(process.env.PYTHON ?? (process.platform === 'win32' ? 'python' : 'python3'), ['-c', String.raw`
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

test('candidate-producing jobs require a version tag or explicit build-only dispatch', () => {
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

test('manual candidate dispatch is opt-in, source-pinned and cannot publish', () => {
  assert.match(workflow, /build_candidates:\n        description: .*\n        type: boolean\n        default: false/);
  assert.match(job('lint-workflows'), /EXPECTED_SOURCE: \$\{\{ inputs.expected_source \}\}/);
  assert.match(job('lint-workflows'), /re.fullmatch\('\[0-9a-f\]\{40\}', expected\).*expected != actual/);
  assert.match(workflow, /permissions:\n  contents: read/);
  assert.doesNotMatch(workflow, /contents: write|gh release|git tag|git push|create-release|publish: always/);
  assert.match(job('release-candidate'), /write-release-metadata.py release-candidate --build-id/);
  for (const name of ['review-inference-linux', 'torch-native-e2e']) {
    assert.ok(job(name).includes("github.event_name == 'workflow_dispatch' && !inputs.build_candidates"));
  }
});

test('manual source guard accepts only its exact Git commit', () => {
  const python = process.env.PYTHON ?? (process.platform === 'win32' ? 'python' : 'python3');
  const head = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).stdout.trim();
  const code = job('lint-workflows').match(/python - <<'PYTHON'\n([\s\S]*?)          PYTHON/)[1]
    .replace(/^          /gm, '');
  const different = (head[0] === 'a' ? 'b' : 'a') + head.slice(1);
  for (const [source, accepted] of [[head, true], ['', false], ['not-a-commit', false], [different, false]]) {
    const result = spawnSync(python, ['-c', code], {
      cwd: root, encoding: 'utf8', timeout: 10_000, env: { ...process.env, EXPECTED_SOURCE: source },
    });
    assert.equal(result.status === 0, accepted, result.stderr);
  }
});

test('no package bundles a runtime; Pumas installs ONNX Runtime at runtime', () => {
  for (const name of ['build-rust', 'windows-release']) {
    assert.match(job(name), /python scripts\/release\/stage-rpc\.py\n/);
  }
  // Headless archives package no runtime either: nothing stages, probes or
  // pins an ONNX Runtime library, anywhere in the workflow.
  assert.doesNotMatch(workflow, /onnx_runtime_stage\.py|onnx_runtime_probe\.py|onnx-runtime-pins|--runtime-dir|native-loader\.json/);
  const inference = job('headless-inference');
  assert.match(inference, /binary: pumas-rpc.exe/);
  assert.match(inference, /headless_inference_ci\.py --target \$\{\{ matrix\.target \}\} --output-dir/);
  assert.match(inference, /name: Verify untrained model lifecycle on ONNX Runtime installed by the packaged Pumas\n        timeout-minutes: 20/);
  assert.match(inference, /verify-packaged-onnx.py .*matrix.binary/);
  assert.match(job('torch-quality'), /python -m unittest -v test_inference_release test_release_metadata/);
});

test('shared quality jobs remain available to pull requests and ordinary pushes', () => {
  for (const name of ['lint-workflows', 'rust-quality', 'headless', 'build-frontend', 'torch-quality']) {
    assert.doesNotMatch(job(name), /^    if:/m, `${name} must not be job-gated to version tags`);
  }
});

// Review jobs run for pull requests, the merged commit on main and ordinary
// manual dispatches, never for candidate runs.
const reviewGuard = "    if: github.event_name == 'pull_request' || (github.event_name == 'push' && github.ref == 'refs/heads/main') || (github.event_name == 'workflow_dispatch' && !inputs.build_candidates)\n";

test('review inference candidate is bounded, source-pinned and never publishes a release', () => {
  const review = job('review-inference-linux');
  assert.ok(review.includes(reviewGuard));
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
  assert.match(review, /headless_inference_ci\.py --target linux-x86_64 --output-dir/);
  assert.match(review, /check-artifacts\.mjs .* headless-inference-linux/);
  assert.match(review, /name: Verify extracted Pumas installs ONNX Runtime and runs an untrained synthetic ONNX graph\n        timeout-minutes: 20/);
  assert.match(review, /verify_synthetic_packaged_onnx\.py --candidate-dir .* --expected-source "\$EXPECTED_SOURCE_SHA"/);
  assert.ok(review.indexOf('verify_synthetic_packaged_onnx.py') < review.indexOf('uses: actions/upload-artifact@'));
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
  assert.match(
    frontend,
    new RegExp(`uses: actions/upload-artifact@[0-9a-f]{40}(?: # \\S+)?\\n        if: ${escapedVersionTagGuard}`),
  );
});

test('both renderer variants build and are checked on every trigger, apart from the packaging input', () => {
  const frontend = job('build-frontend');
  const steps = [
    // Vitest suites for the library-only replacements (frontend/vitest.library-only.config.ts).
    ['Test library-only renderer behavior', 'pnpm --dir frontend test:library-only'],
    ['Build library-only renderer', 'pnpm --dir frontend build:library-only --outDir "${{ runner.temp }}/frontend-library-only" --emptyOutDir'],
    ['Verify library-only renderer excludes inference plugins', 'node scripts/release/check-frontend-variant.mjs "${{ runner.temp }}/frontend-library-only" library-only'],
    ['Build default renderer', 'pnpm --dir frontend build'],
    ['Verify default renderer includes inference plugins', 'node scripts/release/check-frontend-variant.mjs frontend/dist default'],
  ];
  let previous = -1;
  for (const [name, run] of steps) {
    const step = `      - name: ${name}\n        run: ${run}\n`;
    assert.ok(frontend.includes(step), `${name} must run unguarded`);
    assert.ok(frontend.indexOf(step) > previous, `${name} is out of order`);
    previous = frontend.indexOf(step);
  }
  // The uploaded packaging input is the default renderer, never library-only.
  assert.match(frontend, /name: frontend-dist\n          path: frontend\/dist\//);
  assert.ok(previous < frontend.indexOf('uses: actions/upload-artifact@'));
});


const nativeQualification = fs.readFileSync(path.join(root, 'scripts/rust/native_qualification.py'), 'utf8');
const nativeShards = ['torch', 'default-features', 'inference-disabled'];

test('every native shard runs on every native QA platform', () => {
  const native = job('torch-quality');
  assert.match(native, /os: \[ubuntu-24\.04, windows-2025, macos-15\]/);
  assert.match(native, new RegExp(`shard: \\[${nativeShards.join(', ')}\\]`));
  // The runner step has no platform or shard skip; the shard picks its groups.
  assert.ok(native.includes(
    '      - name: Test native Rust behavior (${{ matrix.shard }})\n'
    + '        run: python scripts/rust/native_qualification.py --shard ${{ matrix.shard }}\n',
  ));
  for (const shard of nativeShards) {
    assert.match(nativeQualification, new RegExp(`Group\\(\\s+"${shard}",`), shard);
  }
});

test('native workspace custody and cleanup run on every native QA platform', () => {
  for (const filter of ['acquisition::workspace::tests', 'native_']) {
    assert.ok(nativeQualification.includes(`f("${filter}"),`), `${filter} must run without a host restriction`);
  }
});

test('headless owner identity checks include native Windows on ordinary PRs', () => {
  const native = job('torch-quality');
  assert.match(native, /os: \[ubuntu-24\.04, windows-2025, macos-15\]/);
  assert.doesNotMatch(native, /^    if:/m);
  // Only the shard that runs the Python suites selects it; no platform or release skip.
  assert.ok(native.includes(
    '      - name: Test native headless owner root identity\n'
    + "        if: matrix.shard == 'torch'\n"
    + '        working-directory: scripts/release\n'
    + '        run: python -m unittest -v test_headless_inference.OwnerContractTests\n',
  ), 'owner identity tests must run without a platform or release skip');
});

// A shell's final exit code must not mask a preceding failing test: native
// cargo runs go through the runner, which checks every exit status.
test('native cargo tests run only through the status-checking runner', () => {
  const native = job('torch-quality');
  assert.doesNotMatch(native, /run: cargo test/);
  assert.doesNotMatch(native, /shell: pwsh/);
  assert.match(nativeQualification, /if executed\.returncode != 0:/);
  assert.match(nativeQualification, /if listed\.returncode != 0:/);
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


test('dependency audit is advisory on reviews, blocking on candidates and feeds the release candidate', () => {
  const audit = job('dependency-audit');
  assert.match(audit, /^    needs: lint-workflows$/m);
  assert.doesNotMatch(audit, /^    if:/m, 'the scan runs for every trigger');
  // D6: a failing scan never fails a review run, and always fails a candidate run.
  // The scan step, not the job, is advisory, so a review run's check stays green
  // and carries a warning instead of showing as failed.
  assert.doesNotMatch(audit, /^    continue-on-error:/m, 'the job itself never ignores a failure');
  const scan = audit.slice(audit.indexOf('Scan Rust, JavaScript and Python dependency inputs'));
  assert.match(scan, /^        id: scan$/m);
  assert.ok(scan.includes(`        continue-on-error: \${{ !(${versionTagGuard}) }}\n`));
  const report = audit.slice(audit.indexOf('Report an advisory scan failure'));
  assert.match(report, /^        if: steps\.scan\.outcome == 'failure'$/m);
  assert.match(report, /::warning /);
  assert.match(audit, /persist-credentials: false/);
  assert.doesNotMatch(audit, /secrets\.|permissions:|cargo install|pip install (?!--require-hashes)/);
  assert.match(audit, /pip install --require-hashes -r scripts\/dev\/pip-audit-python\.lock/);
  assert.match(audit, /cargo-deny\/releases\/download\/(\d+\.\d+\.\d+)\/cargo-deny-\1-x86_64-unknown-linux-musl\.tar\.gz/);
  assert.match(audit, /echo "[0-9a-f]{64} {2}\$tool_dir\/cargo-deny\.tar\.gz" \| sha256sum --check/);
  assert.ok(audit.includes('node scripts/release/dependency-audit.mjs run --output "${{ runner.temp }}/current-vulnerability-results"'));
  const upload = audit.slice(audit.indexOf('Upload current vulnerability results'));
  assert.match(upload, /^        if: always\(\)$/m);
  assert.match(upload, /^          name: current-vulnerability-results$/m);
  assert.match(upload, /if-no-files-found: error/);

  // The artifact name is the review the release plan requires.
  const plan = JSON.parse(fs.readFileSync(path.join(root, 'scripts/release/artifact-plan.json'), 'utf8'));
  assert.ok(plan.release_metadata.required_reviews.includes('current-vulnerability-results'));

  const candidate = job('release-candidate');
  assert.match(candidate, /^    needs: \[dependency-audit, /m);
  assert.match(candidate, /name: current-vulnerability-results\n/);
  const verify = 'check-artifacts.mjs "${{ runner.temp }}/current-vulnerability-results" current-vulnerability-results "${{ github.sha }}"';
  assert.ok(candidate.includes(verify));
  assert.ok(candidate.indexOf(verify) < candidate.indexOf('write-release-metadata.py'));
});

test('every checkout drops its credentials; no job pushes', () => {
  for (const file of ['build.yml', 'scorecard.yml']) {
    const text = fs.readFileSync(path.join(root, '.github/workflows', file), 'utf8');
    assert.doesNotMatch(text, /git push|contents: write/, file);
    const checkouts = [...text.matchAll(/uses: actions\/checkout@[0-9a-f]{40} # \S+\n((?: {8,}.*\n)*)/g)];
    assert.ok(checkouts.length > 0, file);
    for (const [, block] of checkouts) assert.match(block, /persist-credentials: false/, file);
  }
});

test('Python versions come from a version file, never a floating literal', () => {
  assert.doesNotMatch(workflow, /python-version: /);
  const files = new Set([...workflow.matchAll(/python-version-file: (\S+)/g)].map(match => match[1]));
  assert.deepEqual([...files].sort(), ['.python-version', 'scripts/release/acceptance.python-version']);
  for (const file of files) {
    assert.match(fs.readFileSync(path.join(root, file), 'utf8'), /^\d+\.\d+\.\d+\n$/, `${file} pins an exact version`);
  }
  // The acceptance tooling needs compression.zstd (CPython 3.14+).
  assert.match(fs.readFileSync(path.join(root, 'scripts/release/acceptance.python-version'), 'utf8'), /^3\.1[4-9]\./);
  assert.match(fs.readFileSync(path.join(root, 'scripts/release/torch-managed-python-acceptance.py'), 'utf8'), /sys\.version_info >= \(3, 14\)/);
});

test('pushes to main run every gate a pull request runs', () => {
  const jobs = [...workflow.matchAll(/^  ([a-z][a-z0-9-]+):\n/gm)].map(match => match[1]);
  for (const name of jobs) {
    const guard = job(name).match(/^    if: (.*)$/m)?.[1];
    if (!guard || !guard.includes("github.event_name == 'pull_request'")) continue;
    assert.ok(guard.includes("(github.event_name == 'push' && github.ref == 'refs/heads/main')"), `${name} skips pushes to main`);
  }
});

test('the desktop contract has one tracked copy, checked against a fresh export', () => {
  const generated = path.join(root, 'packages/desktop-contract/generated');
  assert.deepEqual(fs.readdirSync(generated).sort(), ['desktop-contract.d.ts', 'desktop-contract.js', 'desktop-contract.validators.js']);
  for (const consumer of ['electron', 'frontend']) {
    assert.ok(!fs.existsSync(path.join(root, consumer, 'src/generated')), `${consumer} keeps its own generated contract copy`);
  }
  assert.match(job('build-frontend'), /^      - run: pnpm --dir electron check:desktop-contract$/m);
});
