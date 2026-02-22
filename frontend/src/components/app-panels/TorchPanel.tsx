import { AppConnectionInfo } from '../AppConnectionInfo';
import { ModelManager, type ModelManagerProps } from '../ModelManager';
import { VersionManagementPanel } from './VersionManagementPanel';
import { TorchModelSlotsSection } from './sections/TorchModelSlotsSection';
import { TorchServerConfigSection } from './sections/TorchServerConfigSection';
import type { AppVersionState } from '../../utils/appVersionState';
import type { ModelCategory } from '../../types/apps';

export interface TorchPanelProps {
  appDisplayName: string;
  connectionUrl?: string;
  versions: AppVersionState;
  showVersionManager: boolean;
  onShowVersionManager: (show: boolean) => void;
  activeShortcutState?: { menu: boolean; desktop: boolean };
  diskSpacePercent: number;
  modelManagerProps: ModelManagerProps;
  isTorchRunning: boolean;
  modelGroups: ModelCategory[];
}

export function TorchPanel({
  appDisplayName,
  connectionUrl,
  versions,
  showVersionManager,
  onShowVersionManager,
  activeShortcutState,
  diskSpacePercent,
  modelManagerProps,
  isTorchRunning,
  modelGroups,
}: TorchPanelProps) {
  const isManagerOpen = versions.isSupported && showVersionManager;

  return (
    <div className="flex-1 flex flex-col gap-4 p-6 overflow-hidden">
      <div className="w-full flex flex-col gap-4">
        <VersionManagementPanel
          appDisplayName={appDisplayName}
          versions={versions}
          showManager={showVersionManager}
          onShowManager={onShowVersionManager}
          activeShortcutState={activeShortcutState}
          diskSpacePercent={diskSpacePercent}
        />
        {!isManagerOpen && connectionUrl && (
          <AppConnectionInfo url={connectionUrl} />
        )}
      </div>

      {!isManagerOpen && isTorchRunning && (
        <TorchServerConfigSection connectionUrl={connectionUrl} />
      )}

      {!isManagerOpen && connectionUrl && (
        <TorchModelSlotsSection
          connectionUrl={connectionUrl}
          isRunning={isTorchRunning}
          modelGroups={modelGroups}
        />
      )}

      {!isManagerOpen && <ModelManager {...modelManagerProps} />}
    </div>
  );
}
