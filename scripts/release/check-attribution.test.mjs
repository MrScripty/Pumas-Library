import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const source = fileURLToPath(new URL('../..', import.meta.url));
const targets = ['x86_64-unknown-linux-gnu', 'x86_64-pc-windows-msvc', 'aarch64-apple-darwin'];
const arms = ['LinuxX8664', 'WindowsX8664', 'MacosArm64'];
const catalogPath = 'scripts/release/licenses/managed-python-sources.json';
const planPath = 'scripts/release/artifact-plan.json';
const pinsPath = 'rust/crates/pumas-app-manager/src/version_manager/managed_python.rs';
const mappingPath = 'scripts/release/torch-managed-python-acceptance.py';
const directory = 'docs/release-attribution/0.7.0';
const hash = data => createHash('sha256').update(data).digest('hex');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-attribution-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const inputs = {};
  const write = (name, data, tracked = false) => {
    const file = path.join(root, name);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, data);
    if (tracked) inputs[name] = hash(data);
  };
  const json = (name, value, tracked = false) => write(name, `${JSON.stringify(value, null, 2)}\n`, tracked);
  const base = ['rust/Cargo.lock', 'rust/Cargo.toml', 'rust/crates/pumas-core/Cargo.toml', 'rust/crates/pumas-rpc/Cargo.toml', 'rust/crates/pumas-app-manager/Cargo.toml', 'pnpm-lock.yaml', 'electron/package.json', 'frontend/package.json'];
  for (const name of base) write(name, '{}', true);
  json('package.json', { version: '0.7.0' });
  json(planPath, { desktop_targets: targets.map(rust_target => ({ rust_target })) }, true);
  const uvHashes = Object.fromEntries(targets.map((target, index) => [target, String.fromCharCode(97 + index).repeat(64)]));
  const flavors = Object.fromEntries(targets.map(target => [target, target.includes('windows') ? 'pgo' : 'pgo+lto']));
  write(pinsPath, `const UV_VERSION: &str = "0.12.18";\nconst UV_BASE_URL: &str = "https://releases.astral.sh/github/uv/releases/download/0.12.18/";\nfn pin(self) -> UvPin {\n${targets.map((target, index) => `    Self::${arms[index]} => UvPin {\n        archive: "uv-${target}.tar.gz",\n        sha256: "${uvHashes[target]}",`).join('\n')}\n    fn accepts_observed`, true);
  write(mappingPath, `REVIEWED_PBS_RELEASE = "20260901"\nFULL_FLAVORS = {\n${targets.map(target => `    "${target}": "${flavors[target]}",`).join('\n')}\n}\n`, true);
  write('scripts/release/licenses/source.txt', 'upstream');
  write(`${directory}/THIRD-PARTY-NOTICES.txt`, 'license text');
  const entries = [];
  const managed = [];
  const packages = [];
  const paths = {};
  for (const target of targets) {
    const reportPath = `docs/plans/torch-cross-platform-runtime-management/reports/v2.14.0-${target}-cpu-rpc-restart-acceptance/runtime.json`;
    const evidence = `docs/plans/torch-cross-platform-runtime-management/reports/managed-python-license-collection/${target}-cpython-3.14.7/managed-python-licenses`;
    const manifestPath = `${evidence}/full-archive-manifest.json`;
    const archiveDir = `${evidence}/full-archive`;
    const pythonPath = `${archiveDir}/PYTHON.json`;
    const legalPath = `${archiveDir}/licenses/LICENSE.cpython.txt`;
    const selected = `https://releases.astral.sh/github/python-build-standalone/releases/download/20260901/cpython-3.14.7%2B20260901-${target}-install_only_stripped.tar.gz`;
    const fullName = `cpython-3.14.7+20260901-${target}-${flavors[target]}-full.tar.zst`;
    const full = `https://github.com/astral-sh/python-build-standalone/releases/download/20260901/${encodeURIComponent(fullName)}`;
    const legal = target.includes('windows') ? 'Windows legal\r\nsecond line\r\n' : 'Legal text\n';
    const python = { target_triple: target, python_version: '3.14.7', build_options: flavors[target], license_path: 'licenses/LICENSE.cpython.txt', licenses: ['Python-2.0'] };
    const pythonBytes = Buffer.from(`${JSON.stringify(python)}\n`);
    write(legalPath, legal, true);
    write(pythonPath, pythonBytes, true);
    json(reportPath, { managed_python: {
      distribution: { implementation: 'CPython', targetTriple: target, version: '3.14.7', catalogKey: `cpython-3.14.7-${target}`, sourceUrl: selected },
      provider: { name: 'uv', version: '0.12.18', archiveSha256: uvHashes[target] },
    } }, true);
    json(manifestPath, {
      target, source_url: selected, archive_url: full, archive_name: fullName, archive_sha256: 'd'.repeat(64),
      python_json: { path: 'PYTHON.json', sha256: hash(pythonBytes), size: pythonBytes.length },
      files: [{ path: 'licenses/LICENSE.cpython.txt', sha256: hash(legal) }],
    }, true);
    entries.push({ target, runtime_report: reportPath, manifest: manifestPath });
    managed.push({ target, runtime_report: reportPath, manifest: manifestPath,
      catalog_key: `cpython-3.14.7-${target}`, selected_install_only_url: selected,
      provider_name: 'uv', provider_version: '0.12.18',
      provider_archive_sha256: uvHashes[target], full_archive_url: full,
      full_archive_sha256: 'd'.repeat(64), python_json: pythonPath,
      legal_files: [{ path: legalPath, sha256: hash(legal) }],
    });
    packages.push({ name: 'CPython full archive', version: '3.14.7', scope: `full archive ${target}`,
      source: full, texts: [{ path: 'licenses/LICENSE.cpython.txt', sha256: hash(legal) }],
    });
    paths[target] = { reportPath, manifestPath, pythonPath, legalPath };
  }
  json(catalogPath, { schema_version: 1, targets: entries }, true);
  const inventory = { schema_version: 1, input_sha256: inputs, notices_sha256: hash('license text'),
    packages, managed_python: managed, sources: [{ file: 'source.txt', sha256: hash('upstream') }],
  };
  json(`${directory}/inventory.json`, inventory);
  fs.copyFileSync(new URL('./check-attribution.cjs', import.meta.url), path.join(root, 'scripts/release/check-attribution.cjs'));
  fs.copyFileSync(path.join(source, '.gitattributes'), path.join(root, '.gitattributes'));
  const run = () => spawnSync(process.execPath, [path.join(root, 'scripts/release/check-attribution.cjs')], { encoding: 'utf8' });
  return { root, run, write, json, inventory, paths };
}

