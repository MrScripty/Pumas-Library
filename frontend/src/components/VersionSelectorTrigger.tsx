import React from 'react';
import { Loader2 } from 'lucide-react';
import {
  FirstVersionInstallButton,
  OpenActiveInstallButton,
  TriggerDefaultButton,
  VersionManagerButton,
} from './VersionSelectorTriggerControls';

interface VersionSelectorTriggerProps {
  hasVersionsToShow: boolean;
  hasInstalledVersions: boolean;
  installingVersion: string | null | undefined;
  isLoading: boolean;
  isSwitching: boolean;
  activeVersion: string | null;
  defaultVersion: string | null;
  displayVersion: string;
  showOpenedIndicator: boolean;
  isOpeningPath: boolean;
  folderIconColor: string;
  emphasizeInstall: boolean;
  hasNewVersion: boolean;
  latestVersion: string | null;
  installNetworkStatus: 'idle' | 'downloading' | 'stalled' | 'failed';
  hasInstallActivity: boolean;
  isInstallPending: boolean;
  isInstallFailed: boolean;
  ringDegrees: number;
  onToggleOpen: () => void;
  onToggleDefault: () => void;
  onOpenActiveInstall: (event: React.MouseEvent) => void;
  onOpenVersionManager: (event: React.MouseEvent) => void;
  canMakeDefault: boolean;
}

export function VersionSelectorTrigger({
  hasVersionsToShow,
  hasInstalledVersions,
  installingVersion,
  isLoading,
  isSwitching,
  activeVersion,
  defaultVersion,
  displayVersion,
  showOpenedIndicator,
  isOpeningPath,
  folderIconColor,
  emphasizeInstall,
  hasNewVersion,
  latestVersion,
  installNetworkStatus,
  hasInstallActivity,
  isInstallPending,
  isInstallFailed,
  ringDegrees,
  onToggleOpen,
  onToggleDefault,
  onOpenActiveInstall,
  onOpenVersionManager,
  canMakeDefault,
}: VersionSelectorTriggerProps) {
  return (
    <div
      className={`flex h-10 w-full items-center justify-center rounded border border-[hsl(var(--border-control))] bg-[hsl(var(--surface-interactive))] transition-colors ${
        !hasVersionsToShow || isLoading || isSwitching ? 'opacity-50' : ''
      }`}
    >
      {!hasInstalledVersions && !installingVersion ? (
        <FirstVersionInstallButton isLoading={isLoading} onOpenVersionManager={onOpenVersionManager} />
      ) : (
        <>
          <div className="flex flex-1 items-center gap-2 px-3">
            <span className="inline-flex w-4 items-center justify-center">
              {isSwitching ? (
                <Loader2 size={14} className="animate-spin text-[hsl(var(--text-tertiary))]" />
              ) : canMakeDefault ? (
                <TriggerDefaultButton
                  activeVersion={activeVersion}
                  defaultVersion={defaultVersion}
                  isLoading={isLoading}
                  onToggleDefault={onToggleDefault}
                />
              ) : (
                <div className="w-4" />
              )}
            </span>
            <button
              onClick={onToggleOpen}
              disabled={!hasVersionsToShow || isLoading || isSwitching}
              className="min-w-0 flex-1 text-left transition-opacity hover:opacity-80 disabled:cursor-not-allowed"
            >
              <span className="block truncate text-sm font-medium text-[hsl(var(--text-primary))]">
                {displayVersion}
              </span>
            </button>
          </div>

          <div className="flex items-center gap-2 px-3">
            <OpenActiveInstallButton
              activeVersion={activeVersion}
              folderIconColor={folderIconColor}
              isLoading={isLoading}
              isOpeningPath={isOpeningPath}
              onOpenActiveInstall={onOpenActiveInstall}
              showOpenedIndicator={showOpenedIndicator}
            />

            <VersionManagerButton
              emphasizeInstall={emphasizeInstall}
              hasInstallActivity={hasInstallActivity}
              hasNewVersion={hasNewVersion}
              installNetworkStatus={installNetworkStatus}
              isInstallFailed={isInstallFailed}
              isInstallPending={isInstallPending}
              isLoading={isLoading}
              latestVersion={latestVersion}
              onOpenVersionManager={onOpenVersionManager}
              ringDegrees={ringDegrees}
            />
          </div>
        </>
      )}
    </div>
  );
}
