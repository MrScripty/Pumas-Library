import * as path from 'path';

type BuildProfile = 'debug' | 'release';

export interface BackendBinaryPathOptions {
  defaultBuildProfile: BuildProfile;
  isPackaged: boolean;
  platform?: NodeJS.Platform;
  resourcesPath: string;
  sourceRoot: string;
  overridePath?: string;
}

export function backendBinaryName(platform: NodeJS.Platform = process.platform): string {
  return platform === 'win32' ? 'pumas-rpc.exe' : 'pumas-rpc';
}

/**
 * A packaged desktop always runs the backend it shipped with: its generated
 * response validators are closed and match only that build, so the
 * `PUMAS_RPC_BINARY` override is a source-checkout affordance.
 */
export function resolveBackendBinaryPath(options: BackendBinaryPathOptions): string {
  const binaryName = backendBinaryName(options.platform);

  if (options.isPackaged) {
    return path.join(options.resourcesPath, binaryName);
  }

  const overridePath = options.overridePath?.trim();
  if (overridePath) {
    return path.resolve(overridePath);
  }

  return path.join(options.sourceRoot, 'rust', 'target', options.defaultBuildProfile, binaryName);
}

/** True when a backend override was supplied but a packaged build ignores it. */
export function isIgnoredPackagedOverride(options: Pick<BackendBinaryPathOptions, 'isPackaged' | 'overridePath'>): boolean {
  return options.isPackaged && Boolean(options.overridePath?.trim());
}
