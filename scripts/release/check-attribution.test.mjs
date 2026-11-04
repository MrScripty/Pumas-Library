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
const supplementalDir = 'scripts/release/licenses/managed-python-supplemental';
const supplementalReason = 'declared_by_python_json_but_absent_from_upstream_full_archive';
const python = process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3');
const hash = data => createHash('sha256').update(data).digest('hex');

test('bundled JPEG attribution retains IJG terms and exact wrapper provenance', () => {
  const result = spawnSync(process.env.PYTHON ?? (process.platform === 'win32' ? 'python' : 'python3'),
    ['-m', 'unittest', '-v', 'test_jpeg_attribution'],
    { cwd: path.join(source, 'scripts/release'), encoding: 'utf8', timeout: 30_000 });
  assert.equal(result.status, 0, result.stderr || String(result.error));
});

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
  // Mirrors PBS 20260901: PYTHON.json declares texts its full archive omits.
  const supplemental = [
    { declared_path: 'licenses/LICENSE.zlib-ng.txt', component: 'zlib-ng', version: '2.2.4', license: 'Zlib',
      file: 'zlib-ng-2.2.4-LICENSE.md', text: 'zlib-ng terms\n', targets: targets.filter(target => !target.includes('windows')).sort() },
    { declared_path: 'licenses/LICENSE.zstd.txt', component: 'zstd', version: '1.5.7', license: 'BSD-3-Clause',
      file: 'zstd-1.5.7-LICENSE', text: 'zstd terms\n', targets: targets.slice().sort() },
  ].map(item => {
    const record = {
      schema_version: 1, declared_path: item.declared_path, supplemental_reason: supplementalReason,
      note: 'Fixture text supplied outside the archive.', python_build_standalone_release: '20260901',
      pin_authority: 'https://github.com/astral-sh/python-build-standalone/blob/20260901/pythonbuild/downloads.py',
      targets: item.targets, component: item.component, version: item.version, license: item.license,
      source_archive: { url: `https://example.invalid/${item.component}.tar.gz`, size: 10, sha256: 'b'.repeat(64) },
      path_in_source_archive: `${item.component}/LICENSE`, file: item.file,
      size: Buffer.byteLength(item.text), sha256: hash(item.text),
    };
    const textPath = `${supplementalDir}/${item.file}`;
    const recordPath = `${textPath}.provenance.json`;
    write(textPath, item.text, true);
    json(recordPath, record, true);
    return { record, textPath, recordPath };
  });
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
    const supplied = supplemental.filter(item => item.record.targets.includes(target));
    const python = { target_triple: target, python_version: '3.14.7', build_options: flavors[target], license_path: 'licenses/LICENSE.cpython.txt', licenses: ['Python-2.0'],
      build_info: { extensions: { _zstd: [{ license_paths: supplied.map(item => item.record.declared_path) }] } } };
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
      supplemental_legal_files: supplied.map(item => ({ declared_path: item.record.declared_path,
        path: item.textPath, sha256: item.record.sha256, provenance: item.recordPath })),
    });
    packages.push({ name: 'CPython full archive', version: '3.14.7', license_declared: 'Python-2.0', source: full,
      scope: `Managed Python full-archive notice superset for ${target}; selected install-only runtime tracked separately`,
      texts: [{ path: 'licenses/LICENSE.cpython.txt', sha256: hash(legal) }],
    });
    for (const { record } of supplied) {
      packages.push({ name: record.component, version: record.version, license_declared: record.license,
        source: record.source_archive.url,
        scope: `Managed Python supplemental license text for ${target}; PYTHON.json declares ${record.declared_path} but the CPython full archive omits it; supplied from ${record.path_in_source_archive} in the source archive pinned by python-build-standalone 20260901 (sha256 ${record.source_archive.sha256}), not from the CPython archive`,
        texts: [{ path: `managed-python-supplemental/${record.file}`, sha256: record.sha256 }],
      });
    }
    paths[target] = { reportPath, manifestPath, pythonPath, legalPath };
  }
  json(catalogPath, { schema_version: 1, targets: entries }, true);
  const inventory = { schema_version: 1, input_sha256: inputs, notices_sha256: hash('license text'),
    packages, managed_python: managed, sources: [{ file: 'source.txt', sha256: hash('upstream') }],
  };
  json(`${directory}/inventory.json`, inventory);
  fs.copyFileSync(new URL('./check-attribution.cjs', import.meta.url), path.join(root, 'scripts/release/check-attribution.cjs'));
  fs.copyFileSync(path.join(source, '.gitattributes'), path.join(root, '.gitattributes'));
  const run = (features = []) => spawnSync(process.execPath, [path.join(root, 'scripts/release/check-attribution.cjs'),
    '--directory', path.join(root, directory), ...(features.length ? ['--features', ...features] : [])], { encoding: 'utf8' });
  const save = () => json(`${directory}/inventory.json`, inventory);
  return { root, run, write, json, inventory, paths, supplemental, save };
}

