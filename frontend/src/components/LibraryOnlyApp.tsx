import { Header } from './Header';
import { StatusNoticesBanner } from './StatusNoticesBanner';
import { ModelImportDialog } from './ModelImportDialog';
import { ModelImportDropZone } from './ModelImportDropZone';
import { ModelManager } from './ModelManager';
import { getLauncherLatestVersion } from './AppShellState';
import { useActiveModelDownload } from '../hooks/useActiveModelDownload';
import { useAppImportDialog } from '../hooks/useAppImportDialog';
import { useAppStartupChecks } from '../hooks/useAppStartupChecks';
import { useAppWindowActions } from '../hooks/useAppWindowActions';
import { useDiskSpace } from '../hooks/useDiskSpace';
import { useLauncherUpdates } from '../hooks/useLauncherUpdates';
import { useModelPreferences } from '../hooks/useModelPreferences';
import { useModels } from '../hooks/useModels';
import { useStatus } from '../hooks/useStatus';

/** The complete application when inference plugin support is compiled out. */
export default function LibraryOnlyApp() {
  const {
    status,
    systemResources,
    networkAvailable,
    modelLibraryLoaded,
    refetch: refetchStatus,
  } = useStatus();
  const { fetchDiskSpace } = useDiskSpace();
  const { modelGroups, libraryLoadStatus, scanModels, fetchModels } = useModels();
  const { activeDownload, activeDownloadCount } = useActiveModelDownload();
  const {
    checkLauncherUpdates,
    checkLauncherVersion,
    isCheckingLauncherUpdates,
    launcherUpdateAvailable,
    launcherUpdateState,
    openLauncherUpdate,
  } = useLauncherUpdates();
  const { excludedModels, starredModels, toggleLink, toggleStar } = useModelPreferences({
    selectedAppId: null,
  });
  const { closeWindow, minimizeWindow, openModelsRoot, chooseLibraryRoot } = useAppWindowActions();
  const {
    handleImportComplete,
    handleImportDialogClose,
    handlePathsDropped,
    importPaths,
    showImportDialog,
  } = useAppImportDialog({ onImportComplete: fetchModels });

  useAppStartupChecks({
    activeVersion: null,
    checkLauncherVersion,
    fetchDiskSpace,
    refetchStatus,
  });

  return (
    <div className="relative flex h-screen w-full flex-col overflow-hidden font-mono gradient-bg-blobs">
      <ModelImportDropZone onPathsDropped={handlePathsDropped} enabled={true} />

      {showImportDialog && importPaths.length > 0 && (
        <ModelImportDialog
          importPaths={importPaths}
          onClose={handleImportDialogClose}
          onImportComplete={handleImportComplete}
        />
      )}

      <Header
        systemResources={systemResources}
        launcherUpdateAvailable={launcherUpdateAvailable}
        launcherLatestVersion={getLauncherLatestVersion(launcherUpdateState)}
        isCheckingLauncherUpdates={isCheckingLauncherUpdates}
        onCheckLauncherUpdates={() => { void checkLauncherUpdates(); }}
        onDownloadLauncherUpdate={() => { void openLauncherUpdate(); }}
        onMinimize={minimizeWindow}
        onClose={closeWindow}
        networkAvailable={networkAvailable}
        modelLibraryLoaded={modelLibraryLoaded}
        activeModelDownload={activeDownload}
        activeModelDownloadCount={activeDownloadCount}
      />
      <StatusNoticesBanner notices={status?.notices ?? []} />

      <main className="relative z-10 flex flex-1 flex-col overflow-hidden p-6">
        <ModelManager
          modelGroups={modelGroups}
          libraryLoadStatus={libraryLoadStatus}
          starredModels={starredModels}
          excludedModels={excludedModels}
          onToggleStar={toggleStar}
          onToggleLink={toggleLink}
          selectedAppId={null}
          onAddModels={scanModels}
          onOpenModelsRoot={openModelsRoot}
          onModelsImported={fetchModels}
          activeVersion={null}
          onChooseExistingLibrary={chooseLibraryRoot}
        />
      </main>
    </div>
  );
}
