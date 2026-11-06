import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
  SCANS, SUMMARY, checkVulnerabilityResults, loadExceptions, recipeRequirements, scanOutcome,
  summarizeCargoDeny, summarizePipAudit, summarizePnpmAudit, verifiedRuntimeRequirements,
} from './dependency-audit.mjs';
import { checkReview } from './check-artifacts.mjs';

const scan = id => SCANS.find(item => item.id === id);
const importers = new Map([['electron', '0.8.0'], ['frontend', '0.8.0']]);

function pnpmReport(...advisories) {
  return { advisories: Object.fromEntries(advisories.map((item, index) => [String(index), item])) };
}

test('pnpm importer-name matches are dropped with a reason, real findings kept', () => {
  const report = pnpmReport(
    { module_name: 'electron', github_advisory_id: 'GHSA-self', severity: 'high', findings: [{ version: '0.8.0', paths: ['electron'] }] },
    { module_name: 'electron', github_advisory_id: 'GHSA-real', severity: 'high', findings: [{ version: '43.8.1', paths: ['electron>electron'] }] },
    { module_name: 'react', github_advisory_id: 'GHSA-react', severity: 'low', findings: [{ version: '19.0.0', paths: ['frontend>react'] }] },
  );
  const result = summarizePnpmAudit(report, { importers, gate: name => name === 'electron', exceptions: [] });
  assert.deepEqual(result.dropped.map(item => item.id), ['GHSA-self']);
  assert.match(result.dropped[0].reason, /workspace importer/);
  assert.deepEqual(result.findings.map(item => [item.id, item.gating]), [['GHSA-real', true], ['GHSA-react', false]]);
  // In the all-dependencies scan only the shipped runtime blocks.
  assert.equal(scanOutcome(scan('pnpm-all'), result).status, 'findings');
  const buildOnly = summarizePnpmAudit(pnpmReport(report.advisories[2]), { importers, gate: name => name === 'electron', exceptions: [] });
  assert.equal(scanOutcome(scan('pnpm-all'), buildOnly).status, 'recorded');
  // A production finding always blocks.
  assert.equal(scanOutcome(scan('pnpm-prod'), buildOnly).status, 'findings');
});

test('pnpm errors and missing reports are scan errors, never a pass', () => {
  const result = summarizePnpmAudit({ error: { message: 'registry unreachable' } }, { importers, gate: () => true, exceptions: [] });
  assert.equal(result.complete, false);
  assert.deepEqual(scanOutcome(scan('pnpm-prod'), result), { status: 'error', reason: 'registry unreachable' });
});

test('accepted exceptions need a reason and suppress only their own finding', () => {
  assert.throws(() => loadExceptions(JSON.stringify({ schema_version: 1, exceptions: [{ ecosystem: 'npm', id: 'GHSA-x', package: 'a' }] })), /reason/);
  assert.throws(() => loadExceptions(JSON.stringify({ exceptions: [] })), /invalid/);
  const exceptions = loadExceptions(JSON.stringify({ schema_version: 1, exceptions: [
    { ecosystem: 'pypi', id: 'CVE-1', package: 'pillow', reason: 'decoder path is never reached' },
  ] }));
  const report = { dependencies: [
    { name: 'pillow', version: '12.3.0', vulns: [{ id: 'PYSEC-1', aliases: ['CVE-1'] }, { id: 'PYSEC-2', aliases: [] }] },
  ] };
  const result = summarizePipAudit(report, { exceptions });
  assert.equal(result.findings[0].accepted, 'decoder path is never reached');
  assert.equal(result.findings[1].accepted, undefined);
  assert.equal(scanOutcome(scan('pip-audio-recipe'), result).status, 'findings');
  report.dependencies[0].vulns.pop();
  assert.equal(scanOutcome(scan('pip-audio-recipe'), summarizePipAudit(report, { exceptions })).status, 'recorded');
});

test('pip scans fail closed on unaudited packages only when gating', () => {
  const report = { dependencies: [{ name: 'nunchaku', version: '1.2.0', skip_reason: 'not on PyPI' }, { name: 'numpy', version: '2.5.3', vulns: [] }] };
  const result = summarizePipAudit(report, { exceptions: [] });
  assert.equal(scanOutcome(scan('pip-audio-recipe'), result).status, 'incomplete');
  assert.equal(scanOutcome(scan('pip-verified-runtimes'), result).status, 'pass');
  assert.equal(scanOutcome(scan('pip-dev-lock'), summarizePipAudit({ dependencies: [
    { name: 'starlette', version: '0.50.0', vulns: [{ id: 'PYSEC-3' }, { id: 'PYSEC-3' }] }] }, { exceptions: [] })).status, 'recorded');
  assert.equal(summarizePipAudit({}, { exceptions: [] }).complete, false);
});