// Run the real generator's managed-CPython collection against a fixture root.
function generateManagedPython(root) {
  const probe = `import importlib.util, json, pathlib, sys
spec = importlib.util.spec_from_file_location('notices', sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
module.ROOT = pathlib.Path(sys.argv[2])
module.LICENSES = module.ROOT / 'scripts/release/licenses'
packages = []
def add(name, version, license_id, source, root, files, scope):
    packages.append({'name': name, 'version': version, 'license_declared': license_id, 'source': source, 'scope': scope,
        'texts': [{'path': file.relative_to(root if file.is_relative_to(root) else module.LICENSES).as_posix(),
                   'sha256': module.digest(file.read_bytes())} for file in files]})
try:
    inventory, inputs, _ = module.collect_managed_python(add)
except ValueError as error:
    print(json.dumps({'error': str(error)}))
else:
    print(json.dumps({'inventory': inventory, 'inputs': sorted(inputs), 'packages': packages}))`;
  const result = spawnSync(python, ['-c', probe, path.join(source, 'scripts/release/generate-notices.py'), root], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

test('attribution survives a Windows-style checkout with original legal bytes', t => {
  const { root, run, inventory, paths, supplemental } = fixture(t);
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
  for (const { textPath } of supplemental) {
    assert.deepEqual(fs.readFileSync(path.join(checkout, textPath)), fs.readFileSync(path.join(root, textPath)));
  }
  assert.equal(Object.keys(inventory.input_sha256).length, 12 + targets.length * 4 + supplemental.length * 2);
});

test('S3 notices require the explicit feature profile and complete Rust closure', t => {
  const data = fixture(t);
  assert.match(data.run(['s3']).stderr, /feature profile mismatch/);
  data.inventory.rust_profile = { package: 'pumas-rpc', default_features: true, features: ['s3'],
    targets: ['x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc'] };
  data.inventory.rust_dependencies = [{ name: 'aws-sdk-s3', version: '1.137.0' }];
  const notice = { name: 'aws-sdk-s3', version: '1.137.0', scope: 'Rust normal/build dependency, all desktop targets' };
  data.inventory.packages.push(notice);
  const save = () => data.json(`${directory}/inventory.json`, data.inventory);
  save();
  assert.equal(data.run(['s3']).status, 0);
  assert.match(data.run().stderr, /feature profile mismatch/);
  data.inventory.packages.pop();
  save();
  assert.match(data.run(['s3']).stderr, /Incomplete Rust release attribution closure/);
  data.inventory.packages.push(notice);
  data.inventory.rust_dependencies.push({ ...data.inventory.rust_dependencies[0] });
  save();
  assert.match(data.run(['s3']).stderr, /Incomplete Rust release attribution closure/);
});

test('generator refuses selected dependencies absent from metadata instead of omitting notices', t => {
  const data = fixture(t);
  const probe = `import importlib.util, pathlib, sys\nspec = importlib.util.spec_from_file_location('notices', sys.argv[1])\nmodule = importlib.util.module_from_spec(spec)\nspec.loader.exec_module(module)\nmodule.LICENSES = pathlib.Path(sys.argv[2])\n(module.LICENSES / 'sources.json').write_text('[]')\nseen = []\ndef run(*args):\n seen.append(args)\n if args[1] == 'metadata':\n  assert '--features' in args and 'pumas-rpc/s3' in args\n  return '{"packages": []}'\n return 'aws-sdk-s3 v1.137.0\\n'\nmodule.run = run\ntry:\n module.collect(('s3',))\nexcept ValueError as error:\n assert 'Incomplete Rust attribution closure' in str(error), str(error)\nelse:\n raise AssertionError('selected SDK was silently omitted')`;
  const result = spawnSync(python, ['-c', probe, path.join(source, 'scripts/release/generate-notices.py'), path.join(data.root, 'scripts/release/licenses')], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
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

test('supplemental license rules pass their Python unit tests', () => {
  const result = spawnSync(python, ['-m', 'unittest', '-v', 'test_managed_python_supplemental'],
    { cwd: path.join(source, 'scripts/release'), encoding: 'utf8', timeout: 30_000 });
  assert.equal(result.status, 0, result.stderr || String(result.error));
});

test('generator and checker agree on verified supplemental CPython notices', t => {
  const data = fixture(t);
  assert.equal(data.run().status, 0, data.run().stderr);
  const generated = generateManagedPython(data.root);
  assert.equal(generated.error, undefined, generated.error);
  const byTarget = (a, b) => a.target.localeCompare(b.target);
  assert.deepEqual(generated.inventory, data.inventory.managed_python.slice().sort(byTarget));
  const byNotice = (a, b) => `${a.scope}\0${a.name}`.localeCompare(`${b.scope}\0${b.name}`);
  assert.deepEqual(generated.packages.slice().sort(byNotice), data.inventory.packages.slice().sort(byNotice));
  const base = new Set(['rust/Cargo.lock', 'rust/Cargo.toml', 'rust/crates/pumas-core/Cargo.toml', 'rust/crates/pumas-rpc/Cargo.toml', 'rust/crates/pumas-app-manager/Cargo.toml', 'pnpm-lock.yaml', 'electron/package.json', 'frontend/package.json']);
  assert.deepEqual(generated.inputs, Object.keys(data.inventory.input_sha256).filter(name => !base.has(name)).sort());
  const windows = generated.inventory.find(item => item.target === 'x86_64-pc-windows-msvc');
  assert.deepEqual(windows.supplemental_legal_files.map(item => item.declared_path), ['licenses/LICENSE.zstd.txt']);
  for (const item of generated.inventory) {
    assert.ok(item.legal_files.every(file => !file.path.startsWith(supplementalDir)), 'supplemental text listed as archive member');
  }
});

test('supplemental text whose bytes differ from its provenance is refused', t => {
  const data = fixture(t);
  const { textPath } = data.supplemental[1];
  data.write(textPath, 'altered zstd terms\n');
  assert.match(data.run().stderr, /Stale release attribution/);
  data.inventory.input_sha256[textPath] = hash('altered zstd terms\n');
  data.save();
  assert.match(data.run().stderr, /Changed managed CPython supplemental license/);
  assert.match(generateManagedPython(data.root).error, /Changed managed CPython supplemental license/);
});

test('declared licenses absent from the archive need an exact verified supplemental text', t => {
  const unsupplied = fixture(t);
  const zlib = unsupplied.supplemental[0];
  for (const name of [zlib.textPath, zlib.recordPath]) {
    fs.unlinkSync(path.join(unsupplied.root, name));
    delete unsupplied.inventory.input_sha256[name];
  }
  for (const item of unsupplied.inventory.managed_python) {
    item.supplemental_legal_files = item.supplemental_legal_files.filter(file => file.path !== zlib.textPath);
  }
  unsupplied.save();
  assert.match(unsupplied.run().stderr, /absent from the full archive and without a verified supplemental text .*LICENSE\.zlib-ng\.txt/);
  assert.match(generateManagedPython(unsupplied.root).error, /absent from the full archive.*LICENSE\.zlib-ng\.txt/);

  const undeclared = fixture(t);
  const extra = { ...undeclared.supplemental[1].record, declared_path: 'licenses/LICENSE.other.txt', file: 'other-LICENSE' };
  undeclared.write(`${supplementalDir}/other-LICENSE`, 'zstd terms\n', false);
  undeclared.json(`${supplementalDir}/other-LICENSE.provenance.json`, extra);
  assert.match(undeclared.run().stderr, /Supplemental managed CPython licenses not needed .*LICENSE\.other\.txt/);
  assert.match(generateManagedPython(undeclared.root).error, /not needed .*LICENSE\.other\.txt/);

  const orphan = fixture(t);
  orphan.write(`${supplementalDir}/unexplained-LICENSE`, 'terms\n');
  assert.match(orphan.run().stderr, /Supplemental license texts without provenance/);

  const presented = fixture(t);
  const notice = presented.inventory.packages.find(item => item.name === 'zstd');
  notice.texts = [{ path: 'licenses/LICENSE.zstd.txt', sha256: notice.texts[0].sha256 }];
  presented.save();
  assert.match(presented.run().stderr, /Missing managed CPython supplemental notice/);

  const omitted = fixture(t);
  omitted.inventory.managed_python[0].supplemental_legal_files = [];
  omitted.save();
  assert.match(omitted.run().stderr, /Managed CPython supplemental inventory mismatch/);
});
