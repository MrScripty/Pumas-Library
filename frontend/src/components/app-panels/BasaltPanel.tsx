import { useMemo } from 'react';
import { ModelManager, type ModelManagerProps } from '../ModelManager';
import { filterProviderCompatibleModelGroups } from '../../utils/runtimeProviderDescriptors';
import { RuntimeProfileSettingsSection } from './sections/RuntimeProfileSettingsSection';
import { VersionManagementPanel } from './VersionManagementPanel';
import type { AppVersionState } from '../../utils/appVersionState';

export interface BasaltPanelProps {
  appDisplayName: string;
  versions: AppVersionState;
  showVersionManager: boolean;
  onShowVersionManager: (show: boolean) => void;
  diskSpacePercent: number | null;
  modelManagerProps: ModelManagerProps;
}

export function BasaltPanel({
  appDisplayName,
  versions,
  showVersionManager,
  onShowVersionManager,
  diskSpacePercent,
  modelManagerProps,
}: BasaltPanelProps) {
  const modelGroups = useMemo(
    () => filterProviderCompatibleModelGroups(modelManagerProps.modelGroups, 'basalt'),
    [modelManagerProps.modelGroups]
  );
  const isManagerOpen = versions.isSupported && showVersionManager;

  return (
    <div className="flex-1 flex flex-col gap-4 p-6 overflow-hidden">
      <VersionManagementPanel
        appDisplayName={appDisplayName}
        versions={versions}
        showManager={showVersionManager}
        onShowManager={onShowVersionManager}
        diskSpacePercent={diskSpacePercent}
      />
      {!isManagerOpen && <RuntimeProfileSettingsSection provider="basalt" />}
      {!isManagerOpen && <ModelManager {...modelManagerProps} modelGroups={modelGroups} />}
    </div>
  );
}
