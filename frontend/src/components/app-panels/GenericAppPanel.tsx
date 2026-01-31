/**
 * Generic App Panel Component
 *
 * Config-driven app panel that renders sections based on plugin configuration.
 * No app-specific code - purely data-driven from plugin JSON files.
 */

import { useMemo } from 'react';
import type { PluginConfig, PanelSection } from '../../types/plugins';
import type { AppVersionState } from '../../utils/appVersionState';
import type { ModelManagerProps } from '../ModelManager';
import {
  VersionManagerSection,
  ConnectionInfoSection,
  DependencyStatusSection,
  StatsSection,
  ModelSelectorSection,
  ModelLibrarySection,
} from './sections';

export interface GenericAppPanelProps {
  /** Plugin configuration */
  plugin: PluginConfig;
  /** Version management state */
  versions: AppVersionState;
  /** Whether version manager is open */
  showVersionManager: boolean;
  /** Callback to toggle version manager */
  onShowVersionManager: (show: boolean) => void;
  /** Whether the app is currently running */
  isRunning: boolean;
  /** Shortcut state for menu/desktop shortcuts */
  activeShortcutState?: { menu: boolean; desktop: boolean };
  /** Disk space usage percentage */
  diskSpacePercent?: number;
  /** Dependency status for Python apps */
  dependencyStatus?: {
    isChecking: boolean;
    isInstalled: boolean | null;
    isInstalling: boolean;
    onInstall: () => void;
  };
  /** Model manager props for model library section */
  modelManagerProps?: ModelManagerProps;
  /** Local models for model selector section */
  localModels?: Array<{ id?: string; name: string; path?: string; category: string; size?: number }>;
}

/**
 * Renders a single panel section based on its type.
 */
function PanelSectionRenderer({
  section,
  props,
}: {
  section: PanelSection;
  props: GenericAppPanelProps;
}) {
  const {
    plugin,
    versions,
    showVersionManager,
    onShowVersionManager,
    isRunning,
    activeShortcutState,
    diskSpacePercent,
    dependencyStatus,
    modelManagerProps,
    localModels,
  } = props;

  const sectionConfig = section.config || {};

  switch (section.type) {
    case 'version_manager':
      return (
        <VersionManagerSection
          appDisplayName={plugin.displayName}
          versions={versions}
          showManager={showVersionManager}
          onShowManager={onShowVersionManager}
          activeShortcutState={activeShortcutState}
          diskSpacePercent={diskSpacePercent}
          backLabel={sectionConfig['backLabel'] as string | undefined}
        />
      );

    case 'connection_info':
      return (
        <ConnectionInfoSection
          connection={plugin.connection}
          isRunning={isRunning}
          label={sectionConfig['label'] as string | undefined}
        />
      );

    case 'dependency_status':
      if (!dependencyStatus) return null;
      return (
        <DependencyStatusSection
          isChecking={dependencyStatus.isChecking}
          isInstalled={dependencyStatus.isInstalled}
          isInstalling={dependencyStatus.isInstalling}
          isAppRunning={isRunning}
          onInstall={dependencyStatus.onInstall}
        />
      );

    case 'stats':
      return (
        <StatsSection
          appId={plugin.id}
          config={{
            showMemory: sectionConfig['showMemory'] as boolean | undefined,
            showLoadedModels: sectionConfig['showLoadedModels'] as boolean | undefined,
            pollingIntervalMs: sectionConfig['pollingIntervalMs'] as number | undefined,
          }}
          isRunning={isRunning}
        />
      );

    case 'model_selector':
      return (
        <ModelSelectorSection
          appId={plugin.id}
          compatibility={plugin.modelCompatibility}
          config={{
            filter: sectionConfig['filter'] as string | undefined,
          }}
          isRunning={isRunning}
          localModels={localModels}
        />
      );

    case 'model_library':
      if (!modelManagerProps) return null;
      return (
        <ModelLibrarySection
          {...modelManagerProps}
          enabled={true}
        />
      );

    default:
      // Unknown section type - skip silently
      return null;
  }
}

/**
 * Generic App Panel
 *
 * Renders sections based on plugin's panelLayout configuration.
 * Each section type maps to a specific component.
 */
export function GenericAppPanel(props: GenericAppPanelProps) {
  const { plugin, versions, showVersionManager } = props;
  const isVersionManagerOpen = versions.isSupported && showVersionManager;

  // Sections to render when version manager is NOT open
  const mainSections = useMemo(() => {
    return plugin.panelLayout.filter((section) => {
      // Version manager is always rendered, but shown/hidden via its own logic
      if (section.type === 'version_manager') return true;
      // Other sections are hidden when version manager is open
      return !isVersionManagerOpen;
    });
  }, [plugin.panelLayout, isVersionManagerOpen]);

  return (
    <div className="flex-1 flex flex-col gap-4 p-6 overflow-hidden">
      {mainSections.map((section, index) => (
        <PanelSectionRenderer
          key={`${section.type}-${index}`}
          section={section}
          props={props}
        />
      ))}
    </div>
  );
}
