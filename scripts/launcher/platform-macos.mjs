import path from 'node:path';

export function createMacOSPlatformService() {
  return Object.freeze({
    id: 'macos',
    npmCommand: 'npm',
    cargoCommand: 'cargo',
    releaseBackendBinary(context) {
      return path.join(context.rustTargetDir, 'release', context.appBin);
    },
  });
}
