/**
 * Path-free confirmation/result contract for the desktop metadata upgrade,
 * declared once in electron/contracts/launcher-root.schema.json.
 */
export type {
  LauncherRootReadyState as LibraryUpgradeReadyState,
  LibraryUpgradeConfirmation,
  LibraryUpgradeResult,
} from './desktop-contract';
