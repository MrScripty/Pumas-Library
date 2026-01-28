/**
 * Dependency Installation Section
 *
 * Displays dependency installation status and button.
 * Extracted from App.tsx for better separation of concerns.
 */

import { motion, AnimatePresence } from 'framer-motion';
import { ArrowDownToLine, Loader2, AlertTriangle } from 'lucide-react';
import { Tooltip } from './ui';

interface DependencySectionProps {
  depsInstalled: boolean | null;
  isInstalling: boolean;
  comfyUIRunning: boolean;
  onInstall: () => void;
}

export function DependencySection({
  depsInstalled,
  isInstalling,
  comfyUIRunning,
  onInstall,
}: DependencySectionProps) {
  const tooltipContent = isInstalling
    ? 'Installing...'
    : comfyUIRunning
      ? 'Stop app first'
      : 'Install dependencies';

  return (
    <div className="w-full mb-4 min-h-[40px] flex items-center justify-center">
      <AnimatePresence mode="wait">
        {depsInstalled === false ? (
          <Tooltip content={tooltipContent} position="bottom">
            <motion.button
              key="install-btn"
              layout
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.5, transition: { duration: 0.2 } }}
              onClick={onInstall}
              disabled={isInstalling || comfyUIRunning}
              className="p-2 bg-[hsl(var(--surface-interactive))] hover:bg-[hsl(var(--surface-interactive-hover))] text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] flex items-center justify-center transition-colors active:scale-[0.98] rounded disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isInstalling ? (
                <Loader2 className="animate-spin w-5 h-5" />
              ) : comfyUIRunning ? (
                <AlertTriangle className="w-5 h-5 text-[hsl(var(--accent-warning))]" />
              ) : (
                <ArrowDownToLine className="w-5 h-5" />
              )}
            </motion.button>
          </Tooltip>
        ) : null}
      </AnimatePresence>
    </div>
  );
}
