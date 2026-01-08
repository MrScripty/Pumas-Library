/**
 * Dependency Installation Section
 *
 * Displays dependency installation status and button.
 * Extracted from App.tsx for better separation of concerns.
 */

import { motion, AnimatePresence } from 'framer-motion';
import { ArrowDownToLine, Loader2 } from 'lucide-react';

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
  return (
    <div className="w-full mb-6 min-h-[50px] flex items-center justify-center">
      <AnimatePresence mode="wait">
        {depsInstalled === false ? (
          <motion.button
            key="install-btn"
            layout
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.5, transition: { duration: 0.2 } }}
            onClick={onInstall}
            disabled={isInstalling || comfyUIRunning}
            className="w-full h-12 bg-[hsl(var(--surface-interactive))] hover:bg-[hsl(var(--surface-interactive-hover))] text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] font-bold text-sm flex items-center justify-center gap-3 transition-colors active:scale-[0.98] rounded-sm disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isInstalling ? (
              <>
                <Loader2 className="animate-spin" size={18} />
                <span>Installing (Check Terminal)...</span>
              </>
            ) : comfyUIRunning ? (
              <>
                <ArrowDownToLine size={18} />
                <span>Stop ComfyUI to Install</span>
              </>
            ) : (
              <>
                <ArrowDownToLine size={18} />
                <span>Install Missing Dependencies</span>
              </>
            )}
          </motion.button>
        ) : null}
      </AnimatePresence>
    </div>
  );
}
