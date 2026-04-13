import path from 'node:path';

export function createMacOSPlatformService() {
  return Object.freeze({
    id: 'macos',
    corepackCommand: 'corepack',
    cargoCommand: 'cargo',
    releaseBackendBinary(context) {
      return path.join(context.rustTargetDir, 'release', context.appBin);
    },
  });
}
