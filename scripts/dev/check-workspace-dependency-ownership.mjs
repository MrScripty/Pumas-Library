#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const moduleDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(moduleDir, '..', '..');

const workspaceToolRequirements = [
  {
    packagePath: 'frontend/package.json',
    requiredDevDependencies: [
      '@eslint/js',
      '@types/node',
      '@types/react',
      '@types/react-dom',
      '@vitejs/plugin-react',
      '@vitest/coverage-v8',
      'eslint',
      'eslint-plugin-jsx-a11y',
      'eslint-plugin-react',
      'glob',
      'jsdom',
      'typescript',
      'typescript-eslint',
      'vite',
      'vitest',
    ],
  },
  {
    packagePath: 'electron/package.json',
    requiredDevDependencies: [
      '@eslint/js',
      '@types/node',
      'electron',
      'electron-builder',
      'eslint',
      'typescript',
      'typescript-eslint',
    ],
  },
];

const violations = [];
const rootManifest = readJson('package.json');

for (const field of ['dependencies', 'devDependencies']) {
  if (rootManifest[field] && Object.keys(rootManifest[field]).length > 0) {
    violations.push(`root package.json must not own ${field}; declare tools in the workspace that runs them`);
  }
}

for (const requirement of workspaceToolRequirements) {
  const manifest = readJson(requirement.packagePath);
  const devDependencies = manifest.devDependencies ?? {};

  for (const dependencyName of requirement.requiredDevDependencies) {
    if (!Object.hasOwn(devDependencies, dependencyName)) {
      violations.push(`${requirement.packagePath} must declare devDependency ${dependencyName}`);
    }
  }
}

if (violations.length > 0) {
  console.error('\nWorkspace dependency ownership violations:\n');
  for (const violation of violations) {
    console.error(`  ${violation}`);
  }
  process.exit(1);
}

console.log('Workspace dependency ownership checks passed');

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(repoRoot, relativePath), 'utf8'));
}
