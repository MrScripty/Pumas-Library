#!/usr/bin/env node
// Produces and checks the `current-vulnerability-results` release review
// (scripts/release/artifact-plan.json release_metadata.required_reviews).
//
//   node scripts/release/dependency-audit.mjs run --output DIR --cargo-deny BIN [--python EXE]
//   node scripts/release/dependency-audit.mjs check DIR [--expected-source SHA] [--require-pass]
//
// `run` scans the Rust, JavaScript and Python inputs, writes each tool's raw
// report plus summary.json into DIR, and exits non-zero when a gating scan has
// findings or any scan could not complete. CI decides whether that exit blocks
// (docs/SECURITY.md#dependency-scans): advisory on pull requests, blocking on
// version tags and candidate dispatches.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));

export const REVIEW_ID = 'current-vulnerability-results';
export const SUMMARY = 'summary.json';
const exceptionsPath = 'scripts/release/dependency-audit-exceptions.json';
const verifiedRuntimesPath = 'torch-server/runtime/verified-runtimes.json';
const devLockPath = 'torch-server/requirements-dev.lock';
// Declared as devDependencies but shipped inside every desktop package.
const shippedDevRuntimes = ['electron'];

// Every scan the review must contain, in order. `gating` scans decide the
// outcome; the others are recorded for the reviewer with the stated reason.
export const SCANS = [
  { id: 'cargo-deny', ecosystem: 'cargo', input: 'rust/Cargo.lock', report: 'cargo-deny.jsonl', gating: true,
    scope: 'Rust workspace, all features, the three release targets; advisories and licenses (rust/deny.toml)' },
  { id: 'pnpm-prod', ecosystem: 'npm', input: 'pnpm-lock.yaml', report: 'pnpm-audit-prod.json', gating: true,
    scope: 'Production dependencies of the frontend and Electron workspaces' },
  { id: 'pnpm-all', ecosystem: 'npm', input: 'pnpm-lock.yaml', report: 'pnpm-audit-all.json', gating: 'shipped-runtime',
    scope: `All dependencies; findings gate only for shipped runtimes declared as devDependencies (${shippedDevRuntimes.join(', ')}), the rest are build-only` },
  { id: 'pip-dev-lock', ecosystem: 'pypi', input: devLockPath, report: 'pip-audit-dev-lock.json', gating: false,
    scope: 'Development and CI tooling lock; not shipped' },
  { id: 'pip-verified-runtimes', ecosystem: 'pypi', input: verifiedRuntimesPath, report: 'pip-audit-verified-runtimes.json', gating: false,
    scope: 'Verified Torch combinations offered as install shortcuts; installs resolve current versions, so these are evidence, not pins' },
];

function fail(message) {
  throw new Error(message);
}

export function loadExceptions(text) {
  const data = JSON.parse(text);
  if (data?.schema_version !== 1 || !Array.isArray(data.exceptions)) fail('invalid dependency-audit exceptions file');
  for (const item of data.exceptions) {
    if (!['npm', 'pypi'].includes(item?.ecosystem) || typeof item.id !== 'string' || !item.id
      || typeof item.package !== 'string' || !item.package || typeof item.reason !== 'string' || item.reason.length < 10) {
      fail(`dependency-audit exception needs ecosystem, id, package and a reason: ${JSON.stringify(item)}`);
    }
  }
  return data.exceptions;
}

function accepted(exceptions, ecosystem, finding) {
  return exceptions.find(item => item.ecosystem === ecosystem && item.package === finding.package
    && (item.id === finding.id || finding.aliases?.includes(item.id)));
}

// Strip a PEP 440 local segment (`+cpu`); PyPI advisories name public versions.
function publicVersion(version) {
  return String(version).split('+')[0];
}

export function verifiedRuntimeRequirements(verified) {
  if (verified?.schema_version !== 1 || !Array.isArray(verified.records)) fail('invalid verified-runtimes.json');
  const notes = [];
  const pins = new Map();
  for (const record of verified.records) {
    for (const [name, raw] of Object.entries(record.packages ?? {})) {
      const version = publicVersion(raw);
      if (version !== raw) notes.push(`${name} ${raw} audited as public version ${version}`);
      pins.set(`${name}==${version}`, true);
    }
  }
  return { text: `${[...pins.keys()].sort().join('\n')}\n`, notes };
}

