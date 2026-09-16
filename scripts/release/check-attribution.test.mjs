import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

test('attribution survives a Windows-style Git checkout without changing legal bytes', t => {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-attribution-checkout-'));
  t.after(() => fs.rmSync(temporary, { recursive: true, force: true }));
  const repository = path.join(temporary, 'repository');
  const checkout = path.join(temporary, 'checkout');
  const source = fileURLToPath(new URL('../..', import.meta.url));
  const version = JSON.parse(fs.readFileSync(path.join(source, 'package.json'))).version;
  const directory = `docs/release-attribution/${version}`;
  const inventory = JSON.parse(fs.readFileSync(path.join(source, directory, 'inventory.json')));
  const files = [
    '.gitattributes', 'package.json', 'scripts/release/check-attribution.cjs',
    `${directory}/inventory.json`, `${directory}/THIRD-PARTY-NOTICES.txt`,
    ...Object.keys(inventory.input_sha256),
    ...inventory.sources.map(item => `scripts/release/licenses/${item.file}`),
  ];
  for (const name of files) {
    const target = path.join(repository, name);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(source, name), target);
  }
  const run = (command, args) => {
    const result = spawnSync(command, args, { cwd: repository, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
  };
  run('git', ['init', '--quiet']);
  run('git', ['-c', 'core.autocrlf=false', 'add', '.']);
  run('git', ['-c', 'core.autocrlf=true', 'checkout-index', '--all', `--prefix=${checkout}/`]);
  run(process.execPath, [path.join(checkout, 'scripts/release/check-attribution.cjs')]);
});

test('packaging refuses stale inputs, missing notices and altered upstream texts', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-attribution-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const hash = text => createHash('sha256').update(text).digest('hex');
  const write = (name, text) => { const p = path.join(root, name); fs.mkdirSync(path.dirname(p), { recursive: true }); fs.writeFileSync(p, text); };
  const inputs = ['rust/Cargo.lock', 'rust/Cargo.toml', 'rust/crates/pumas-core/Cargo.toml', 'rust/crates/pumas-rpc/Cargo.toml', 'rust/crates/pumas-app-manager/Cargo.toml', 'pnpm-lock.yaml', 'electron/package.json', 'frontend/package.json'];
  for (const input of inputs) write(input, '{}');
  write('package.json', '{"version":"0.7.0"}');
  write('docs/release-attribution/0.7.0/THIRD-PARTY-NOTICES.txt', 'license text');
  write('scripts/release/licenses/source.txt', 'upstream');
  write('docs/release-attribution/0.7.0/inventory.json', JSON.stringify({
    input_sha256: Object.fromEntries(inputs.map(p => [p, hash('{}')])),
    notices_sha256: hash('license text'), packages: [{ name: 'fixture' }],
    sources: [{ file: 'source.txt', sha256: hash('upstream') }],
  }));
  write('scripts/release/check-attribution.cjs', fs.readFileSync(new URL('./check-attribution.cjs', import.meta.url)));
  const run = () => spawnSync(process.execPath, [path.join(root, 'scripts/release/check-attribution.cjs')], { encoding: 'utf8' });
  assert.equal(run().status, 0);
  write('rust/Cargo.lock', 'changed dependency');
  assert.match(run().stderr, /Stale release attribution/);
  write('rust/Cargo.lock', '{}');
  write('rust/crates/pumas-core/Cargo.toml', 'changed feature selection');
  assert.match(run().stderr, /Stale release attribution/);
  write('rust/crates/pumas-core/Cargo.toml', '{}');
  write('scripts/release/licenses/source.txt', 'altered upstream');
  assert.match(run().stderr, /Changed upstream license/);
  write('scripts/release/licenses/source.txt', 'upstream');
  fs.unlinkSync(path.join(root, 'docs/release-attribution/0.7.0/THIRD-PARTY-NOTICES.txt'));
  assert.notEqual(run().status, 0);
});
