import { AnimatePresence, motion } from 'framer-motion';
import { useHover } from '@react-aria/interactions';
import { Check, Link2 } from 'lucide-react';
import { VersionSelectorDefaultButton } from './VersionSelectorDefaultButton';

interface VersionDropdownItemProps {
  version: string;
  isActive: boolean;
  isInstalling: boolean;
  isSwitching: boolean;
  isLoading: boolean;
  isDefault: boolean;
  isEnabled: boolean;
  supportsShortcuts: boolean;
  onMakeDefault?: (tag: string | null) => Promise<boolean>;
  onSwitchVersion: (tag: string) => void;
  onToggleShortcuts?: (version: string, enabled: boolean) => Promise<void>;
}

function VersionDropdownItem({
  version,
  isActive,
  isInstalling,
  isSwitching,
  isLoading,
  isDefault,
  isEnabled,
  supportsShortcuts,
  onMakeDefault,
  onSwitchVersion,
  onToggleShortcuts,
}: VersionDropdownItemProps) {
  const { hoverProps: rowHoverProps, isHovered: isRowHovered } = useHover({});

  return (
    <div
      {...rowHoverProps}
      className={`relative flex w-full items-center justify-between px-3 py-2 text-left text-sm transition-colors ${
        isActive
          ? 'bg-[hsl(var(--surface-interactive-hover))] text-[hsl(var(--accent-success))]'
          : isInstalling
            ? 'bg-[hsl(var(--surface-interactive))] text-[hsl(var(--text-tertiary))]'
            : 'text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))] hover:text-[hsl(var(--text-primary))]'
      } ${isSwitching || isInstalling ? 'cursor-not-allowed opacity-50' : ''}`}
    >
      <div className="flex min-w-0 items-center gap-2">
        <div className="flex w-4 flex-shrink-0 items-center justify-center">
          {onMakeDefault ? (
            <VersionSelectorDefaultButton
              isDefault={isDefault}
              isLoading={isLoading}
              isRowHovered={isRowHovered}
              isSwitching={isSwitching}
              onMakeDefault={onMakeDefault}
              version={version}
            />
          ) : (
            <div className="w-4" />
          )}
        </div>
        <button
          type="button"
          onClick={() => onSwitchVersion(version)}
          disabled={isInstalling}
          className="flex min-w-0 flex-1 items-center gap-2 bg-transparent p-0 text-left disabled:cursor-not-allowed"
          aria-label={`Switch to ${version}`}
        >
          <span className="truncate font-medium">{version}</span>
          {isInstalling && (
            <span className="rounded-full border border-amber-400/60 bg-amber-500/20 px-1.5 py-[2px] text-[10px] text-amber-200">
              Installing
            </span>
          )}
        </button>
      </div>
      <div className="flex items-center gap-2 pr-12">
        {supportsShortcuts && !isInstalling && (isRowHovered || isEnabled) && onToggleShortcuts && (
          <button
            type="button"
            onClick={async (event) => {
              event.stopPropagation();
              const next = !isEnabled;
              await onToggleShortcuts(version, next);
            }}
            disabled={isSwitching || isLoading}
            className="absolute right-8 top-1/2 flex -translate-y-1/2 items-center justify-center transition-colors"
            aria-label={isEnabled ? `Disable shortcuts for ${version}` : `Enable shortcuts for ${version}`}
            title={isEnabled ? 'Shortcuts enabled (click to disable)' : 'Shortcuts disabled (click to enable)'}
          >
            <Link2
              size={14}
              className={isEnabled ? 'text-[hsl(var(--accent-link))]' : 'text-[hsl(var(--text-tertiary))]'}
              style={{ opacity: 1 }}
              aria-hidden
            />
          </button>
        )}
        {isActive && (
          <span className="absolute right-2 top-1/2 -translate-y-1/2">
            <Check size={14} className="text-[hsl(var(--accent-success))]" />
          </span>
        )}
      </div>
    </div>
  );
}

interface VersionSelectorDropdownProps {
  isOpen: boolean;
  hasVersionsToShow: boolean;
  combinedVersions: string[];
  activeVersion: string | null;
  shortcutState: Record<string, { menu: boolean; desktop: boolean }>;
  supportsShortcuts: boolean;
  installingVersion: string | null | undefined;
  installedVersions: string[];
  isInstallComplete: boolean;
  defaultVersion: string | null;
  isSwitching: boolean;
  isLoading: boolean;
  onMakeDefault?: (tag: string | null) => Promise<boolean>;
  onSwitchVersion: (tag: string) => void;
  onToggleShortcuts: (version: string, enabled: boolean) => Promise<void>;
}

export function VersionSelectorDropdown({
  isOpen,
  hasVersionsToShow,
  combinedVersions,
  activeVersion,
  shortcutState,
  supportsShortcuts,
  installingVersion,
  installedVersions,
  isInstallComplete,
  defaultVersion,
  isSwitching,
  isLoading,
  onMakeDefault,
  onSwitchVersion,
  onToggleShortcuts,
}: VersionSelectorDropdownProps) {
  return (
    <AnimatePresence>
      {isOpen && hasVersionsToShow && (
        <motion.div
          initial={{ opacity: 0, y: -10 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -10 }}
          transition={{ duration: 0.2 }}
          className="absolute left-0 right-0 top-full z-50 mt-1 overflow-hidden rounded bg-[hsl(var(--surface-overlay))]/80 backdrop-blur-sm"
        >
          <div className="max-h-64 overflow-y-auto">
            {combinedVersions.map((version) => {
              const isActive = version === activeVersion;
              const toggles = shortcutState[version] || { menu: false, desktop: false };
              const isEnabled = supportsShortcuts && toggles.menu && toggles.desktop;
              const isInstalling =
                installingVersion === version &&
                !installedVersions.includes(version) &&
                !isInstallComplete;
              const isDefault = defaultVersion === version;
              return (
                <VersionDropdownItem
                  key={version}
                  version={version}
                  isActive={isActive}
                  isInstalling={isInstalling}
                  isSwitching={isSwitching}
                  isLoading={isLoading}
                  isDefault={isDefault}
                  isEnabled={isEnabled}
                  supportsShortcuts={supportsShortcuts}
                  onMakeDefault={onMakeDefault}
                  onSwitchVersion={onSwitchVersion}
                  onToggleShortcuts={onToggleShortcuts}
                />
              );
            })}
          </div>

          {installedVersions.length === 0 && (
            <div className="px-3 py-4 text-center text-sm text-[hsl(var(--text-tertiary))]">
              No versions installed
            </div>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
