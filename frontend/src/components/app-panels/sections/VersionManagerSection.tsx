/**
 * Version Manager Section for GenericAppPanel.
 *
 * Wraps VersionManagementPanel for use in plugin-driven layouts.
 */

import { VersionManagementPanel } from '../VersionManagementPanel';
import type { AppVersionState } from '../../../utils/appVersionState';

export interface VersionManagerSectionProps {
  appDisplayName: string;
  versions: AppVersionState;
  showManager: boolean;
  onShowManager: (show: boolean) => void;
  activeShortcutState?: { menu: boolean; desktop: boolean };
  diskSpacePercent?: number;
  backLabel?: string;
}

export function VersionManagerSection({
  appDisplayName,
  versions,
  showManager,
  onShowManager,
  activeShortcutState,
  diskSpacePercent = 0,
  backLabel,
}: VersionManagerSectionProps) {
  if (!versions.isSupported) {
    return null;
  }

  return (
    <VersionManagementPanel
      appDisplayName={appDisplayName}
      backLabel={backLabel}
      versions={versions}
      showManager={showManager}
      onShowManager={onShowManager}
      activeShortcutState={activeShortcutState}
      diskSpacePercent={diskSpacePercent}
    />
  );
}
