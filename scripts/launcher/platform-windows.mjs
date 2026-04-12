import path from 'node:path';

export function createWindowsPlatformService() {
  return Object.freeze({
    id: 'windows',
    npmCommand: 'npm.cmd',
    cargoCommand: 'cargo.exe',
    releaseBackendBinary(context) {
      return path.join(context.rustTargetDir, 'release', `${context.appBin}.exe`);
    },
  });
}
