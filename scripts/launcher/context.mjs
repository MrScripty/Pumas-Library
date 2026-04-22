import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defaultDisplayName } from './contract.mjs';

const moduleDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(moduleDir, '..', '..');

export function createLauncherContext() {
  return {
    repoRoot,
    displayName: defaultDisplayName(),
    appBin: 'pumas-rpc',
    rustManifestPath: path.join(repoRoot, 'rust', 'Cargo.toml'),
    rustTargetDir: path.join(repoRoot, 'rust', 'target'),
    frontendDir: path.join(repoRoot, 'frontend'),
    frontendDistIndex: path.join(repoRoot, 'frontend', 'dist', 'index.html'),
    electronDir: path.join(repoRoot, 'electron'),
    electronDistMain: path.join(repoRoot, 'electron', 'dist', 'main.js'),
    torchServerDir: path.join(repoRoot, 'torch-server'),
    torchServerTestsDir: path.join(repoRoot, 'torch-server', 'tests'),
  };
}
