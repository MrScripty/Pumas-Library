import { useMemo, useState } from 'react';
import { motion } from 'framer-motion';
import { ArrowLeft, RefreshCw } from 'lucide-react';
import { VersionSelector } from '../VersionSelector';
import { InstallDialog } from '../InstallDialog';
import type { AppVersionState } from '../../utils/appVersionState';

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
  backLabel,
  versions,
  showManager,
  onShowManager,
  activeShortcutState,
  diskSpacePercent = 0,
}: VersionManagementPanelProps) {
  const [isRefreshing, setIsRefreshing] = useState(false);

  const latestVersion = versions.availableVersions[0]?.tag_name ?? null;
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
      <div className="w-full flex-1 flex flex-col gap-4 min-h-0">
        <div className="w-full flex items-center justify-between flex-shrink-0">
          <button
            onClick={() => onShowManager(false)}
            className="flex items-center gap-2 px-3 py-2 rounded border border-[hsl(var(--border-control))] bg-[hsl(var(--surface-interactive))] hover:bg-[hsl(var(--surface-interactive-hover))] text-[hsl(var(--text-primary))] text-sm transition-colors"
          >
            <ArrowLeft size={14} />
            <span>{backLabel ?? `Back to ${appDisplayName}`}</span>
          </button>
          <div className="flex items-center gap-3 text-xs text-[hsl(var(--text-secondary))]">
            <span>{versions.installedVersions.length} installed</span>
            <motion.button
              onClick={handleRefresh}
              disabled={isRefreshing || versions.isLoading}
              className="p-2 rounded hover:bg-[hsl(var(--surface-interactive-hover))] transition-colors disabled:opacity-50"
              whileHover={{ scale: isRefreshing || versions.isLoading ? 1 : 1.05 }}
              whileTap={{ scale: isRefreshing || versions.isLoading ? 1 : 0.96 }}
              title="Refresh versions"
            >
              <RefreshCw
                size={14}
                className={
                  isRefreshing
                    ? 'animate-spin text-[hsl(var(--text-tertiary))]'
                    : 'text-[hsl(var(--text-secondary))]'
                }
              />
            </motion.button>
          </div>
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
