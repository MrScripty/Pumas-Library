import { createLinuxPlatformService } from './platform-linux.mjs';
import { createMacOSPlatformService } from './platform-macos.mjs';
import { createWindowsPlatformService } from './platform-windows.mjs';

export function createPlatformService(platform = process.platform) {
  switch (platform) {
    case 'win32':
      return createWindowsPlatformService();
    case 'darwin':
      return createMacOSPlatformService();
    default:
      return createLinuxPlatformService();
  }
}
