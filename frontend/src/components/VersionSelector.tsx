import React, { useCallback, useEffect, useRef, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Check, Loader2, Download, FolderOpen, CheckCircle2, Link2, Anchor, CircleX } from 'lucide-react';

interface VersionSelectorProps {
  installedVersions: string[];
  activeVersion: string | null;
  isLoading: boolean;
  switchVersion: (tag: string) => Promise<boolean>;
  openActiveInstall: () => Promise<boolean>;
  onOpenVersionManager: () => void;
  activeShortcutState?: { menu: boolean; desktop: boolean };
  installingVersion?: string | null;
  installNetworkStatus?: 'idle' | 'downloading' | 'stalled' | 'failed';
  defaultVersion?: string | null;
  onMakeDefault?: (tag: string | null) => Promise<boolean>;
  diskSpacePercent?: number;
}

export function VersionSelector({
  installedVersions,
  activeVersion,
  isLoading,
  switchVersion,
  openActiveInstall,
  onOpenVersionManager,
  activeShortcutState,
  installingVersion,
  installNetworkStatus = 'idle',
  defaultVersion,
  onMakeDefault,
  diskSpacePercent = 0,
}: VersionSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isSwitching, setIsSwitching] = useState(false);
  const [isOpeningPath, setIsOpeningPath] = useState(false);
  const [showOpenedIndicator, setShowOpenedIndicator] = useState(false);
  const openedIndicatorTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [hoveredVersion, setHoveredVersion] = useState<string | null>(null);
  const [hoveredAnchor, setHoveredAnchor] = useState<string | null>(null);
  const [hoveredSelectorAnchor, setHoveredSelectorAnchor] = useState(false);
  const [shortcutState, setShortcutState] = useState<Record<string, { menu: boolean; desktop: boolean }>>({});
  const containerRef = useRef<HTMLDivElement | null>(null);
  const hoverDefaultWasActive = useRef<boolean>(false);

  const refreshShortcutStates = useCallback(async () => {
    if (!window.pywebview?.api?.get_all_shortcut_states) {
      return;
    }

    try {
      const result = await window.pywebview.api.get_all_shortcut_states();
      const states = result?.states?.states;
      if (result.success && states) {
        const mapped: Record<string, { menu: boolean; desktop: boolean }> = {};
        Object.entries(states).forEach(([tag, state]) => {
          mapped[tag] = {
            menu: Boolean((state as any).menu),
            desktop: Boolean((state as any).desktop),
          };
        });
        setShortcutState(mapped);
      }
    } catch (err) {
      console.error('Failed to fetch shortcut states', err);
    }
  }, []);

  const handleVersionSwitch = async (tag: string) => {
    if (tag === activeVersion) {
      setIsOpen(false);
      return;
    }

    setIsSwitching(true);
    try {
      await switchVersion(tag);
      setIsOpen(false);
    } catch (e) {
      console.error('Failed to switch version:', e);
    } finally {
      setIsSwitching(false);
    }
  };

  const handleOpenActiveInstall = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!activeVersion) {
      return;
    }

    setIsOpeningPath(true);
    setShowOpenedIndicator(false);
    try {
      await openActiveInstall();
      setShowOpenedIndicator(true);
      if (openedIndicatorTimeout.current) {
        clearTimeout(openedIndicatorTimeout.current);
      }
      openedIndicatorTimeout.current = setTimeout(() => setShowOpenedIndicator(false), 2000);
    } catch (err) {
      console.error('Failed to open active installation path:', err);
    } finally {
      setIsOpeningPath(false);
    }
  };

  const combinedVersions = React.useMemo(() => {
    const unique = new Set(installedVersions);
    const merged = [...installedVersions];
    if (installingVersion && !unique.has(installingVersion)) {
      merged.push(installingVersion);
    }
    // Sort by version number - most recent first (reverse sort)
    return merged.sort((a, b) => b.localeCompare(a, undefined, { numeric: true, sensitivity: 'base' }));
  }, [installedVersions, installingVersion]);

  const displayVersion = activeVersion || 'No version selected';
  const hasInstalledVersions = installedVersions.length > 0;
  const hasVersionsToShow = combinedVersions.length > 0;
  const emphasizeInstall = !hasInstalledVersions && !isLoading;

  // Determine folder icon color based on disk space
  const getFolderIconColor = () => {
    if (diskSpacePercent >= 95) return 'text-red-500';
    if (diskSpacePercent >= 85) return 'text-orange-500';
    return 'text-gray-400';
  };

  const folderIconColor = getFolderIconColor();

  useEffect(() => {
    return () => {
      if (openedIndicatorTimeout.current) {
        clearTimeout(openedIndicatorTimeout.current);
      }
    };
  }, []);

  // Refresh shortcut states when installed versions change
  useEffect(() => {
    if (!installedVersions.length) {
      setShortcutState({});
      return;
    }
    refreshShortcutStates();
  }, [installedVersions, refreshShortcutStates]);

  // Keep active version shortcut in sync with main toggle state
  useEffect(() => {
    if (activeVersion && activeShortcutState) {
      setShortcutState((prev) => ({
        ...prev,
        [activeVersion]: {
          menu: activeShortcutState.menu,
          desktop: activeShortcutState.desktop,
        },
      }));
    }
  }, [activeVersion, activeShortcutState?.menu, activeShortcutState?.desktop]);

  // Close dropdown on outside click
  useEffect(() => {
    const handleOutsideClick = (event: MouseEvent | TouchEvent) => {
      if (!isOpen) return;
      const target = event.target as Node | null;
      if (containerRef.current && target && !containerRef.current.contains(target)) {
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleOutsideClick);
    document.addEventListener('touchstart', handleOutsideClick);
    return () => {
      document.removeEventListener('mousedown', handleOutsideClick);
      document.removeEventListener('touchstart', handleOutsideClick);
    };
  }, [isOpen]);

  return (
    <div className="relative w-full" ref={containerRef}>
      {/* Version Selector Container - Changed from button to div to allow nested buttons */}
      <div
        className={`w-full h-10 bg-[#2a2a2a] border border-[#444] rounded flex items-center justify-center transition-colors ${
          !hasVersionsToShow || isLoading || isSwitching ? 'opacity-50' : ''
        }`}
      >
        {/* No versions installed - show centered download button */}
        {!hasInstalledVersions && !installingVersion ? (
          <motion.button
            onClick={(e) => {
              e.stopPropagation();
              onOpenVersionManager();
            }}
            disabled={isLoading}
            className="p-2 rounded hover:bg-[#444] transition-colors disabled:opacity-50"
            whileHover={{ scale: 1.2 }}
            whileTap={{ scale: 0.9 }}
            title="Install your first version"
          >
            <Download
              size={20}
              className="text-[#55ff55] animate-pulse"
              style={{ filter: 'drop-shadow(0 0 8px #55ff55)' }}
            />
          </motion.button>
        ) : (
          <>
            {/* Left side - clickable area for version selector */}
            <button
              onClick={() => {
                console.log('Version selector clicked, hasVersionsToShow:', hasVersionsToShow, 'isOpen:', isOpen);
                if (hasVersionsToShow) {
                  setIsOpen(!isOpen);
                  console.log('Set isOpen to:', !isOpen);
                }
              }}
              disabled={!hasVersionsToShow || isLoading || isSwitching}
              className="flex items-center gap-2 flex-1 hover:opacity-80 transition-opacity disabled:cursor-not-allowed px-3"
            >
              <span className="inline-flex items-center justify-center w-4">
                {isSwitching ? (
                  <Loader2 size={14} className="text-gray-400 animate-spin" />
                ) : (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      if (!onMakeDefault || !activeVersion) return;
                      const isDefault = defaultVersion === activeVersion;
                      const target = isDefault ? null : activeVersion;
                      onMakeDefault(target).catch((err) => console.error('Failed to toggle default', err));
                    }}
                    onMouseEnter={() => setHoveredSelectorAnchor(true)}
                    onMouseLeave={() => setHoveredSelectorAnchor(false)}
                    className="flex items-center justify-center w-4 h-4"
                    title={
                      defaultVersion === activeVersion
                        ? 'Click to unset as default'
                        : 'Click to set as default'
                    }
                    disabled={!onMakeDefault || isLoading}
                  >
                    {defaultVersion === activeVersion ? (
                      hoveredSelectorAnchor ? (
                        <CircleX size={14} className="text-gray-500" />
                      ) : (
                        <Anchor
                          size={14}
                          className="text-[#55ff55]"
                        />
                      )
                    ) : (
                      <div className="w-2 h-2 rounded-full bg-[#55ff55]" />
                    )}
                  </button>
                )}
              </span>
              <span className="text-sm text-white font-medium">
                {displayVersion}
              </span>
            </button>

            {/* Right side - action buttons */}
            <div className="flex items-center gap-2 px-3">
              {/* Open in File Manager */}
              <motion.button
                onClick={handleOpenActiveInstall}
                disabled={!activeVersion || isOpeningPath || isLoading}
                className="p-1 rounded hover:bg-[#444] transition-colors disabled:opacity-50"
                whileHover={{ scale: activeVersion ? 1.1 : 1 }}
                whileTap={{ scale: activeVersion ? 0.9 : 1 }}
                title={activeVersion ? 'Open active version in file manager' : 'No active version to open'}
              >
                {showOpenedIndicator ? (
                  <CheckCircle2 size={14} className="text-[#55ff55]" />
                ) : isOpeningPath ? (
                  <Loader2 size={14} className="text-gray-400 animate-spin" />
                ) : (
                  <FolderOpen size={14} className={folderIconColor} />
                )}
              </motion.button>

              {/* Download Button */}
              <motion.button
                onClick={(e) => {
                  console.log('Download button clicked!');
                  e.stopPropagation();
                  onOpenVersionManager();
                  console.log('Switching to version manager view');
                }}
                disabled={isLoading}
                className={`p-1 rounded hover:bg-[#444] transition-colors disabled:opacity-50 ${
                  emphasizeInstall ? 'animate-pulse ring-1 ring-[#55ff55]/60' : ''
                }`}
                whileHover={{ scale: 1.1 }}
                whileTap={{ scale: 0.9 }}
                title="Install new version"
              >
                <Download
                  size={14}
                  className={`text-gray-400 ${
                    installNetworkStatus === 'downloading'
                      ? 'animate-pulse text-[#7dff7d]'
                      : installNetworkStatus === 'stalled'
                        ? 'animate-pulse text-[#ffc266]'
                        : installNetworkStatus === 'failed'
                          ? 'animate-pulse text-[#ff6b6b]'
                          : ''
                  }`}
                  style={
                    installNetworkStatus === 'downloading'
                      ? { filter: 'drop-shadow(0 0 6px #7dff7d)' }
                      : installNetworkStatus === 'stalled'
                        ? { filter: 'drop-shadow(0 0 6px #ffc266)' }
                        : installNetworkStatus === 'failed'
                          ? { filter: 'drop-shadow(0 0 6px #ff6b6b)' }
                          : undefined
                  }
                />
              </motion.button>

              {/* Refresh Button moved to manager; dropdown arrow removed for cleaner look */}
            </div>
          </>
        )}
      </div>

      {/* Dropdown Menu */}
      <AnimatePresence>
        {isOpen && hasVersionsToShow && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            transition={{ duration: 0.2 }}
            className="absolute top-full left-0 right-0 mt-1 bg-[#2a2a2a] border border-[#444] rounded shadow-lg overflow-hidden z-50"
          >
            <div className="max-h-64 overflow-y-auto">
              {combinedVersions.map((version) => {
                const isActive = version === activeVersion;
                const toggles = shortcutState[version] || { menu: false, desktop: false };
                const isEnabled = toggles.menu && toggles.desktop;
                const isPartial = toggles.menu !== toggles.desktop;
                const showHover = hoveredVersion === version;
                const isInstalling = installingVersion === version && !installedVersions.includes(version);
                const isDefault = defaultVersion === version;
                const showAnchorHover = hoveredAnchor === version && !!onMakeDefault;
                return (
                  <div
                    key={version}
                    onClick={() => {
                      if (isInstalling) return;
                      handleVersionSwitch(version);
                    }}
                    onMouseEnter={() => {
                      setHoveredVersion(version);
                    }}
                    onMouseLeave={() => {
                      setHoveredVersion((prev) => (prev === version ? null : prev));
                      if (hoveredAnchor === version) {
                        setHoveredAnchor(null);
                        hoverDefaultWasActive.current = false;
                      }
                    }}
                className={`relative w-full px-3 py-2 text-left text-sm flex items-center justify-between transition-colors ${
                  isActive
                    ? 'bg-[#333333] text-[#55ff55]'
                    : isInstalling
                      ? 'text-gray-500 bg-[#2a2a2a]'
                      : 'text-gray-300 hover:bg-[#333333] hover:text-white'
                } ${isSwitching || isInstalling ? 'opacity-50 cursor-not-allowed' : ''}`}
              >
                    <div className="flex items-center gap-2 min-w-0">
                      {/* Anchor on the left of version */}
                      <div className="w-4 flex items-center justify-center flex-shrink-0">
                        {onMakeDefault ? (
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              if (!onMakeDefault) return;
                              if (isDefault) {
                                onMakeDefault(null).catch((err) => console.error('Failed to clear default', err));
                              } else {
                                onMakeDefault(version).catch((err) => console.error('Failed to set default', err));
                              }
                            }}
                            onMouseEnter={() => {
                              hoverDefaultWasActive.current = isDefault;
                              setHoveredAnchor(version);
                            }}
                            onMouseLeave={() => {
                              setHoveredAnchor((prev) => (prev === version ? null : prev));
                            }}
                            className="flex items-center justify-center"
                            title={
                              isDefault
                                ? 'Click to unset as default'
                                : 'Click to set as default'
                            }
                            disabled={isSwitching || isLoading}
                          >
                            {isDefault && showAnchorHover && hoverDefaultWasActive.current ? (
                              <CircleX size={14} className="text-gray-500" />
                            ) : isDefault ? (
                              <Anchor
                                size={14}
                                className="text-[#55ff55]"
                              />
                            ) : showHover ? (
                              <Anchor size={14} className="text-gray-500" />
                            ) : (
                              <Anchor size={14} className="text-transparent" />
                            )}
                          </button>
                        ) : (
                          <div className="w-4" />
                        )}
                      </div>
                      <span className="font-medium truncate">{version}</span>
                      {isInstalling && (
                        <span className="px-1.5 py-[2px] text-[10px] rounded-full bg-amber-500/20 border border-amber-400/60 text-amber-200">
                          Installing
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-2 pr-12">
                      {!isInstalling && (showHover || isEnabled) && (
                        <button
                          onClick={async (e) => {
                            e.stopPropagation();
                            if (!window.pywebview?.api?.set_version_shortcuts) {
                              console.warn('Shortcut API not available');
                              return;
                            }

                            const next = !isEnabled;
                            setShortcutState((prev) => ({
                              ...prev,
                              [version]: { menu: next, desktop: next },
                            }));

                            try {
                              if (window.pywebview?.api?.set_version_shortcuts) {
                                const result = await window.pywebview.api.set_version_shortcuts(version, next);
                                if (result?.state) {
                                  setShortcutState((prev) => ({
                                    ...prev,
                                    [version]: {
                                      menu: Boolean(result.state.menu),
                                      desktop: Boolean(result.state.desktop),
                                    },
                                  }));
                                }
                              }
                            } catch (err) {
                              console.error('Failed to toggle shortcuts', err);
                              // revert on failure
                              setShortcutState((prev) => ({
                                ...prev,
                                [version]: { menu: isEnabled, desktop: isEnabled },
                              }));
                            }
                          }}
                          disabled={isSwitching || isLoading}
                          className="absolute right-8 top-1/2 -translate-y-1/2 flex items-center justify-center transition-colors"
                          title={isEnabled ? 'Shortcuts enabled (click to disable)' : 'Shortcuts disabled (click to enable)'}
                        >
                          <Link2
                            size={14}
                            className={isEnabled ? 'text-[#0080ff]' : 'text-gray-500'}
                            style={{ opacity: 1 }}
                            aria-hidden
                          />
                        </button>
                      )}
                      {isActive && (
                        <span className="absolute right-2 top-1/2 -translate-y-1/2">
                          <Check size={14} className="text-[#55ff55]" />
                        </span>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>

            {installedVersions.length === 0 && (
              <div className="px-3 py-4 text-sm text-gray-500 text-center">
                No versions installed
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>

    </div>
  );
}
