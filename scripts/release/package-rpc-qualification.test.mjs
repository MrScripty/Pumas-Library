import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { packageQualification } from './package-rpc-qualification.mjs';

test('qualification archive contains only binary, pinned provenance and notice inputs', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'qualification-test-'));
  try {
    const notices = path.join(temp, 'docs/release-attribution/0.7.0');
    fs.mkdirSync(notices, { recursive: true });
    fs.writeFileSync(path.join(temp, 'LICENSE'), 'synthetic project license');
    for (const name of ['THIRD-PARTY-NOTICES.txt', 'README.md', 'inventory.json']) fs.writeFileSync(path.join(notices, name), `synthetic ${name}`);
    const binary = path.join(temp, 'pumas-rpc');
    fs.writeFileSync(binary, 'synthetic executable bytes; never executed');
    fs.writeFileSync(path.join(temp, 'model.gguf'), 'must not ship');
    fs.writeFileSync(path.join(temp, 'credential.txt'), 'synthetic sentinel must not ship');
    const execute = (cmd, args) => {
      if (cmd === 'strip') return; // Do not run a synthetic executable through strip.
      return execFileSync(cmd, args);
    };
    const provenance = { source_head: 'a'.repeat(40), source_tree: 'b'.repeat(40), default_features: false };
    const result = packageQualification(binary, path.join(temp, 'out'), provenance, { repository: temp, execute });
    const members = execFileSync('tar', ['-tzf', result.output], { encoding: 'utf8' }).trim().split('\n');
    assert.deepEqual(members, ['ATTRIBUTION-README.md', 'ATTRIBUTION-inventory.json', 'LICENSE.txt', 'SHA256SUMS', 'THIRD-PARTY-NOTICES.txt', 'pumas-rpc', 'qualification.json']);
    const manifest = JSON.parse(execFileSync('tar', ['-xOzf', result.output, 'qualification.json'], { encoding: 'utf8' }));
    assert.equal(manifest.source_head, provenance.source_head);
    assert.equal(manifest.binary_sha256, createHash('sha256').update(fs.readFileSync(binary)).digest('hex'));
    assert.match(manifest.purpose, /unreleased/);
    assert.match(manifest.attribution_scope, /no Python runtime/);
    assert.throws(() => packageQualification(binary, path.join(temp, 'out'), provenance, { repository: temp, execute }), /Refusing to overwrite/);
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
});
