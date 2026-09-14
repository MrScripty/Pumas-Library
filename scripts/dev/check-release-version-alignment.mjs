#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const moduleDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(moduleDir, '..', '..');

const versions = new Map([
  ['package.json', readPackageVersion('package.json')],
  ['frontend/package.json', readPackageVersion('frontend/package.json')],
  ['electron/package.json', readPackageVersion('electron/package.json')],
  ['rust/Cargo.toml [workspace.package]', readRustWorkspaceVersion()],
]);

const uniqueVersions = new Set(versions.values());

if (uniqueVersions.size > 1) {
  console.error('\nRelease version alignment failed:\n');
  for (const [source, version] of versions) {
    console.error(`  ${source}: ${version}`);
  }
  process.exit(1);
}

const version = [...uniqueVersions][0];
const tag = process.argv[2] ?? (process.env.GITHUB_REF_TYPE === 'tag' ? process.env.GITHUB_REF_NAME : undefined);
if (tag !== undefined && tag !== `v${version}`) {
  console.error(`Release tag ${tag} does not match manifest version v${version}`);
  process.exit(1);
}

console.log(`Release version alignment checks passed (${version})`);

function readPackageVersion(relativePath) {
  const manifest = JSON.parse(fs.readFileSync(path.join(repoRoot, relativePath), 'utf8'));
  if (typeof manifest.version !== 'string' || manifest.version.trim() === '') {
    throw new Error(`${relativePath} is missing a string version field`);
  }
  return manifest.version;
}

function readRustWorkspaceVersion() {
  const cargoToml = fs.readFileSync(path.join(repoRoot, 'rust', 'Cargo.toml'), 'utf8');
  const workspacePackageMatch = cargoToml.match(
    /^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m
  );

  if (!workspacePackageMatch) {
    throw new Error('rust/Cargo.toml is missing [workspace.package] version');
  }

  return workspacePackageMatch[1];
}
