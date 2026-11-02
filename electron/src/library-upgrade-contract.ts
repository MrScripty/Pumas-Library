/** Path-free confirmation/result contract for the desktop metadata upgrade. */
export type LibraryUpgradeConfirmation = {
  oldWritersStopped: true;
  noDowngradeAccepted: true;
};

// The result is published to the renderer; it is declared once in
// electron/contracts and generated.
export type {
  LauncherRootReadyState as LibraryUpgradeReadyState,
  LibraryUpgradeResult,
} from './generated/desktop-contract';