test('requirement inputs audit public versions and say so', () => {
  const recipe = recipeRequirements({ artifacts: [{ name: 'torch', version: '2.10.0+cpu' }, { name: 'numpy', version: '2.5.3' }] });
  assert.equal(recipe.text, 'torch==2.10.0\nnumpy==2.5.3\n');
  assert.deepEqual(recipe.notes, ['torch 2.10.0+cpu audited as public version 2.10.0']);
  assert.throws(() => recipeRequirements({ artifacts: [] }), /no artifacts/);
  const verified = verifiedRuntimeRequirements({ schema_version: 1, records: [
    { packages: { torch: '2.9.1+cu130', psutil: '7.2.1' } }, { packages: { psutil: '7.2.1' } }] });
  assert.equal(verified.text, 'psutil==7.2.1\ntorch==2.9.1\n');
  // The real recipe and records are well formed.
  const root = new URL('../../', import.meta.url);
  recipeRequirements(JSON.parse(fs.readFileSync(new URL('rust/crates/pumas-core/src/runtime_read_source/audio_candidate_recipe.json', root))));
  verifiedRuntimeRequirements(JSON.parse(fs.readFileSync(new URL('torch-server/runtime/verified-runtimes.json', root))));
  loadExceptions(fs.readFileSync(new URL('scripts/release/dependency-audit-exceptions.json', root), 'utf8'));
});

test('cargo-deny errors become findings; a missing summary is an error', () => {
  const diagnostic = { type: 'diagnostic', fields: { severity: 'error', code: 'vulnerability', message: 'bad',
    advisory: { id: 'RUSTSEC-2026-0001', package: 'zstd-safe' }, graphs: [{ Krate: { name: 'zstd-safe', version: '7.2.4' } }] } };
  const note = { type: 'diagnostic', fields: { severity: 'note', code: 'license-accepted' } };
  const summary = { type: 'summary', fields: { advisories: { errors: 1 } } };
  const result = summarizeCargoDeny([diagnostic, note, summary].map(item => JSON.stringify(item)).join('\n'));
  assert.deepEqual(result.findings.map(item => [item.id, item.package, item.version]), [['RUSTSEC-2026-0001', 'zstd-safe', '7.2.4']]);
  assert.equal(scanOutcome(scan('cargo-deny'), result).status, 'findings');
  assert.equal(scanOutcome(scan('cargo-deny'), summarizeCargoDeny('error: failed to fetch advisory db\n')).status, 'error');
  assert.equal(scanOutcome(scan('cargo-deny'), summarizeCargoDeny(JSON.stringify(summary))).status, 'pass');
});

function results(t, { outcome = 'pass', commit = 'a'.repeat(40), omit } = {}) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-vuln-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const scans = SCANS.filter(item => item.id !== omit).map(item => ({ ...item, status: outcome === 'pass' ? 'pass' : 'findings', reason: 'fixture' }));
  for (const item of scans) fs.writeFileSync(path.join(directory, item.report), '{}');
  fs.writeFileSync(path.join(directory, SUMMARY), JSON.stringify({
    schema_version: 1, review: 'current-vulnerability-results', source: { commit },
    tools: { 'cargo-deny': 'cargo-deny 0.20.2', pnpm: '10.33.0', 'pip-audit': 'pip-audit 2.10.1' }, outcome, scans,
  }));
  return directory;
}

test('review evidence must be complete, from the candidate source, and passing when required', t => {
  const commit = 'a'.repeat(40);
  assert.equal(checkVulnerabilityResults(results(t)).outcome, 'pass');
  assert.equal(checkReview(results(t), 'current-vulnerability-results', { expectedSource: commit, requirePass: true }).outcome, 'pass');
  assert.throws(() => checkVulnerabilityResults(results(t), { expectedSource: 'b'.repeat(40) }), /not the candidate source/);
  assert.throws(() => checkVulnerabilityResults(results(t, { omit: 'pip-dev-lock' })), /expected scans/);
  const failing = results(t, { outcome: 'fail' });
  assert.equal(checkVulnerabilityResults(failing).outcome, 'fail');
  assert.throws(() => checkVulnerabilityResults(failing, { requirePass: true }), /outcome fail: cargo-deny \(findings: fixture\)/);
  const empty = results(t);
  fs.writeFileSync(path.join(empty, 'cargo-deny.jsonl'), '');
  assert.throws(() => checkVulnerabilityResults(empty), /cargo-deny report is missing or empty/);
  assert.throws(() => checkVulnerabilityResults(fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-vuln-empty-'))), /missing or invalid summary.json/);
});

test('every required release review is either produced by CI or named as a maintainer review', () => {
  assert.throws(() => checkReview('.', 'project-license-presence'), /maintainer review/);
  assert.throws(() => checkReview('.', 'not-a-review'), /Unknown release review/);
});
