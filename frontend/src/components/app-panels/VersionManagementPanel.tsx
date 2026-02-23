import { useMemo, useState } from 'react';
import { ArrowLeft, RefreshCw } from 'lucide-react';
import { VersionSelector } from '../VersionSelector';
import { InstallDialog } from '../InstallDialog';
import type { AppVersionState } from '../../utils/appVersionState';
import { IconButton } from '../ui';

interface VersionManagementPanelProps {
  appDisplayName: string;
  backLabel?: string;
  versions: AppVersionState;
  showManager: boolean;
  onShowManager: (show: boolean) => void;
  activeShortcutState?: { menu: boolean; desktop: boolean };
  diskSpacePercent?: number;
}

export function VersionManagementPanel({
  appDisplayName,
  backLabel: _backLabel,
  versions,
  showManager,
  onShowManager,
  activeShortcutState,
  diskSpacePercent = 0,
}: VersionManagementPanelProps) {
  const [isRefreshing, setIsRefreshing] = useState(false);

  const latestVersion = versions.availableVersions[0]?.tagName ?? null;
  const hasNewVersion = useMemo(() => {
    if (!latestVersion) return false;
    return !versions.installedVersions.includes(latestVersion);
  }, [latestVersion, versions.installedVersions]);

  const handleRefresh = async () => {
    if (isRefreshing || versions.isLoading) return;
    setIsRefreshing(true);
    try {
      await versions.refreshAll(true);
    } finally {
      setIsRefreshing(false);
    }
  };

  const handleMakeDefault = async (tag: string | null) => {
    await versions.setDefaultVersion(tag);
    return true;
  };

  if (!versions.isSupported) {
    return null;
  }

  if (showManager) {
    return (
      <div className="w-full flex-1 flex flex-col gap-2 min-h-0">
        <div className="w-full flex items-center justify-between flex-shrink-0">
          <div className="flex items-center gap-2 text-sm text-[hsl(var(--text-secondary))]">
            <IconButton
              icon={<ArrowLeft />}
              tooltip="Back"
              onClick={() => onShowManager(false)}
              size="md"
            />
            <span>{versions.installedVersions.length} installed</span>
          </div>
          <IconButton
            icon={<RefreshCw className={isRefreshing ? 'animate-spin' : ''} />}
            tooltip="Refresh"
            onClick={handleRefresh}
            disabled={isRefreshing || versions.isLoading}
            size="md"
          />
        </div>
        <div className="w-full flex-1 min-h-0 overflow-hidden">
          <InstallDialog
            isOpen={showManager}
            onClose={() => onShowManager(false)}
            availableVersions={versions.availableVersions}
            installedVersions={versions.installedVersions}
            isLoading={versions.isLoading}
            onInstallVersion={versions.installVersion}
            onRemoveVersion={versions.removeVersion}
            onRefreshAll={versions.refreshAll}
            installingTag={versions.installingTag}
            installationProgress={versions.installationProgress}
            installNetworkStatus={versions.installNetworkStatus}
            onRefreshProgress={versions.fetchInstallationProgress}
            displayMode="page"
            appDisplayName={appDisplayName}
            appId={versions.appId ?? undefined}
            isRateLimited={versions.isRateLimited}
            rateLimitRetryAfter={versions.rateLimitRetryAfter}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="w-full">
      <VersionSelector
        installedVersions={versions.installedVersions}
        activeVersion={versions.activeVersion}
        isLoading={versions.isLoading}
        switchVersion={versions.switchVersion}
        openActiveInstall={versions.openActiveInstall}
        onOpenVersionManager={() => onShowManager(true)}
        installNetworkStatus={versions.installNetworkStatus}
        installationProgress={versions.installationProgress}
        defaultVersion={versions.defaultVersion}
        onMakeDefault={handleMakeDefault}
        installingVersion={versions.installingTag}
        activeShortcutState={activeShortcutState}
        diskSpacePercent={diskSpacePercent}
        hasNewVersion={hasNewVersion}
        latestVersion={latestVersion}
        supportsShortcuts={versions.supportsShortcuts}
      />
    </div>
  );
}
