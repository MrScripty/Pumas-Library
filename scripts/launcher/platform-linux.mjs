import path from 'node:path';

export function createLinuxPlatformService() {
  return Object.freeze({
    id: 'linux',
    corepackCommand: 'corepack',
    cargoCommand: 'cargo',
    releaseBackendBinary(context) {
      return path.join(context.rustTargetDir, 'release', context.appBin);
    },
  });
}
