import { motion } from 'framer-motion';
import { DependencySection } from '../DependencySection';
import { StatusDisplay } from '../StatusDisplay';
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
  const isManagerOpen = versions.isSupported && showVersionManager;
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
              </motion.div>
            </>
          )}
        </>
      )}
    </div>
  );
}
