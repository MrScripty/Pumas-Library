import React, { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronDown, RefreshCw, Check, Loader2, Download } from 'lucide-react';
import { useVersions } from '../hooks/useVersions';
import { InstallDialog } from './InstallDialog';

export function VersionSelector() {
  const {
    installedVersions,
    activeVersion,
    availableVersions,
    isLoading,
    switchVersion,
    installVersion,
    refreshAll,
  } = useVersions();

  const [isOpen, setIsOpen] = useState(false);
  const [isSwitching, setIsSwitching] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isInstallDialogOpen, setIsInstallDialogOpen] = useState(false);

  console.log('VersionSelector mounted - installedVersions:', installedVersions.length, 'availableVersions:', availableVersions.length);

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

  const handleRefresh = async (e: React.MouseEvent) => {
    e.stopPropagation();
    setIsRefreshing(true);
    try {
      await refreshAll(true); // Force refresh from GitHub
    } finally {
      setIsRefreshing(false);
    }
  };

  const displayVersion = activeVersion || 'No version selected';
  const hasInstalledVersions = installedVersions.length > 0;

  return (
    <div className="relative w-full">
      {/* Version Selector Container - Changed from button to div to allow nested buttons */}
      <div
        className={`w-full h-10 bg-[#2a2a2a] border border-[#444] rounded flex items-center justify-between px-3 transition-colors ${
          !hasInstalledVersions || isLoading || isSwitching
            ? 'opacity-50'
            : ''
        }`}
      >
        {/* Left side - clickable area for version selector */}
        <button
          onClick={() => {
            console.log('Version selector clicked, hasInstalledVersions:', hasInstalledVersions, 'isOpen:', isOpen);
            if (hasInstalledVersions) {
              setIsOpen(!isOpen);
              console.log('Set isOpen to:', !isOpen);
            }
          }}
          disabled={!hasInstalledVersions || isLoading || isSwitching}
          className="flex items-center gap-2 flex-1 hover:opacity-80 transition-opacity disabled:cursor-not-allowed"
        >
          {isSwitching ? (
            <Loader2 size={14} className="text-gray-400 animate-spin" />
          ) : (
            <div className="w-2 h-2 rounded-full bg-[#55ff55]" />
          )}
          <span className="text-sm text-white font-medium">
            {isSwitching ? 'Switching...' : displayVersion}
          </span>
        </button>

        {/* Right side - action buttons */}
        <div className="flex items-center gap-2">
          {/* Download Button */}
          <motion.button
            onClick={(e) => {
              console.log('Download button clicked!');
              e.stopPropagation();
              setIsInstallDialogOpen(true);
              console.log('Install dialog state set to true');
            }}
            disabled={isLoading}
            className="p-1 rounded hover:bg-[#444] transition-colors disabled:opacity-50"
            whileHover={{ scale: 1.1 }}
            whileTap={{ scale: 0.9 }}
            title="Install new version"
          >
            <Download size={14} className="text-gray-400" />
          </motion.button>

          {/* Refresh Button */}
          <motion.button
            onClick={handleRefresh}
            disabled={isRefreshing || isLoading}
            className="p-1 rounded hover:bg-[#444] transition-colors disabled:opacity-50"
            whileHover={{ scale: 1.1 }}
            whileTap={{ scale: 0.9 }}
            title="Refresh from GitHub"
          >
            <RefreshCw
              size={14}
              className={`text-gray-400 ${isRefreshing ? 'animate-spin' : ''}`}
            />
          </motion.button>

          {/* Dropdown Arrow */}
          {hasInstalledVersions && (
            <motion.div
              animate={{ rotate: isOpen ? 180 : 0 }}
              transition={{ duration: 0.2 }}
            >
              <ChevronDown size={16} className="text-gray-400" />
            </motion.div>
          )}
        </div>
      </div>

      {/* Dropdown Menu */}
      <AnimatePresence>
        {isOpen && hasInstalledVersions && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            transition={{ duration: 0.2 }}
            className="absolute top-full left-0 right-0 mt-1 bg-[#2a2a2a] border border-[#444] rounded shadow-lg overflow-hidden z-50"
          >
            <div className="max-h-64 overflow-y-auto">
              {installedVersions.map((version) => {
                const isActive = version === activeVersion;
                return (
                  <button
                    key={version}
                    onClick={() => handleVersionSwitch(version)}
                    disabled={isSwitching}
                    className={`w-full px-3 py-2 text-left text-sm flex items-center justify-between transition-colors ${
                      isActive
                        ? 'bg-[#333333] text-[#55ff55]'
                        : 'text-gray-300 hover:bg-[#333333] hover:text-white'
                    } ${isSwitching ? 'opacity-50 cursor-not-allowed' : ''}`}
                  >
                    <span className="font-medium">{version}</span>
                    {isActive && <Check size={14} className="text-[#55ff55]" />}
                  </button>
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

      {/* Install Dialog */}
      <InstallDialog
        isOpen={isInstallDialogOpen}
        onClose={() => setIsInstallDialogOpen(false)}
        availableVersions={availableVersions}
        installedVersions={installedVersions}
        isLoading={isLoading}
        onInstallVersion={installVersion}
        onRefreshAll={refreshAll}
      />
    </div>
  );
}
