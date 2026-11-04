const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');

const root = path.resolve(__dirname, '../..');
const catalogPath = 'scripts/release/licenses/managed-python-sources.json';
const planPath = 'scripts/release/artifact-plan.json';
const pinsPath = 'rust/crates/pumas-app-manager/src/version_manager/managed_python.rs';
const mappingPath = 'scripts/release/torch-managed-python-acceptance.py';
const supplementalPath = 'scripts/release/licenses/managed-python-supplemental';
const supplementalSuffix = '.provenance.json';
const supplementalReason = 'declared_by_python_json_but_absent_from_upstream_full_archive';
const supplementalKeys = [
  'schema_version', 'declared_path', 'supplemental_reason', 'note',
  'python_build_standalone_release', 'pin_authority', 'targets', 'component',
  'version', 'license', 'source_archive', 'path_in_source_archive', 'file', 'size', 'sha256',
].sort();
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

function declaredLicensePaths(metadata) {
  const paths = new Set();
  const visit = value => {
    if (Array.isArray(value)) value.forEach(visit);
    else if (value && typeof value === 'object') {
      for (const [key, item] of Object.entries(value)) {
        if ((key === 'license_path' || key === 'license_paths') && item !== null && item !== undefined) {
          const values = typeof item === 'string' ? [item] : item;
          if (!Array.isArray(values) || !values.every(entry => typeof entry === 'string' && entry)) {
            throw new Error(`Invalid PYTHON.json license reference: ${JSON.stringify(item)}`);
          }
          values.forEach(entry => paths.add(entry));
        } else visit(item);
      }
    }
  };
  visit(metadata);
  return paths;
}

// Supplemental texts satisfy declared CPython license paths that the upstream
// full archive omits. They are byte-verified against their provenance records
// and must never be presented as archive members.
function supplementalRecords(relative, hash, release, expectedTargets) {
  const directory = relative(supplementalPath);
  if (!fs.existsSync(directory)) return [];
  const entries = fs.readdirSync(directory, { withFileTypes: true });
  if (entries.some(item => !item.isFile())) throw new Error('Supplemental license directory must hold regular files');
  const sha = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
  const positive = value => Number.isInteger(value) && value > 0;
  const text = value => typeof value === 'string' && value.trim() !== '';
  const records = entries.filter(item => item.name.endsWith(supplementalSuffix)).map(item => {
    const recordPath = `${supplementalPath}/${item.name}`;
    const record = JSON.parse(fs.readFileSync(relative(recordPath)));
    const source = record?.source_archive;
    const targets = record?.targets;
    if (!record || typeof record !== 'object' || Array.isArray(record)
      || JSON.stringify(Object.keys(record).sort()) !== JSON.stringify(supplementalKeys)
      || record.schema_version !== 1 || record.supplemental_reason !== supplementalReason
      || record.python_build_standalone_release !== release
      || record.pin_authority !== `https://github.com/astral-sh/python-build-standalone/blob/${release}/pythonbuild/downloads.py`
      || typeof record.declared_path !== 'string' || !/^licenses\/[A-Za-z0-9._-]+$/.test(record.declared_path)
      || !Array.isArray(targets) || !targets.length || !targets.every(text)
      || JSON.stringify(targets) !== JSON.stringify([...new Set(targets)].sort())
      || !targets.every(target => expectedTargets.includes(target))
      || !['note', 'component', 'version', 'license', 'path_in_source_archive'].every(key => text(record[key]))
      || !source || typeof source !== 'object'
      || JSON.stringify(Object.keys(source).sort()) !== JSON.stringify(['sha256', 'size', 'url'])
      || typeof source.url !== 'string' || !source.url.startsWith('https://')
      || !positive(source.size) || !sha(source.sha256)
      || record.file !== item.name.slice(0, -supplementalSuffix.length)
      || !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(record.file)
      || !positive(record.size) || !sha(record.sha256)) {
      throw new Error(`Invalid managed CPython supplemental license record: ${item.name}`);
    }
    const textPath = `${supplementalPath}/${record.file}`;
    const data = fs.readFileSync(relative(textPath));
    if (hash(relative(textPath)) !== record.sha256 || data.length !== record.size) {
      throw new Error(`Changed managed CPython supplemental license: ${record.file}`);
    }
    return { ...record, recordPath, textPath };
  });
  const referenced = new Set(records.map(record => record.file));
  const orphans = entries.filter(item => !item.name.endsWith(supplementalSuffix) && !referenced.has(item.name));
  if (orphans.length) throw new Error(`Supplemental license texts without provenance: ${orphans.map(item => item.name)}`);
  const claimed = new Set();
  for (const record of records) {
    for (const target of record.targets) {
      const key = `${target}\0${record.declared_path}`;
      if (claimed.has(key)) throw new Error(`Duplicate supplemental license for ${target}: ${record.declared_path}`);
      claimed.add(key);
    }
  }
  return records;
}

