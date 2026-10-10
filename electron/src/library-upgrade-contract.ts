/** Path-free confirmation/result contract for the desktop metadata upgrade. */
export type LibraryUpgradeConfirmation = {
  oldWritersStopped: true;
  noDowngradeAccepted: true;
};
export type LibraryUpgradeReadyState = {
  status: 'ready';
  selectionAction: 'select-library' | 'correct-launch-input';
  libraryScopeId: string | null;
};
export type LibraryUpgradeResult =
  | { status: 'ready'; state: LibraryUpgradeReadyState }
  | { status: 'upgraded' }
  | { status: 'unavailable' }
  | { status: 'failed'; stage: 'upgrade' | 'open' };
