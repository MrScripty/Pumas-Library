/** Package one existing Linux RPC test output, never a production release. */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const attribution = 'docs/release-attribution/0.7.0';
const hash = file => createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const capture = args => execFileSync(args[0], args.slice(1), { cwd: root, encoding: 'utf8' }).trim();

export function packageQualification(binary, outputDirectory, provenance, options = {}) {
  const repository = options.repository ?? root;
  const execute = options.execute ?? execFileSync;
  if (!fs.statSync(binary).isFile() || fs.statSync(binary).size === 0) throw new Error('Missing RPC binary');
  const output = path.join(outputDirectory, 'pumas-rpc-qualification-linux.tar.gz');
  fs.mkdirSync(outputDirectory, { recursive: true });
  if (fs.existsSync(output)) throw new Error('Refusing to overwrite qualification archive');
  const stage = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-rpc-qualification-'));
  try {
    fs.copyFileSync(binary, path.join(stage, 'pumas-rpc'));
    const originalBinarySha256 = hash(binary);
    execute('strip', ['--strip-debug', '--strip-unneeded', path.join(stage, 'pumas-rpc')]);
    const inputs = [
      ['LICENSE', 'LICENSE.txt'],
      [`${attribution}/THIRD-PARTY-NOTICES.txt`, 'THIRD-PARTY-NOTICES.txt'],
      [`${attribution}/README.md`, 'ATTRIBUTION-README.md'],
      [`${attribution}/inventory.json`, 'ATTRIBUTION-inventory.json'],
    ];
    const files = {};
    for (const [input, name] of inputs) {
      fs.copyFileSync(path.join(repository, input), path.join(stage, name));
      files[name] = { source: input, sha256: hash(path.join(stage, name)) };
    }
    const manifest = {
      purpose: 'unreleased library-only qualification; not production-ready or an inference runtime',
      ...provenance,
      original_binary_sha256: originalBinarySha256,
      binary_sha256: hash(path.join(stage, 'pumas-rpc')),
      binary_transformation: 'strip --strip-debug --strip-unneeded',
      attribution_scope: 'unchanged repository notice inventory is a conservative superset; no Python runtime, model weights, credentials or runtime userdata are packaged',
      files,
    };
    fs.writeFileSync(path.join(stage, 'qualification.json'), `${JSON.stringify(manifest, null, 2)}\n`);
    const sums = fs.readdirSync(stage).sort().map(name => `${hash(path.join(stage, name))}  ${name}`).join('\n');
    fs.writeFileSync(path.join(stage, 'SHA256SUMS'), `${sums}\n`);
    // A fresh, private allowlisted stage prevents bundling runtime/config data.
    execute('tar', ['-C', stage, '-czf', path.resolve(output), ...fs.readdirSync(stage).sort()]);
    fs.writeFileSync(`${output}.sha256`, `${hash(output)}  ${path.basename(output)}\n`);
    return { output, manifest };
  } finally {
    fs.rmSync(stage, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (process.platform !== 'linux' || process.arch !== 'x64') throw new Error('Qualification target is Linux x86_64 only');
  const [binary, output, sourceHead] = process.argv.slice(2);
  if (!binary || !output || !/^[a-f0-9]{40}$/.test(sourceHead ?? '')) throw new Error('Expected binary, output directory and source head SHA');
  const provenance = {
    source_head: sourceHead,
    source_tree: capture(['git', 'rev-parse', `${sourceHead}^{tree}`]),
    checkout_commit: capture(['git', 'rev-parse', 'HEAD']),
    checkout_tree: capture(['git', 'rev-parse', 'HEAD^{tree}']),
    toolchain: capture(['rustc', '-Vv']),
    cargo: capture(['cargo', '-V']),
    target: 'x86_64-unknown-linux-gnu',
    build_profile: 'debug/test output',
    build_command: 'cargo test --locked --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features',
    default_features: false,
    feature_selection: 'no RPC default inference-plugins; Cargo test may unify dependency test-support features',
    runtime_dependencies: capture(['ldd', path.resolve(binary)]),
  };
  packageQualification(binary, output, provenance);
}