function checkAttribution(directory, features = []) {
  if (!Array.isArray(features) || (features.length && (features.length !== 1 || features[0] !== 's3'))) {
    throw new Error('Unsupported release attribution feature profile');
  }
  const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'))).version;
  if (typeof directory !== 'string' || !directory) {
    directory = path.join(root, 'docs/release-attribution', version + (features.length ? '-s3' : ''));
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
  const profile = inventory.rust_profile;
  if (profile || features.length) {
    const targets = ['x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc'];
    if (profile?.package !== 'pumas-rpc' || profile.default_features !== true
      || JSON.stringify(profile.features) !== JSON.stringify(features)
      || JSON.stringify(profile.targets) !== JSON.stringify(targets)) {
      throw new Error('Release attribution feature profile mismatch');
    }
    const rust = inventory.packages.filter(item => item.scope?.startsWith('Rust normal/build dependency'));
    const identities = values => values.map(item => `${item.name}@${item.version}`).sort();
    const closure = inventory.rust_dependencies;
    if (!Array.isArray(closure) || !closure.length
      || new Set(identities(closure)).size !== closure.length
      || JSON.stringify(identities(closure)) !== JSON.stringify(identities(rust))
      || (features.length && !closure.some(item => item.name === 'aws-sdk-s3'))) {
      throw new Error('Incomplete Rust release attribution closure');
    }
  }
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
  const supplemental = supplementalRecords(relative, hash, pbs.release, expectedTargets);
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
    const archived = Array.isArray(files) ? files.map(item => item.path) : [];
    const actual = fs.readdirSync(relative(`${archiveDir}/licenses`), { withFileTypes: true })
      .filter(item => item.isFile()).map(item => `licenses/${item.name}`).sort();
    if (!archived.length || new Set(archived).size !== archived.length
      || archived.some(name => !/^licenses\/[A-Za-z0-9._-]+$/.test(name))
      || JSON.stringify(archived.slice().sort()) !== JSON.stringify(actual)
      || !archived.includes(python.license_path)) {
      throw new Error(`Incomplete managed CPython legal files: ${entry.target}`);
    }
    // Every declared reference must be an archive member or a verified supplemental text.
    const missing = [...declaredLicensePaths(python)].filter(name => !archived.includes(name)).sort();
    const applicable = supplemental.filter(item => item.targets.includes(entry.target));
    const supplied = applicable.filter(item => missing.includes(item.declared_path))
      .sort((a, b) => a.declared_path.localeCompare(b.declared_path));
    const unsatisfied = missing.filter(name => !supplied.some(item => item.declared_path === name));
    if (unsatisfied.length) {
      throw new Error(`Declared managed CPython licenses absent from the full archive and without a verified supplemental text for ${entry.target}: ${unsatisfied}`);
    }
    const stale = applicable.filter(item => !supplied.includes(item)).map(item => item.declared_path);
    if (stale.length) throw new Error(`Supplemental managed CPython licenses not needed for ${entry.target}: ${stale}`);
    const supplementalFiles = supplied.map(item => ({
      declared_path: item.declared_path, path: item.textPath, sha256: item.sha256, provenance: item.recordPath,
    }));
    if (JSON.stringify(record.supplemental_legal_files) !== JSON.stringify(supplementalFiles)) {
      throw new Error(`Managed CPython supplemental inventory mismatch: ${entry.target}`);
    }
    for (const item of supplied) {
      input(item.textPath);
      input(item.recordPath);
      if (inventory.input_sha256[item.textPath] !== item.sha256) throw new Error(`Changed managed CPython supplemental license: ${item.textPath}`);
      const label = item.textPath.slice('scripts/release/licenses/'.length);
      const notice = inventory.packages.find(pkg => pkg.name === item.component && pkg.version === item.version
        && pkg.scope?.startsWith(`Managed Python supplemental license text for ${entry.target};`));
      if (!notice || notice.source !== item.source_archive.url || notice.license_declared !== item.license
        || !notice.scope.includes(item.declared_path) || !notice.scope.includes(item.source_archive.sha256)
        || JSON.stringify(notice.texts) !== JSON.stringify([{ path: label, sha256: item.sha256 }])) {
        throw new Error(`Missing managed CPython supplemental notice: ${entry.target} ${item.declared_path}`);
      }
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
  const args = process.argv.slice(2);
  let directory;
  let features = [];
  while (args.length) {
    const flag = args.shift();
    if (flag === '--directory' && args.length && !directory) directory = args.shift();
    else if (flag === '--features' && args[0] === 's3' && !features.length) features = [args.shift()];
    else throw new Error('usage: check-attribution.cjs [--directory DIRECTORY] [--features s3]');
  }
  checkAttribution(directory, features);
  console.log('Release attribution matches current inputs and pinned texts');
}
