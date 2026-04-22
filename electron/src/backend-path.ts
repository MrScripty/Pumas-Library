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

export function resolveBackendBinaryPath(options: BackendBinaryPathOptions): string {
  const overridePath = options.overridePath?.trim();
  if (overridePath) {
    return path.resolve(overridePath);
  }

  const binaryName = backendBinaryName(options.platform);

  if (options.isPackaged) {
    return path.join(options.resourcesPath, binaryName);
  }

  return path.join(options.sourceRoot, 'rust', 'target', options.defaultBuildProfile, binaryName);
}
