/**
 * Mapping Preview Dialog Component (Phase 1C)
 *
 * A dialog wrapper for the MappingPreview component, providing a modal
 * interface for previewing and applying model mappings for a specific
 * ComfyUI version.
 */

import React, { useState, useCallback, useEffect } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { motion, AnimatePresence } from 'framer-motion';
import {
  X,
  FolderSymlink,
  AlertTriangle,
  HardDrive,
  Package,
} from 'lucide-react';
import { MappingPreview } from './MappingPreview';
import { getLogger } from '../utils/logger';

const logger = getLogger('MappingPreviewDialog');

interface SandboxInfo {
  is_sandboxed: boolean;
  sandbox_type?: 'flatpak' | 'snap' | 'docker' | 'unknown';
  limitations?: string[];
  recommendation?: string;
}

interface MappingPreviewDialogProps {
  /** Whether the dialog is open */
  isOpen: boolean;
  /** Version tag to preview mapping for */
  versionTag: string;
  /** Callback when dialog is closed */
  onClose: () => void;
  /** Callback when mapping is successfully applied */
  onMappingApplied?: () => void;
}

export const MappingPreviewDialog: React.FC<MappingPreviewDialogProps> = ({
  isOpen,
  versionTag,
  onClose,
  onMappingApplied,
}) => {
  const [sandboxInfo, setSandboxInfo] = useState<SandboxInfo | null>(null);
  const [crossFsWarning, setCrossFsWarning] = useState<{
    cross_filesystem: boolean;
    warning?: string;
    recommendation?: string;
  } | null>(null);

  // Fetch sandbox and cross-filesystem warnings on open
  useEffect(() => {
    if (!isOpen || !versionTag) return;

    const fetchWarnings = async () => {
      try {
        // Check for cross-filesystem warning
        if (isAPIAvailable()) {
          const cfResult = await api.get_cross_filesystem_warning(versionTag);
          if (cfResult.success && cfResult.cross_filesystem) {
            setCrossFsWarning({
              cross_filesystem: true,
              warning: cfResult.warning,
              recommendation: cfResult.recommendation,
            });
          } else {
            setCrossFsWarning(null);
          }
        }

        // Check for sandbox environment
        if (isAPIAvailable()) {
          const sbResult = await api.get_sandbox_info();
          if (sbResult.success && sbResult.is_sandboxed) {
            setSandboxInfo({
              is_sandboxed: true,
              sandbox_type: sbResult.sandbox_type as SandboxInfo['sandbox_type'],
              limitations: sbResult.limitations,
            });
          } else {
            setSandboxInfo(null);
          }
        }
      } catch (error) {
        logger.error('Error fetching warnings', { error });
      }
    };

    void fetchWarnings();
  }, [isOpen, versionTag]);

  const handleMappingApplied = useCallback(
    (result: { links_created: number; links_removed: number }) => {
      logger.info('Mapping applied from dialog', result);
      onMappingApplied?.();
    },
    [onMappingApplied]
  );

  const handleClose = useCallback(() => {
    onClose();
  }, [onClose]);

  // Handle escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) {
        handleClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, handleClose]);

  if (!isOpen) return null;

  return (
    <AnimatePresence>
      {isOpen && (
        <>
          {/* Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 bg-black/50 z-50"
            onClick={handleClose}
          />

          {/* Dialog */}
          <motion.div
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.95 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-50 flex items-center justify-center p-4"
          >
            <div
              className="bg-[hsl(var(--launcher-bg-primary))] rounded-lg shadow-xl border border-[hsl(var(--launcher-border))] w-full max-w-2xl max-h-[80vh] flex flex-col"
              onClick={(e) => e.stopPropagation()}
            >
              {/* Header */}
              <div className="flex items-center justify-between px-6 py-4 border-b border-[hsl(var(--launcher-border))]">
                <div className="flex items-center gap-3">
                  <FolderSymlink className="w-5 h-5 text-[hsl(var(--accent-primary))]" />
                  <div>
                    <h2 className="text-lg font-semibold text-[hsl(var(--launcher-text-primary))]">
                      Sync Library Models
                    </h2>
                    <p className="text-sm text-[hsl(var(--launcher-text-secondary))]">
                      {versionTag}
                    </p>
                  </div>
                </div>
                <button
                  onClick={handleClose}
                  className="p-2 rounded-lg hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors"
                >
                  <X className="w-5 h-5 text-[hsl(var(--launcher-text-secondary))]" />
                </button>
              </div>

              {/* Content */}
              <div className="flex-1 overflow-y-auto p-6 space-y-4">
                {/* Sandbox Warning */}
                {sandboxInfo?.is_sandboxed && (
                  <div className="p-4 bg-[hsl(var(--accent-warning)/0.1)] rounded-lg border border-[hsl(var(--accent-warning)/0.3)]">
                    <div className="flex items-start gap-3">
                      <Package className="w-5 h-5 text-[hsl(var(--accent-warning))] flex-shrink-0 mt-0.5" />
                      <div>
                        <div className="font-medium text-[hsl(var(--accent-warning))]">
                          Sandboxed Environment Detected
                        </div>
                        <div className="text-sm text-[hsl(var(--launcher-text-secondary))] mt-1">
                          Running in {sandboxInfo.sandbox_type || 'a sandboxed'} environment.
                          {sandboxInfo.limitations && sandboxInfo.limitations.length > 0 && (
                            <ul className="list-disc list-inside mt-2 space-y-1">
                              {sandboxInfo.limitations.map((limitation, index) => (
                                <li key={index}>{limitation}</li>
                              ))}
                            </ul>
                          )}
                        </div>
                        {sandboxInfo.recommendation && (
                          <div className="text-sm text-[hsl(var(--launcher-text-tertiary))] mt-2">
                            Tip: {sandboxInfo.recommendation}
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                )}

                {/* Cross-Filesystem Warning */}
                {crossFsWarning?.cross_filesystem && (
                  <div className="p-4 bg-[hsl(var(--accent-warning)/0.1)] rounded-lg border border-[hsl(var(--accent-warning)/0.3)]">
                    <div className="flex items-start gap-3">
                      <HardDrive className="w-5 h-5 text-[hsl(var(--accent-warning))] flex-shrink-0 mt-0.5" />
                      <div>
                        <div className="font-medium text-[hsl(var(--accent-warning))]">
                          Cross-Filesystem Warning
                        </div>
                        <div className="text-sm text-[hsl(var(--launcher-text-secondary))] mt-1">
                          {crossFsWarning.warning ||
                            'Model library and version are on different filesystems. Symlinks may not work correctly.'}
                        </div>
                        {crossFsWarning.recommendation && (
                          <div className="text-sm text-[hsl(var(--launcher-text-tertiary))] mt-2">
                            Tip: {crossFsWarning.recommendation}
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                )}

                {/* Description */}
                <div className="text-sm text-[hsl(var(--launcher-text-secondary))]">
                  Preview and apply model symlink mappings for this ComfyUI version.
                  This will create symlinks from your model library to the version's models
                  directory, making your models available in ComfyUI.
                </div>

                {/* Mapping Preview */}
                <MappingPreview
                  versionTag={versionTag}
                  autoRefresh={true}
                  showApplyButton={true}
                  onMappingApplied={handleMappingApplied}
                />
              </div>

              {/* Footer */}
              <div className="flex items-center justify-end gap-3 px-6 py-4 border-t border-[hsl(var(--launcher-border))]">
                <button
                  onClick={handleClose}
                  className="px-4 py-2 text-sm font-medium text-[hsl(var(--launcher-text-secondary))] hover:text-[hsl(var(--launcher-text-primary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] rounded-lg transition-colors"
                >
                  Close
                </button>
              </div>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
};
