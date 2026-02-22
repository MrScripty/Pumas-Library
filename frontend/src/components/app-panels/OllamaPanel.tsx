import { AppConnectionInfo } from '../AppConnectionInfo';
import { ModelManager, type ModelManagerProps } from '../ModelManager';
import { VersionManagementPanel } from './VersionManagementPanel';
import { OllamaModelSection } from './sections/OllamaModelSection';
import type { AppVersionState } from '../../utils/appVersionState';
import type { ModelCategory } from '../../types/apps';

export interface OllamaPanelProps {
  appDisplayName: string;
  connectionUrl?: string;
  versions: AppVersionState;
  showVersionManager: boolean;
  onShowVersionManager: (show: boolean) => void;
  activeShortcutState?: { menu: boolean; desktop: boolean };
  diskSpacePercent: number;
  modelManagerProps: ModelManagerProps;
  isOllamaRunning: boolean;
  modelGroups: ModelCategory[];
}

export function OllamaPanel({
  appDisplayName,
  connectionUrl,
  versions,
  showVersionManager,
  onShowVersionManager,
  activeShortcutState,
  diskSpacePercent,
  modelManagerProps,
  isOllamaRunning,
  modelGroups,
}: OllamaPanelProps) {
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

      {!isManagerOpen && connectionUrl && (
        <OllamaModelSection
          connectionUrl={connectionUrl}
          isRunning={isOllamaRunning}
          modelGroups={modelGroups}
        />
      )}

      {!isManagerOpen && <ModelManager {...modelManagerProps} />}
    </div>
  );
}
