import path from 'node:path';

export function createLinuxPlatformService() {
  return Object.freeze({
    id: 'linux',
    npmCommand: 'npm',
    cargoCommand: 'cargo',
    releaseBackendBinary(context) {
      return path.join(context.rustTargetDir, 'release', context.appBin);
    },
  });
}
