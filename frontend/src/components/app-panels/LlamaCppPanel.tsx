import { AppConnectionInfo } from '../AppConnectionInfo';
import type { ModelManagerProps } from '../ModelManager';
import { VersionManagementPanel } from './VersionManagementPanel';
import { LlamaCppModelLibrarySection } from './sections/LlamaCppModelLibrarySection';
import { RuntimeProfileSettingsSection } from './sections/RuntimeProfileSettingsSection';
import type { AppVersionState } from '../../utils/appVersionState';

export interface LlamaCppPanelProps {
  appDisplayName: string;
  connectionUrl?: string;
  versions: AppVersionState;
  showVersionManager: boolean;
  onShowVersionManager: (show: boolean) => void;
  activeShortcutState?: { menu: boolean; desktop: boolean };
  diskSpacePercent: number;
  modelManagerProps: ModelManagerProps;
}

export function LlamaCppPanel({
  appDisplayName,
  connectionUrl,
  versions,
  showVersionManager,
  onShowVersionManager,
  activeShortcutState,
  diskSpacePercent,
  modelManagerProps,
}: LlamaCppPanelProps) {
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

      {!isManagerOpen && <RuntimeProfileSettingsSection provider="llama_cpp" />}

      {!isManagerOpen && (
        <LlamaCppModelLibrarySection
          excludedModels={modelManagerProps.excludedModels}
          modelGroups={modelManagerProps.modelGroups}
          servedModels={modelManagerProps.servedModels}
          starredModels={modelManagerProps.starredModels}
          onToggleLink={modelManagerProps.onToggleLink}
          onToggleStar={modelManagerProps.onToggleStar}
        />
      )}
    </div>
  );
}