export function summarizeCargoDeny(jsonl) {
  const findings = [];
  let summary = null;
  for (const line of jsonl.split('\n').filter(Boolean)) {
    let entry;
    try { entry = JSON.parse(line); } catch { continue; }
    if (entry.type === 'summary') summary = entry.fields;
    if (entry.type !== 'diagnostic' || entry.fields?.severity !== 'error') continue;
    const fields = entry.fields;
    const krate = fields.graphs?.[0]?.Krate;
    findings.push({
      id: fields.advisory?.id ?? fields.code ?? 'unknown',
      kind: fields.code ?? 'error',
      package: fields.advisory?.package ?? krate?.name ?? fields.labels?.[0]?.span ?? 'unknown',
      version: krate?.version ?? null,
      title: fields.message ?? '',
    });
  }
  if (!summary) return { complete: false, reason: 'cargo-deny produced no summary', findings };
  return { complete: true, findings };
}

function workspaceImporters(rootDir) {
  const text = fs.readFileSync(path.join(rootDir, 'pnpm-workspace.yaml'), 'utf8');
  const block = text.match(/^packages:\n((?:\s+-\s+.+\n)+)/m)?.[1] ?? '';
  return new Map([...block.matchAll(/-\s+['"]?([^'"\n]+)['"]?/g)].map(match => {
    const dir = match[1].trim();
    return [dir, JSON.parse(fs.readFileSync(path.join(rootDir, dir, 'package.json'), 'utf8')).version];
  }));
}

// pnpm audit reports a workspace importer whose directory shares a public
// package name (our `electron/` workspace) as that package at the workspace's
// own version. Such a finding describes no installed dependency.
export function summarizePnpmAudit(report, { importers, gate, exceptions }) {
  if (!report || typeof report !== 'object' || !report.advisories || typeof report.advisories !== 'object') {
    return { complete: false, reason: report?.error?.message ?? 'pnpm audit produced no advisories object', findings: [] };
  }
  const findings = [];
  const dropped = [];
  for (const advisory of Object.values(report.advisories)) {
    const installed = (advisory.findings ?? []).filter(finding => !(
      finding.paths?.length > 0
      && finding.paths.every(item => importers.get(item) === finding.version && item === advisory.module_name)));
    if (installed.length === 0) {
      dropped.push({ id: advisory.github_advisory_id ?? String(advisory.id), package: advisory.module_name,
        reason: 'pnpm matched a workspace importer directory by name; no such dependency is installed' });
      continue;
    }
    const finding = {
      id: advisory.github_advisory_id ?? String(advisory.id),
      aliases: advisory.cves ?? [],
      package: advisory.module_name,
      version: [...new Set(installed.map(item => item.version))].sort().join(', '),
      severity: advisory.severity,
      title: advisory.title ?? '',
      gating: gate(advisory.module_name),
    };
    const exception = accepted(exceptions, 'npm', finding);
    if (exception) finding.accepted = exception.reason;
    findings.push(finding);
  }
  return { complete: true, findings, dropped };
}

export function summarizePipAudit(report, { exceptions, notes = [] }) {
  if (!Array.isArray(report?.dependencies)) {
    return { complete: false, reason: 'pip-audit produced no dependency list', findings: [] };
  }
  const findings = [];
  const unaudited = [];
  for (const dependency of report.dependencies) {
    if (dependency.skip_reason) unaudited.push({ package: dependency.name, reason: dependency.skip_reason });
    const seen = new Set();
    for (const vuln of dependency.vulns ?? []) {
      if (seen.has(vuln.id)) continue;
      seen.add(vuln.id);
      const finding = {
        id: vuln.id,
        aliases: vuln.aliases ?? [],
        package: dependency.name,
        version: dependency.version,
        fix_versions: vuln.fix_versions ?? [],
      };
      const exception = accepted(exceptions, 'pypi', finding);
      if (exception) finding.accepted = exception.reason;
      findings.push(finding);
    }
  }
  return { complete: true, findings, unaudited, notes };
}

