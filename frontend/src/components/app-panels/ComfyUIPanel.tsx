import { useState } from 'react';
import { motion } from 'framer-motion';
import { FolderSymlink } from 'lucide-react';
import { DependencySection } from '../DependencySection';
import { StatusDisplay } from '../StatusDisplay';
import { MappingPreviewDialog } from '../MappingPreviewDialog';
import { LinkHealthStatus } from '../LinkHealthStatus';
import { VersionManagementPanel } from './VersionManagementPanel';
import type { AppVersionState } from '../../utils/appVersionState';

export interface ComfyUIPanelProps {
  appDisplayName: string;
  versions: AppVersionState;
  showVersionManager: boolean;
  onShowVersionManager: (show: boolean) => void;
  activeShortcutState?: { menu: boolean; desktop: boolean };
  diskSpacePercent: number;
  isCheckingDeps: boolean;
  depsInstalled: boolean | null;
  isInstallingDeps: boolean;
  comfyUIRunning: boolean;
  onInstallDeps: () => void;
  displayStatus: string;
  isSetupComplete: boolean;
}

export function ComfyUIPanel({
  appDisplayName,
  versions,
  showVersionManager,
  onShowVersionManager,
  activeShortcutState,
  diskSpacePercent,
  isCheckingDeps,
  depsInstalled,
  isInstallingDeps,
  comfyUIRunning,
  onInstallDeps,
  displayStatus,
  isSetupComplete,
}: ComfyUIPanelProps) {
  const [showMappingDialog, setShowMappingDialog] = useState(false);
  const isManagerOpen = versions.isSupported && showVersionManager;
  const activeVersion = versions.activeVersion;
  const versionPanel = (
    <VersionManagementPanel
      appDisplayName={appDisplayName}
      backLabel="Back to setup"
      versions={versions}
      showManager={showVersionManager}
      onShowManager={onShowVersionManager}
      activeShortcutState={activeShortcutState}
      diskSpacePercent={diskSpacePercent}
    />
  );

  return (
    <div className="flex-1 p-6 flex flex-col items-center overflow-auto">
      {isCheckingDeps || depsInstalled === null ? (
        <div className="w-full flex items-center justify-center gap-2 text-[hsl(var(--text-secondary))]">
          <span className="text-sm">Checking Dependencies...</span>
        </div>
      ) : (
        <>
          {isManagerOpen ? (
            versionPanel
          ) : (
            <div className="w-full mb-4">
              {versionPanel}
            </div>
          )}

          {!isManagerOpen && (
            <>
              <DependencySection
                depsInstalled={depsInstalled}
                isInstalling={isInstallingDeps}
                comfyUIRunning={comfyUIRunning}
                onInstall={onInstallDeps}
              />

              <motion.div
                className="w-full flex flex-col items-center gap-6"
                animate={{
                  opacity: depsInstalled ? 1 : 0.3,
                  filter: depsInstalled ? 'blur(0px)' : 'blur(1px)',
                  pointerEvents: depsInstalled ? 'auto' : 'none',
                }}
                transition={{ duration: 0.4 }}
              >
                {displayStatus && (
                  <StatusDisplay
                    message={displayStatus}
                    isRunning={comfyUIRunning}
                    isSetupComplete={isSetupComplete}
                  />
                )}

                {activeVersion && (
                  <div className="w-full max-w-md space-y-3">
                    <button
                      onClick={() => setShowMappingDialog(true)}
                      className="w-full flex items-center justify-center gap-2 px-4 py-2.5 text-sm font-medium bg-[hsl(var(--accent-primary))] hover:bg-[hsl(var(--accent-primary-hover))] text-white rounded-lg transition-colors"
                    >
                      <FolderSymlink className="w-4 h-4" />
                      Sync Library Models
                    </button>

                    <LinkHealthStatus activeVersion={activeVersion} />
                  </div>
                )}
              </motion.div>
            </>
          )}
        </>
      )}

      {activeVersion && (
        <MappingPreviewDialog
          isOpen={showMappingDialog}
          versionTag={activeVersion}
          onClose={() => setShowMappingDialog(false)}
        />
      )}
    </div>
  );
}
