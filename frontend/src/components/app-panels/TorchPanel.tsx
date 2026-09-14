import { AppConnectionInfo } from '../AppConnectionInfo';
import { ModelManager, type ModelManagerProps } from '../ModelManager';
import { VersionManagementPanel } from './VersionManagementPanel';
import { RuntimeProfileSettingsSection } from './sections/RuntimeProfileSettingsSection';
import type { AppVersionState } from '../../utils/appVersionState';
import type { ModelCategory } from '../../types/apps';

export interface TorchPanelProps {
  appDisplayName: string;
  connectionUrl?: string;
  versions: AppVersionState;
  showVersionManager: boolean;
  onShowVersionManager: (show: boolean) => void;
  diskSpacePercent: number;
  modelManagerProps: ModelManagerProps;
  isTorchRunning: boolean;
  modelGroups: ModelCategory[];
}

export function TorchPanel({
  appDisplayName,
  versions,
  showVersionManager,
  onShowVersionManager,
  diskSpacePercent,
  modelManagerProps,
}: TorchPanelProps) {
  const endpoint = modelManagerProps.servingEndpoint;
  const gatewayUrl = endpoint?.endpoint_mode === 'pumas_gateway' ? endpoint.endpoint_url : null;
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
        {!isManagerOpen && gatewayUrl && (
          <AppConnectionInfo url={gatewayUrl} />
        )}
      </div>

      {!isManagerOpen && (
        <>
          <p className="text-sm text-[var(--text-secondary)]">
            Create a Torch runtime profile, then use Serve on the image model.
            {' '}{gatewayUrl
              ? 'Copy the Pumas gateway URL above into Tuldok to generate images.'
              : 'The Pumas gateway URL will appear when a model is served.'}
            {' '}Stop Torch profiles before changing runtime versions.
          </p>
          <RuntimeProfileSettingsSection provider="torch" />
        </>
      )}

      {!isManagerOpen && <ModelManager {...modelManagerProps} />}
    </div>
  );
}