// A scan blocks when it is gating and has an unaccepted finding, or when it
// could not audit every package it was given.
export function scanOutcome(scan, result) {
  if (!result.complete) return { status: 'error', reason: result.reason };
  const blocking = result.findings.filter(finding => !finding.accepted
    && (scan.gating === true || (scan.gating === 'shipped-runtime' && finding.gating)));
  if (scan.gating === true && result.unaudited?.length) {
    return { status: 'incomplete', reason: `${result.unaudited.length} package(s) could not be audited` };
  }
  if (blocking.length) return { status: 'findings', reason: `${blocking.length} unaccepted finding(s) in a shipped closure` };
  if (result.findings.length) {
    return { status: 'recorded', reason: scan.gating === false ? `informational: ${scan.scope}` : 'only accepted or build-only findings' };
  }
  return { status: 'pass', reason: 'no findings' };
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, ...options });
  if (result.error) return { status: null, stdout: '', stderr: String(result.error) };
  return result;
}

function toolVersion(command, args) {
  const result = run(command, args);
  return result.status === 0 ? result.stdout.trim() || result.stderr.trim() : null;
}

function readJson(file) {
  try { return JSON.parse(fs.readFileSync(file, 'utf8')); } catch { return null; }
}

function runScans({ output, cargoDeny, python }) {
  fs.mkdirSync(output);
  const exceptions = loadExceptions(fs.readFileSync(path.join(root, exceptionsPath), 'utf8'));
  const importers = workspaceImporters(root);
  const pnpm = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm';
  const results = {};

  const deny = run(cargoDeny, ['--manifest-path', 'rust/Cargo.toml', '--config', 'rust/deny.toml', '--locked',
    '--format', 'json', '--color', 'never', 'check', 'advisories', 'licenses']);
  fs.writeFileSync(path.join(output, 'cargo-deny.jsonl'), deny.stderr ?? '');
  results['cargo-deny'] = summarizeCargoDeny(deny.stderr ?? '');

  for (const [id, args, gate] of [
    ['pnpm-prod', ['audit', '--prod', '--json'], () => true],
    ['pnpm-all', ['audit', '--json'], name => shippedDevRuntimes.includes(name)],
  ]) {
    const audit = run(pnpm, args, { shell: process.platform === 'win32' });
    const report = SCANS.find(scan => scan.id === id).report;
    fs.writeFileSync(path.join(output, report), audit.stdout ?? '');
    let parsed = null;
    try { parsed = JSON.parse(audit.stdout); } catch { parsed = { error: { message: audit.stderr?.trim() || 'pnpm audit output is not JSON' } }; }
    results[id] = summarizePnpmAudit(parsed, { importers, gate, exceptions });
  }

  const inputs = path.join(output, 'inputs');
  fs.mkdirSync(inputs);
  const verified = verifiedRuntimeRequirements(JSON.parse(fs.readFileSync(path.join(root, verifiedRuntimesPath), 'utf8')));
  fs.writeFileSync(path.join(inputs, 'verified-runtimes-requirements.txt'), verified.text);
  for (const [id, args, notes] of [
    ['pip-dev-lock', ['--require-hashes', '--disable-pip', '-r', devLockPath], []],
    ['pip-verified-runtimes', ['--no-deps', '--disable-pip', '-r', path.join(inputs, 'verified-runtimes-requirements.txt')], verified.notes],
  ]) {
    const report = path.join(output, SCANS.find(scan => scan.id === id).report);
    const audit = run(python, ['-m', 'pip_audit', ...args, '--format', 'json', '--output', report, '--progress-spinner', 'off']);
    const parsed = readJson(report);
    results[id] = parsed
      ? summarizePipAudit(parsed, { exceptions, notes })
      : { complete: false, reason: audit.stderr?.trim().split('\n').pop() || 'pip-audit wrote no report', findings: [] };
  }

  const git = (...args) => run('git', args).stdout?.trim() || null;
  const scans = SCANS.map(scan => ({ ...scan, ...scanOutcome(scan, results[scan.id]), ...results[scan.id] }));
  const summary = {
    schema_version: 1,
    review: REVIEW_ID,
    generated_at: new Date().toISOString(),
    source: { commit: git('rev-parse', 'HEAD'), tree: git('rev-parse', 'HEAD^{tree}') },
    tools: {
      'cargo-deny': toolVersion(cargoDeny, ['--version']),
      pnpm: toolVersion(pnpm, ['--version']),
      'pip-audit': toolVersion(python, ['-m', 'pip_audit', '--version']),
    },
    outcome: scans.every(scan => ['pass', 'recorded'].includes(scan.status)) ? 'pass' : 'fail',
    scans,
  };
  fs.writeFileSync(path.join(output, SUMMARY), `${JSON.stringify(summary, null, 2)}\n`);
  return summary;
}

