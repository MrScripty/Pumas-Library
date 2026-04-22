import path from 'node:path';

export function createLinuxPlatformService() {
  return Object.freeze({
    id: 'linux',
    corepackCommand: 'corepack',
    cargoCommand: 'cargo',
    pythonCommand: 'python3',
    pythonModuleArgs(moduleName, args = []) {
      return ['-m', moduleName, ...args];
    },
    debugBackendBinary(context) {
      return path.join(context.rustTargetDir, 'debug', context.appBin);
    },
    releaseBackendBinary(context) {
      return path.join(context.rustTargetDir, 'release', context.appBin);
    },
  });
}