test('attribution survives a Windows-style checkout with original legal bytes', t => {
  const { root, run, inventory, paths } = fixture(t);
  assert.equal(run().status, 0);
  const checkout = path.join(path.dirname(root), `${path.basename(root)}-checkout`);
  t.after(() => fs.rmSync(checkout, { recursive: true, force: true }));
  const git = (args, cwd = root) => {
    const result = spawnSync('git', args, { cwd, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
  };
  git(['init', '--quiet']);
  git(['-c', 'core.autocrlf=false', 'add', '.']);
  git(['-c', 'core.autocrlf=true', 'checkout-index', '--all', `--prefix=${checkout}/`]);
  const checked = spawnSync(process.execPath, [path.join(checkout, 'scripts/release/check-attribution.cjs')], { encoding: 'utf8' });
  assert.equal(checked.status, 0, checked.stderr);
  const windowsLegal = paths['x86_64-pc-windows-msvc'].legalPath;
  assert.deepEqual(fs.readFileSync(path.join(checkout, windowsLegal)), fs.readFileSync(path.join(root, windowsLegal)));
  assert.equal(Object.keys(inventory.input_sha256).length, 12 + targets.length * 4);
});

test('checker rejects stale, missing and mismatched CPython evidence', t => {
  const { root, run, write, json, inventory, paths } = fixture(t);
  assert.equal(run().status, 0);
  const rejectChange = (name, data, pattern) => {
    const original = fs.readFileSync(path.join(root, name));
    write(name, data);
    assert.match(run().stderr, pattern);
    write(name, original);
    assert.equal(run().status, 0);
  };
  rejectChange('rust/Cargo.lock', 'changed', /Stale release attribution/);
  rejectChange(planPath, '{}', /Stale release attribution/);
  rejectChange(catalogPath, '{}', /Stale release attribution/);
  rejectChange(paths[targets[0]].reportPath, '{}', /Stale release attribution/);
  rejectChange(paths[targets[1]].manifestPath, '{}', /Stale release attribution/);
  rejectChange(paths[targets[2]].pythonPath, '{}', /Stale release attribution/);
  rejectChange(paths[targets[1]].legalPath, 'altered\r\n', /Stale release attribution/);
  rejectChange('scripts/release/licenses/source.txt', 'altered', /Changed upstream license/);

  const missing = paths[targets[0]].legalPath;
  fs.unlinkSync(path.join(root, missing));
  assert.notEqual(run().status, 0);
  write(missing, 'Legal text\n');
  delete inventory.input_sha256[missing];
  json(`${directory}/inventory.json`, inventory);
  assert.match(run().stderr, /Stale release attribution/);
  inventory.input_sha256[missing] = hash(fs.readFileSync(path.join(root, missing)));
  const extra = 'scripts/release/licenses/extra.txt';
  write(extra, 'declared input');
  inventory.input_sha256[extra] = hash('declared input');
  json(`${directory}/inventory.json`, inventory);
  assert.equal(run().status, 0);
  write(extra, 'changed declared input');
  assert.match(run().stderr, /Stale release attribution/);
  write(extra, 'declared input');
  const reportPath = paths[targets[0]].reportPath;
  const report = JSON.parse(fs.readFileSync(path.join(root, reportPath)));
  report.managed_python.distribution.sourceUrl += '?different';
  json(reportPath, report);
  inventory.input_sha256[reportPath] = hash(fs.readFileSync(path.join(root, reportPath)));
  json(`${directory}/inventory.json`, inventory);
  assert.match(run().stderr, /selected\/full archive mismatch/);
});

test('checker binds reports to shipped uv pins and the reviewed PBS build', t => {
  const rewrite = (fixtureData, name, value) => {
    fixtureData.json(name, value);
    fixtureData.inventory.input_sha256[name] = hash(fs.readFileSync(path.join(fixtureData.root, name)));
    fixtureData.json(`${directory}/inventory.json`, fixtureData.inventory);
  };
  const target = targets[0];

  const changedPin = fixture(t);
  const pinSource = fs.readFileSync(path.join(changedPin.root, pinsPath), 'utf8');
  changedPin.write(pinsPath, pinSource.replace('a'.repeat(64), 'e'.repeat(64)));
  changedPin.inventory.input_sha256[pinsPath] = hash(fs.readFileSync(path.join(changedPin.root, pinsPath)));
  changedPin.json(`${directory}/inventory.json`, changedPin.inventory);
  assert.match(changedPin.run().stderr, /selected\/full archive mismatch/);

  const changedProvider = fixture(t);
  const providerReport = JSON.parse(fs.readFileSync(path.join(changedProvider.root, changedProvider.paths[target].reportPath)));
  providerReport.managed_python.provider.version = '0.12.19';
  changedProvider.inventory.managed_python[0].provider_version = '0.12.19';
  rewrite(changedProvider, changedProvider.paths[target].reportPath, providerReport);
  assert.match(changedProvider.run().stderr, /selected\/full archive mismatch/);

  const changedHash = fixture(t);
  const hashReport = JSON.parse(fs.readFileSync(path.join(changedHash.root, changedHash.paths[target].reportPath)));
  hashReport.managed_python.provider.archiveSha256 = 'e'.repeat(64);
  changedHash.inventory.managed_python[0].provider_archive_sha256 = 'e'.repeat(64);
  rewrite(changedHash, changedHash.paths[target].reportPath, hashReport);
  assert.match(changedHash.run().stderr, /selected\/full archive mismatch/);

  const changedRelease = fixture(t);
  const releaseReport = JSON.parse(fs.readFileSync(path.join(changedRelease.root, changedRelease.paths[target].reportPath)));
  const releaseManifest = JSON.parse(fs.readFileSync(path.join(changedRelease.root, changedRelease.paths[target].manifestPath)));
  const otherRelease = value => value.replaceAll('20260901', '20261001');
  releaseReport.managed_python.distribution.sourceUrl = otherRelease(releaseReport.managed_python.distribution.sourceUrl);
  releaseManifest.source_url = otherRelease(releaseManifest.source_url);
  releaseManifest.archive_url = otherRelease(releaseManifest.archive_url);
  releaseManifest.archive_name = otherRelease(releaseManifest.archive_name);
  changedRelease.inventory.managed_python[0].selected_install_only_url = releaseReport.managed_python.distribution.sourceUrl;
  changedRelease.inventory.managed_python[0].full_archive_url = releaseManifest.archive_url;
  rewrite(changedRelease, changedRelease.paths[target].reportPath, releaseReport);
  rewrite(changedRelease, changedRelease.paths[target].manifestPath, releaseManifest);
  changedRelease.inventory.packages[0].source = releaseManifest.archive_url;
  changedRelease.json(`${directory}/inventory.json`, changedRelease.inventory);
  assert.match(changedRelease.run().stderr, /selected\/full archive mismatch/);

  const changedVersion = fixture(t);
  const versionReport = JSON.parse(fs.readFileSync(path.join(changedVersion.root, changedVersion.paths[target].reportPath)));
  versionReport.managed_python.distribution.version = '3.14.8';
  versionReport.managed_python.distribution.catalogKey = `cpython-3.14.8-${target}`;
  changedVersion.inventory.managed_python[0].catalog_key = versionReport.managed_python.distribution.catalogKey;
  rewrite(changedVersion, changedVersion.paths[target].reportPath, versionReport);
  assert.match(changedVersion.run().stderr, /selected\/full archive mismatch/);
});

test('swapping complete Rust target pin arms is rejected after rehashing', t => {
  const data = fixture(t);
  const python = process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3');
  const probe = `import importlib.util, pathlib, sys\nspec = importlib.util.spec_from_file_location('notices', sys.argv[1])\nmodule = importlib.util.module_from_spec(spec)\nspec.loader.exec_module(module)\nmodule.ROOT = pathlib.Path(sys.argv[2])\nmodule.retained_provider_pins(set(sys.argv[3].split(',')))`;
  const validateGeneratorPins = () => spawnSync(python, ['-c', probe, path.join(source, 'scripts/release/generate-notices.py'), data.root, targets.join(',')], { encoding: 'utf8' });
  assert.equal(validateGeneratorPins().status, 0);
  const file = path.join(data.root, pinsPath);
  const before = fs.readFileSync(file, 'utf8');
  const pinFor = arm => {
    const match = before.match(new RegExp(`Self::${arm} => UvPin \\{\\s*archive: "([^"]+)",\\s*sha256: "([0-9a-f]{64})"`));
    assert.ok(match);
    return { archive: match[1], sha256: match[2] };
  };
  const linux = pinFor('LinuxX8664');
  const windows = pinFor('WindowsX8664');
  const swap = (text, arm, other) => text.replace(
    new RegExp(`(Self::${arm} => UvPin \\{\\s*)archive: "[^"]+",\\s*sha256: "[0-9a-f]{64}"`),
    (_, prefix) => `${prefix}archive: "${other.archive}",\n        sha256: "${other.sha256}"`,
  );
  data.write(pinsPath, swap(swap(before, 'LinuxX8664', windows), 'WindowsX8664', linux));
  data.inventory.input_sha256[pinsPath] = hash(fs.readFileSync(file));
  data.json(`${directory}/inventory.json`, data.inventory);

  assert.match(data.run().stderr, /Managed uv pins do not match desktop targets/);
  const generated = validateGeneratorPins();
  assert.notEqual(generated.status, 0);
  assert.match(generated.stderr, /Managed uv pins do not match desktop targets/);
});

test('checker rejects missing or malformed full-archive digests after rehashing', t => {
  for (const invalid of [undefined, 'A'.repeat(64), 'd'.repeat(63), 'g'.repeat(64)]) {
    const data = fixture(t);
    const manifestPath = data.paths[targets[0]].manifestPath;
    const manifest = JSON.parse(fs.readFileSync(path.join(data.root, manifestPath)));
    if (invalid === undefined) {
      delete manifest.archive_sha256;
      delete data.inventory.managed_python[0].full_archive_sha256;
    } else {
      manifest.archive_sha256 = invalid;
      data.inventory.managed_python[0].full_archive_sha256 = invalid;
    }
    data.json(manifestPath, manifest);
    data.inventory.input_sha256[manifestPath] = hash(fs.readFileSync(path.join(data.root, manifestPath)));
    data.json(`${directory}/inventory.json`, data.inventory);
    assert.match(data.run().stderr, /selected\/full archive mismatch/, String(invalid));
  }
});

test('checker requires each legal notice text exactly once', t => {
  const data = fixture(t);
  const target = targets[0];
  const manifestPath = data.paths[target].manifestPath;
  const secondPath = data.paths[target].legalPath.replace('LICENSE.cpython.txt', 'LICENSE.extra.txt');
  const second = { path: 'licenses/LICENSE.extra.txt', sha256: hash('second legal\n') };
  data.write(secondPath, 'second legal\n');
  data.inventory.input_sha256[secondPath] = second.sha256;
  const manifest = JSON.parse(fs.readFileSync(path.join(data.root, manifestPath)));
  manifest.files.push(second);
  data.json(manifestPath, manifest);
  data.inventory.input_sha256[manifestPath] = hash(fs.readFileSync(path.join(data.root, manifestPath)));
  data.inventory.managed_python[0].legal_files.push({ path: secondPath, sha256: second.sha256 });
  const notice = data.inventory.packages[0];
  notice.texts.push(second);
  data.json(`${directory}/inventory.json`, data.inventory);
  assert.equal(data.run().status, 0);
  notice.texts[1] = { ...notice.texts[0] };
  data.json(`${directory}/inventory.json`, data.inventory);
  assert.match(data.run().stderr, /Missing managed CPython full-archive notice/);
  notice.texts = [second];
  data.json(`${directory}/inventory.json`, data.inventory);
  assert.match(data.run().stderr, /Missing managed CPython full-archive notice/);
});