// Validates a downloaded review directory: every declared scan is present with
// a nonempty report, from the expected source, and (optionally) passing.
export function checkVulnerabilityResults(directory, { expectedSource, requirePass = false } = {}) {
  const summary = readJson(path.join(directory, SUMMARY));
  if (summary?.schema_version !== 1 || summary.review !== REVIEW_ID) fail(`${REVIEW_ID}: missing or invalid ${SUMMARY}`);
  if (expectedSource && summary.source?.commit !== expectedSource) {
    fail(`${REVIEW_ID}: scanned source ${summary.source?.commit} is not the candidate source ${expectedSource}`);
  }
  const ids = (summary.scans ?? []).map(scan => scan.id);
  if (JSON.stringify(ids) !== JSON.stringify(SCANS.map(scan => scan.id))) {
    fail(`${REVIEW_ID}: expected scans ${JSON.stringify(SCANS.map(scan => scan.id))}; found ${JSON.stringify(ids)}`);
  }
  for (const scan of summary.scans) {
    const report = path.join(directory, scan.report ?? '');
    if (!scan.report || !fs.existsSync(report) || fs.statSync(report).size === 0) fail(`${REVIEW_ID}: ${scan.id} report is missing or empty`);
  }
  const tools = Object.values(summary.tools ?? {});
  if (tools.length !== 3 || !tools.every(Boolean)) fail(`${REVIEW_ID}: a scanner version was not recorded`);
  if (requirePass && summary.outcome !== 'pass') {
    const failing = summary.scans.filter(scan => !['pass', 'recorded'].includes(scan.status));
    fail(`${REVIEW_ID}: outcome ${summary.outcome}: ${failing.map(scan => `${scan.id} (${scan.status}: ${scan.reason})`).join('; ')}`);
  }
  return summary;
}

function report(summary) {
  const lines = [`Dependency scan outcome: ${summary.outcome}`];
  for (const scan of summary.scans) {
    lines.push(`- ${scan.id} [${scan.gating === true ? 'gating' : scan.gating === false ? 'informational' : 'gating for shipped runtimes'}]: ${scan.status} (${scan.reason})`);
    const byPackage = new Map();
    for (const finding of scan.findings ?? []) {
      const key = `${finding.package} ${finding.version ?? ''}`.trim();
      byPackage.set(key, [...(byPackage.get(key) ?? []), `${finding.id}${finding.accepted ? ' (accepted)' : ''}`]);
    }
    for (const [key, ids] of byPackage) lines.push(`    ${key}: ${ids.join(', ')}`);
  }
  const text = `${lines.join('\n')}\n`;
  process.stdout.write(text);
  if (process.env.GITHUB_STEP_SUMMARY) fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, `\`\`\`\n${text}\`\`\`\n`);
}

function option(args, name, fallback) {
  const index = args.indexOf(name);
  if (index === -1) return fallback;
  if (!args[index + 1] || args[index + 1].startsWith('--')) fail(`${name} needs a value`);
  return args[index + 1];
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, ...args] = process.argv.slice(2);
  try {
    if (command === 'run') {
      const output = option(args, '--output');
      const cargoDeny = option(args, '--cargo-deny');
      if (!output || !cargoDeny) fail('run requires --output DIR and --cargo-deny BIN');
      const summary = runScans({
        output: path.resolve(output),
        cargoDeny,
        python: option(args, '--python', process.platform === 'win32' ? 'python' : 'python3'),
      });
      report(summary);
      process.exitCode = summary.outcome === 'pass' ? 0 : 1;
    } else if (command === 'check') {
      if (!args[0] || args[0].startsWith('--')) fail('check requires a results directory');
      const summary = checkVulnerabilityResults(args[0], {
        expectedSource: option(args, '--expected-source'),
        requirePass: args.includes('--require-pass'),
      });
      console.log(`Verified ${REVIEW_ID} for ${summary.source.commit}: ${summary.outcome}`);
    } else {
      fail('usage: dependency-audit.mjs run --output DIR --cargo-deny BIN [--python EXE] | check DIR [--expected-source SHA] [--require-pass]');
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 2;
  }
}
