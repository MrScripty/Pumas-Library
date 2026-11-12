import type { ModelManagerProps } from '../ModelManager';
import { VersionManagementPanel } from './VersionManagementPanel';
import { OnnxRuntimeModelLibrarySection } from './sections/OnnxRuntimeModelLibrarySection';
import { RuntimeProfileSettingsSection } from './sections/RuntimeProfileSettingsSection';
import type { AppVersionState } from '../../utils/appVersionState';

export interface OnnxRuntimePanelProps {
  appDisplayName: string;
  versions: AppVersionState;
  showVersionManager: boolean;
  onShowVersionManager: (show: boolean) => void;
  diskSpacePercent: number | null;
  modelManagerProps: ModelManagerProps;
}

export function OnnxRuntimePanel({
  appDisplayName,
  versions,
  showVersionManager,
  onShowVersionManager,
  diskSpacePercent,
  modelManagerProps,
}: OnnxRuntimePanelProps) {
  const isManagerOpen = versions.isSupported && showVersionManager;

  return (
    <div className="flex-1 flex flex-col gap-4 p-6 overflow-hidden">
      <div className="w-full flex flex-col gap-4">
        <VersionManagementPanel
          appDisplayName={appDisplayName}
          versions={versions}
          showManager={showVersionManager}
          onShowManager={onShowVersionManager}
          diskSpacePercent={diskSpacePercent}
        />
      </div>

      {!isManagerOpen && <RuntimeProfileSettingsSection provider="onnx_runtime" />}

      {!isManagerOpen && (
        <OnnxRuntimeModelLibrarySection
          excludedModels={modelManagerProps.excludedModels}
          modelGroups={modelManagerProps.modelGroups}
          servingEndpoint={modelManagerProps.servingEndpoint}
          servedModels={modelManagerProps.servedModels}
          starredModels={modelManagerProps.starredModels}
          onToggleLink={modelManagerProps.onToggleLink}
          onToggleStar={modelManagerProps.onToggleStar}
        />
      )}
    </div>
  );
}
