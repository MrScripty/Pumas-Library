import * as fs from 'fs';
import * as path from 'path';

export interface LauncherRootResolutionOptions {
  appImagePath?: string;
  argv?: string[];
  devRoot: string;
  execPath: string;
  isPackaged: boolean;
  userDataPath: string;
}

interface PersistedLauncherRootConfig {
  launcherRoot: string;
  selectedPath?: string;
  updatedAt: string;
}

const LAUNCHER_ROOT_OVERRIDE_FILENAME = 'launcher-root.json';

export function resolveLauncherRoot(options: LauncherRootResolutionOptions): string {
  const override = resolveLauncherRootOverride(options.argv ?? process.argv, process.env);
  if (override) {
    return override;
  }

  const persistedOverride = readPersistedLauncherRootOverride(options.userDataPath);
  if (persistedOverride) {
    return persistedOverride;
  }

  const appImagePortableRoot = options.appImagePath
    ? path.join(path.dirname(options.appImagePath), 'pumas-data')
    : undefined;

  if (appImagePortableRoot && isExistingLauncherRoot(appImagePortableRoot)) {
    return appImagePortableRoot;
  }

  const candidateStarts = new Set<string>();
  if (options.appImagePath) {
    candidateStarts.add(path.dirname(options.appImagePath));
  }
  if (options.isPackaged) {
    candidateStarts.add(path.dirname(options.execPath));
  }

  for (const startDir of candidateStarts) {
    const existingRoot = findLauncherRootFrom(startDir);
    if (existingRoot) {
      return existingRoot;
    }
  }

  if (appImagePortableRoot) {
    return appImagePortableRoot;
  }

  if (options.isPackaged) {
    return options.userDataPath;
  }

  return options.devRoot;
}

export function persistLauncherRootOverride(
  userDataPath: string,
  selectedPath: string
): PersistedLauncherRootConfig {
  const launcherRoot = normalizeLauncherRootSelection(selectedPath);
  if (!launcherRoot) {
    throw new Error(
      'Selected path must be a launcher root, shared-resources directory, or shared-resources/models directory.'
    );
  }

  fs.mkdirSync(userDataPath, { recursive: true });

  const config: PersistedLauncherRootConfig = {
    launcherRoot,
    selectedPath: path.resolve(selectedPath),
    updatedAt: new Date().toISOString(),
  };

  fs.writeFileSync(
    launcherRootOverrideConfigPath(userDataPath),
    `${JSON.stringify(config, null, 2)}\n`,
    'utf8'
  );

  return config;
}

export function launcherRootOverrideConfigPath(userDataPath: string): string {
  return path.join(userDataPath, LAUNCHER_ROOT_OVERRIDE_FILENAME);
}

function resolveLauncherRootOverride(argv: string[], env: NodeJS.ProcessEnv): string | null {
  const envOverride = env.PUMAS_LAUNCHER_ROOT?.trim();
  if (envOverride) {
    return path.resolve(envOverride);
  }

  for (let index = 0; index < argv.length; index += 1) {
    const current = argv[index];

    if (current === '--launcher-root') {
      const next = argv[index + 1];
      if (next && !next.startsWith('--')) {
        return path.resolve(next);
      }
      continue;
    }

    if (current.startsWith('--launcher-root=')) {
      const value = current.slice('--launcher-root='.length).trim();
      if (value) {
        return path.resolve(value);
      }
    }
  }

  return null;
}

function readPersistedLauncherRootOverride(userDataPath: string): string | null {
  const configPath = launcherRootOverrideConfigPath(userDataPath);
  if (!fs.existsSync(configPath)) {
    return null;
  }

  try {
    const parsed = JSON.parse(fs.readFileSync(configPath, 'utf8')) as Partial<PersistedLauncherRootConfig>;
    const configuredRoot = typeof parsed.launcherRoot === 'string' ? parsed.launcherRoot : '';
    if (!configuredRoot) {
      return null;
    }

    const normalized = normalizeLauncherRootSelection(configuredRoot);
    return normalized;
  } catch {
    return null;
  }
}

function normalizeLauncherRootSelection(selectedPath: string): string | null {
  const resolved = path.resolve(selectedPath);
  const candidates = [resolved];

  if (path.basename(resolved) === 'models' && path.basename(path.dirname(resolved)) === 'shared-resources') {
    candidates.push(path.dirname(path.dirname(resolved)));
  }

  if (path.basename(resolved) === 'shared-resources') {
    candidates.push(path.dirname(resolved));
  }

  const ancestorCandidate = findLauncherRootFrom(resolved);
  if (ancestorCandidate) {
    candidates.push(ancestorCandidate);
  }

  for (const candidate of candidates) {
    if (isExistingLauncherRoot(candidate)) {
      return candidate;
    }
  }

  return null;
}

function findLauncherRootFrom(startDir: string): string | null {
  let current = path.resolve(startDir);

  while (true) {
    if (isExistingLauncherRoot(current)) {
      return current;
    }

    const parent = path.dirname(current);
    if (parent === current) {
      return null;
    }
    current = parent;
  }
}

function isExistingLauncherRoot(candidate: string): boolean {
  return (
    fs.existsSync(path.join(candidate, 'shared-resources', 'models')) ||
    (fs.existsSync(path.join(candidate, 'launcher-data')) &&
      fs.existsSync(path.join(candidate, 'shared-resources')))
  );
}
