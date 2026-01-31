/**
 * Dependency Status Section for GenericAppPanel.
 *
 * Displays dependency installation status for Python-based apps.
 */

import { motion, AnimatePresence } from 'framer-motion';
import { ArrowDownToLine, Loader2, AlertTriangle, CheckCircle } from 'lucide-react';
import { Tooltip } from '../../ui';

export interface DependencyStatusSectionProps {
  isChecking: boolean;
  isInstalled: boolean | null;
  isInstalling: boolean;
  isAppRunning: boolean;
  onInstall: () => void;
}

export function DependencyStatusSection({
  isChecking,
  isInstalled,
  isInstalling,
  isAppRunning,
  onInstall,
}: DependencyStatusSectionProps) {
  if (isChecking || isInstalled === null) {
    return (
      <div className="w-full flex items-center justify-center gap-2 text-[hsl(var(--text-secondary))] py-4">
        <Loader2 className="w-4 h-4 animate-spin" />
        <span className="text-sm">Checking dependencies...</span>
      </div>
    );
  }

  if (isInstalled === true) {
    return (
      <div className="w-full flex items-center gap-2 text-[hsl(var(--accent-success))] py-2">
        <CheckCircle className="w-4 h-4" />
        <span className="text-sm">Dependencies installed</span>
      </div>
    );
  }

  const tooltipContent = isInstalling
    ? 'Installing...'
    : isAppRunning
      ? 'Stop app first'
      : 'Install dependencies';

  return (
    <div className="w-full mb-4 min-h-[40px] flex items-center justify-center">
      <AnimatePresence mode="wait">
        <Tooltip content={tooltipContent} position="bottom">
          <motion.button
            key="install-btn"
            layout
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.5, transition: { duration: 0.2 } }}
            onClick={onInstall}
            disabled={isInstalling || isAppRunning}
            className="p-2 bg-[hsl(var(--surface-interactive))] hover:bg-[hsl(var(--surface-interactive-hover))] text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] flex items-center justify-center transition-colors active:scale-[0.98] rounded disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isInstalling ? (
              <Loader2 className="animate-spin w-5 h-5" />
            ) : isAppRunning ? (
              <AlertTriangle className="w-5 h-5 text-[hsl(var(--accent-warning))]" />
            ) : (
              <ArrowDownToLine className="w-5 h-5" />
            )}
          </motion.button>
        </Tooltip>
      </AnimatePresence>
    </div>
  );
}
