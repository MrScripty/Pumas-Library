import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowLeft, RefreshCw } from 'lucide-react';
import { VersionSelector } from '../VersionSelector';
import { InstallDialog } from '../InstallDialog';
import { TorchRuntimeProbePanel } from '../TorchRuntimeProbePanel';
import type { createTorchTrialLifecycleStore } from '../TorchTrialLifecycleStore';
import type { AppVersionState } from '../../utils/appVersionState';
import { IconButton } from '../ui';

interface VersionManagementPanelProps {
  appDisplayName: string;
  backLabel?: string;
  versions: AppVersionState;
  showManager: boolean;
  onShowManager: (show: boolean) => void;
  diskSpacePercent?: number;
  trialStore?: ReturnType<typeof createTorchTrialLifecycleStore>;
}

export function VersionManagementPanel({
  appDisplayName,
  backLabel: _backLabel,
  versions,
  showManager,
  onShowManager,
  diskSpacePercent = 0,
  trialStore,
}: VersionManagementPanelProps) {
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [refreshOnOpenPending, setRefreshOnOpenPending] = useState(false);
  const [inspectedTorchTag, setInspectedTorchTag] = useState<string | null>(null);
  const refreshInFlight = useRef(false);
  const refreshOnOpenStarted = useRef(false);

  useEffect(() => {
    setInspectedTorchTag(null);
  }, [versions.activeVersion]);

  const latestVersion = versions.availableVersions[0]?.tagName ?? null;
  const hasNewVersion = useMemo(() => {
    if (!latestVersion) return false;
    return !versions.installedVersions.includes(latestVersion);
  }, [latestVersion, versions.installedVersions]);

  const refreshAvailableVersions = useCallback(async () => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    setIsRefreshing(true);
    try {
      await versions.refreshAll(true);
    } catch {
      // refreshAll records and reports its own errors.
    } finally {
      refreshInFlight.current = false;
      setIsRefreshing(false);
    }
  }, [versions.refreshAll]);

  useEffect(() => {
    if (!showManager) {
      refreshOnOpenStarted.current = false;
      if (refreshOnOpenPending) setRefreshOnOpenPending(false);
      return;
    }
    if (!refreshOnOpenPending || versions.isLoading || refreshOnOpenStarted.current) return;

    refreshOnOpenStarted.current = true;
    void (async () => {
      try {
        await refreshAvailableVersions();
      } finally {
        setRefreshOnOpenPending(false);
      }
    })();
  }, [refreshAvailableVersions, refreshOnOpenPending, showManager, versions.isLoading]);

  const handleRefresh = async () => {
    if (isRefreshing || versions.isLoading) return;
    await refreshAvailableVersions();
  };

  const handleOpenVersionManager = () => {
    setInspectedTorchTag(null);
    refreshOnOpenStarted.current = false;
    setRefreshOnOpenPending(true);
    onShowManager(true);
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
            disabled={isRefreshing || refreshOnOpenPending || versions.isLoading}
            size="md"
          />
        </div>
        <div className="w-full flex-1 min-h-0 overflow-hidden">
          <InstallDialog
            isOpen={showManager}
            onClose={() => onShowManager(false)}
            availableVersions={versions.availableVersions}
            installedVersions={versions.installedVersions}
            isLoading={versions.isLoading || isRefreshing || refreshOnOpenPending}
            onInstallVersion={versions.installVersion}
            onCancelInstallation={versions.cancelInstallation}
            onRemoveVersion={versions.removeVersion}
            onRefreshAll={versions.refreshAll}
            installingTag={versions.installingTag}
            installationProgress={versions.installationProgress}
            installNetworkStatus={versions.installNetworkStatus}
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
        appId={versions.appId}
        installedVersions={versions.installedVersions}
        activeVersion={versions.activeVersion}
        isLoading={versions.isLoading}
        switchVersion={versions.switchVersion}
        openActiveInstall={versions.openActiveInstall}
        onOpenVersionManager={handleOpenVersionManager}
        installNetworkStatus={versions.installNetworkStatus}
        installationProgress={versions.installationProgress}
        defaultVersion={versions.defaultVersion}
        onMakeDefault={handleMakeDefault}
        installingVersion={versions.installingTag}
        diskSpacePercent={diskSpacePercent}
        hasNewVersion={hasNewVersion}
        latestVersion={latestVersion}
      />
      {versions.appId === 'torch' && versions.installedVersions.length > 0 && (
        <>
          <p className="mt-2 text-xs text-[hsl(var(--text-secondary))]">
            Selecting an installed Torch version does not prove it can start. Startup is attempted when you serve a model
            through a Torch profile. Setting a version as default also selects it on future starts.
          </p>
          {versions.activeVersion && <>
            <button type="button" className="mt-2 rounded border px-3 py-2 text-xs" onClick={() => setInspectedTorchTag(versions.activeVersion)}>
              Inspect active Torch runtime
            </button>
            {inspectedTorchTag === versions.activeVersion && <TorchRuntimeProbePanel
              tag={versions.activeVersion}
              trialStore={trialStore}
            />}
          </>}
        </>
      )}
    </div>
  );
}
