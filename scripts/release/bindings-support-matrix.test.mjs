import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const matrix = JSON.parse(read('bindings/support-matrix.json'));
// The code each surface's generator lives in.
const owners = { uniffi: 'rust/crates/pumas-uniffi/src/lib.rs', rustler: 'rust/crates/pumas-rustler/src/lib.rs' };

test('the binding matrix supports no host and its release plan ships no binding', () => {
  assert.equal(matrix.contract.status, 'no_supported_host_bindings');
  assert.deepEqual(matrix.contract.accepted_host_tuples, []);
  const plan = JSON.parse(read('scripts/release/artifact-plan.json'));
  assert.equal(plan.binding_support_matrix, 'bindings/support-matrix.json');
  assert.ok(plan.excluded_outputs.some(item => item.id === 'host-binding-bundles'));
});

test('every unsupported surface states whether its code still exists, and that is true', () => {
  const statuses = Object.keys(matrix.surface_statuses);
  assert.deepEqual(statuses.sort(), ['claim_removed', 'present_pending_removal']);
  for (const surface of matrix.unsupported_surfaces) {
    assert.ok(statuses.includes(surface.status), surface.id);
    const owner = owners[surface.id.split('-')[0]];
    if (surface.status === 'present_pending_removal') {
      assert.ok(owner && fs.existsSync(path.join(root, owner)), `${surface.id} is marked present but has no code`);
      assert.ok(surface.planned_removal.length > 0, surface.id);
    } else {
      assert.ok(surface.removed.length > 0, surface.id);
    }
  }
});

test('removed claims stay removed from the adapter documentation', () => {
  const adapter = read(owners.uniffi);
  for (const surface of matrix.unsupported_surfaces.filter(item => item.status === 'claim_removed')) {
    const docs = adapter.split('\n').filter(line => line.startsWith('//!')).join('\n');
    assert.doesNotMatch(docs, new RegExp(`\\b${surface.host}\\b`, 'i'), `${surface.id} is claimed again`);
  }
});

test('the bindings generator never installs an unpinned tool', () => {
  const script = read('scripts/generate-bindings.sh');
  assert.match(script, /^set -euo pipefail$/m);
  for (const line of script.split('\n').filter(item => /cargo install/.test(item) && !item.trim().startsWith('#'))) {
    assert.match(line, /--locked/, line);
  }
  assert.match(script, /--rev "\$UNIFFI_BINDGEN_CS_REV"/);
  assert.match(script, /^UNIFFI_BINDGEN_CS_REV="[0-9a-f]{40}"$/m);
  assert.doesNotMatch(script, /uniffi-bindgen-cli|--tag /);
  assert.match(script, /cargo run --locked [^\n]*\\\n[^\n]*--bin pumas-uniffi-bindgen/);
});
