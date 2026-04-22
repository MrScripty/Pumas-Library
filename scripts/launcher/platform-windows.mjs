import path from 'node:path';

export function createWindowsPlatformService() {
  return Object.freeze({
    id: 'windows',
    corepackCommand: 'corepack.cmd',
    cargoCommand: 'cargo.exe',
    pythonCommand: 'py.exe',
    pythonModuleArgs(moduleName, args = []) {
      return ['-3', '-m', moduleName, ...args];
    },
    debugBackendBinary(context) {
      return path.join(context.rustTargetDir, 'debug', `${context.appBin}.exe`);
    },
    releaseBackendBinary(context) {
      return path.join(context.rustTargetDir, 'release', `${context.appBin}.exe`);
    },
  });
}
