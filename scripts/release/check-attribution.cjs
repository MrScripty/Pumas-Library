const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');

const root = path.resolve(__dirname, '../..');
const catalogPath = 'scripts/release/licenses/managed-python-sources.json';
const planPath = 'scripts/release/artifact-plan.json';
const pinsPath = 'rust/crates/pumas-app-manager/src/version_manager/managed_python.rs';
const mappingPath = 'scripts/release/torch-managed-python-acceptance.py';
const uvArmTargets = {
  LinuxX8664: 'x86_64-unknown-linux-gnu',
  WindowsX8664: 'x86_64-pc-windows-msvc',
  MacosArm64: 'aarch64-apple-darwin',
};
const baseInputs = [
  'rust/Cargo.lock', 'rust/Cargo.toml',
  'rust/crates/pumas-core/Cargo.toml', 'rust/crates/pumas-rpc/Cargo.toml',
  'rust/crates/pumas-app-manager/Cargo.toml',
  'pnpm-lock.yaml', 'electron/package.json', 'frontend/package.json',
  planPath, catalogPath, pinsPath, mappingPath,
];

function retainedProviderPins(source, expectedTargets) {
  const version = [...source.matchAll(/^const UV_VERSION: &str = "([0-9]+\.[0-9]+\.[0-9]+)";/gm)].map(match => match[1]);
  const base = [...source.matchAll(/^const UV_BASE_URL: &str = "https:\/\/releases\.astral\.sh\/github\/uv\/releases\/download\/([0-9.]+)\/";/gm)].map(match => match[1]);
  const block = source.match(/fn pin\(self\) -> UvPin \{([\s\S]*?)\n    fn accepts_observed/);
  const pins = [...(block?.[1] ?? '').matchAll(/Self::(\w+) => UvPin \{\s*archive: "uv-([a-z0-9_-]+)\.(?:tar\.gz|zip)",\s*sha256: "([0-9a-f]{64})"/g)];
  if (version.length !== 1 || base.length !== 1 || base[0] !== version[0] || pins.length !== 3
    || JSON.stringify(pins.map(pin => pin[1]).sort()) !== JSON.stringify(Object.keys(uvArmTargets).sort())
    || JSON.stringify(pins.map(pin => pin[2]).sort()) !== JSON.stringify(expectedTargets)
    || pins.some(pin => uvArmTargets[pin[1]] !== pin[2])) {
    throw new Error('Managed uv pins do not match desktop targets');
  }
  return { version: version[0], hashes: Object.fromEntries(pins.map(pin => [pin[2], pin[3]])) };
}

function retainedFullArchiveMapping(source, expectedTargets) {
  const release = [...source.matchAll(/^REVIEWED_PBS_RELEASE = "([0-9]{8})"$/gm)].map(match => match[1]);
  const block = source.match(/^FULL_FLAVORS = \{([\s\S]*?)^\}/m);
  const flavors = [...(block?.[1] ?? '').matchAll(/^    "([a-z0-9_-]+)": "([a-z0-9+]+)",$/gm)];
  if (release.length !== 1 || flavors.length !== 3
    || JSON.stringify(flavors.map(item => item[1]).sort()) !== JSON.stringify(expectedTargets)) {
    throw new Error('Managed CPython full-archive mapping does not match desktop targets');
  }
  return { release: release[0], flavors: Object.fromEntries(flavors.map(item => [item[1], item[2]])) };
}

function checkAttribution(directory) {
  const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'))).version;
  if (typeof directory !== 'string' || !directory) {
    directory = path.join(root, 'docs/release-attribution', version);
  }
  const inventory = JSON.parse(fs.readFileSync(path.join(directory, 'inventory.json')));
  const hash = file => createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  const relative = name => {
    if (typeof name !== 'string' || !name || name.includes('\\') || path.posix.isAbsolute(name) || name.split('/').some(part => !part || part === '.' || part === '..')) {
      throw new Error(`Invalid attribution input: ${name}`);
    }
    return path.join(root, name);
  };
  const input = name => {
    const expected = inventory.input_sha256?.[name];
    if (typeof expected !== 'string' || !/^[0-9a-f]{64}$/.test(expected) || hash(relative(name)) !== expected) {
      throw new Error(`Stale release attribution: ${name}`);
    }
  };
  for (const name of baseInputs) input(name);
  for (const name of Object.keys(inventory.input_sha256)) input(name);
  if (hash(path.join(directory, 'THIRD-PARTY-NOTICES.txt')) !== inventory.notices_sha256) throw new Error('Changed release notices');
  if (!inventory.packages?.length) throw new Error('Empty release attribution inventory');
  for (const source of inventory.sources) {
    if (hash(relative(`scripts/release/licenses/${source.file}`)) !== source.sha256) throw new Error(`Changed upstream license: ${source.file}`);
  }

  const plan = JSON.parse(fs.readFileSync(relative(planPath)));
  const expectedTargets = plan.desktop_targets.map(item => item.rust_target).sort();
  const uv = retainedProviderPins(fs.readFileSync(relative(pinsPath), 'utf8'), expectedTargets);
  const pbs = retainedFullArchiveMapping(fs.readFileSync(relative(mappingPath), 'utf8'), expectedTargets);
  const catalog = JSON.parse(fs.readFileSync(relative(catalogPath)));
  const entries = catalog.targets;
  const managed = inventory.managed_python;
  const coversTargets = values => Array.isArray(values) && values.length === 3
    && expectedTargets.length === 3
    && JSON.stringify(values.map(value => value.target).sort()) === JSON.stringify(expectedTargets);
  if (catalog.schema_version !== 1 || !coversTargets(entries) || !coversTargets(managed)) {
    throw new Error('Incomplete managed CPython attribution targets');
  }
  for (const entry of entries) {
    const record = managed.find(item => item.target === entry.target);
    if (record.runtime_report !== entry.runtime_report || record.manifest !== entry.manifest) {
      throw new Error(`Managed CPython evidence mismatch: ${entry.target}`);
    }
    input(entry.runtime_report);
    input(entry.manifest);
    const report = JSON.parse(fs.readFileSync(relative(entry.runtime_report)));
    const manifest = JSON.parse(fs.readFileSync(relative(entry.manifest)));
    const distribution = report.managed_python?.distribution;
    const provider = report.managed_python?.provider;
    const version = distribution?.version;
    const fullName = `cpython-${version}+${pbs.release}-${entry.target}-${pbs.flavors[entry.target]}-full.tar.zst`;
    const fullUrl = `https://github.com/astral-sh/python-build-standalone/releases/download/${pbs.release}/${encodeURIComponent(fullName)}`;
    const selectedUrls = ['install_only', 'install_only_stripped'].map(flavor =>
      `https://releases.astral.sh/github/python-build-standalone/releases/download/${pbs.release}/${encodeURIComponent(`cpython-${version}+${pbs.release}-${entry.target}-${flavor}.tar.gz`)}`);
    const archiveDir = `${path.posix.dirname(entry.manifest)}/full-archive`;
    const pythonPath = `${archiveDir}/PYTHON.json`;
    if (record.python_json !== pythonPath) throw new Error(`Managed CPython PYTHON.json path mismatch: ${entry.target}`);
    input(pythonPath);
    const pythonBytes = fs.readFileSync(relative(pythonPath));
    const python = JSON.parse(pythonBytes);
    const files = manifest.files;
    const declared = Array.isArray(files) ? files.map(item => item.path) : [];
    const actual = fs.readdirSync(relative(`${archiveDir}/licenses`), { withFileTypes: true })
      .filter(item => item.isFile()).map(item => `licenses/${item.name}`).sort();
    if (!declared.length || new Set(declared).size !== declared.length
      || declared.some(name => !/^licenses\/[A-Za-z0-9._-]+$/.test(name))
      || JSON.stringify(declared.slice().sort()) !== JSON.stringify(actual)
      || !declared.includes(python.license_path)) {
      throw new Error(`Incomplete managed CPython legal files: ${entry.target}`);
    }
    const legalFiles = files.map(item => ({ path: `${archiveDir}/${item.path}`, sha256: item.sha256 }));
    const byPath = (a, b) => a.path.localeCompare(b.path);
    if (JSON.stringify(legalFiles.slice().sort(byPath)) !== JSON.stringify(record.legal_files?.slice().sort(byPath))) {
      throw new Error(`Managed CPython legal inventory mismatch: ${entry.target}`);
    }
    for (const item of legalFiles) {
      input(item.path);
      if (inventory.input_sha256[item.path] !== item.sha256) throw new Error(`Changed managed CPython legal file: ${item.path}`);
    }
    if (manifest.python_json?.path !== 'PYTHON.json'
      || inventory.input_sha256[pythonPath] !== manifest.python_json.sha256
      || pythonBytes.length !== manifest.python_json.size
      || python.target_triple !== entry.target || python.python_version !== version
      || python.build_options !== pbs.flavors[entry.target]
      || distribution?.targetTriple !== entry.target || distribution.implementation !== 'CPython'
      || !/^\d+\.\d+\.\d+$/.test(version)
      || !distribution.catalogKey.startsWith(`cpython-${version}-`)
      || provider?.name !== 'uv' || provider.version !== uv.version
      || provider.archiveSha256 !== uv.hashes[entry.target]
      || manifest.target !== entry.target
      || manifest.source_url !== distribution.sourceUrl
      || !selectedUrls.includes(distribution.sourceUrl)
      || manifest.archive_name !== fullName || manifest.archive_url !== fullUrl
      || !/^[0-9a-f]{64}$/.test(manifest.archive_sha256)
      || !/^[0-9a-f]{64}$/.test(record.full_archive_sha256)
      || record.catalog_key !== distribution.catalogKey
      || record.selected_install_only_url !== distribution.sourceUrl
      || record.provider_name !== provider.name
      || record.provider_version !== provider.version
      || record.provider_archive_sha256 !== provider.archiveSha256
      || record.full_archive_url !== manifest.archive_url
      || record.full_archive_sha256 !== manifest.archive_sha256
      || record.selected_install_only_url === record.full_archive_url) {
      throw new Error(`Managed CPython selected/full archive mismatch: ${entry.target}`);
    }
    const notice = inventory.packages.find(item => item.name === 'CPython full archive'
      && item.version === distribution.version && item.scope.includes(entry.target));
    const expectedTexts = files.map(item => ({ path: item.path, sha256: item.sha256 })).sort(byPath);
    if (!notice || notice.source !== manifest.archive_url || !Array.isArray(notice.texts)
      || JSON.stringify(notice.texts.slice().sort(byPath)) !== JSON.stringify(expectedTexts)) {
      throw new Error(`Missing managed CPython full-archive notice: ${entry.target}`);
    }
  }
}

module.exports = checkAttribution;
if (require.main === module) {
  checkAttribution();
  console.log('Release attribution matches current inputs and pinned texts');
}
