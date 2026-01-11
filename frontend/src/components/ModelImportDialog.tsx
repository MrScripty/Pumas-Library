/**
 * Model Import Dialog Component
 *
 * Multi-step wizard for importing model files into the library.
 * Steps: File Review -> Metadata Lookup -> Import Progress -> Complete
 */

import React, { useState, useEffect, useCallback } from 'react';
import {
  X,
  FileBox,
  Loader2,
  CheckCircle2,
  AlertCircle,
  AlertTriangle,
  ChevronRight,
  Shield,
  ShieldAlert,
  ShieldQuestion,
} from 'lucide-react';
import { importAPI } from '../api/import';
import type { ModelImportSpec, SecurityTier } from '../types/pywebview';
import { getLogger } from '../utils/logger';

const logger = getLogger('ModelImportDialog');

/** Import step enumeration */
type ImportStep = 'review' | 'importing' | 'complete';

/** Individual file status during import */
interface FileImportStatus {
  path: string;
  filename: string;
  status: 'pending' | 'importing' | 'success' | 'error';
  error?: string;
  securityTier?: SecurityTier;
  securityAcknowledged?: boolean;
}

interface ModelImportDialogProps {
  /** File paths to import */
  filePaths: string[];
  /** Callback when dialog is closed */
  onClose: () => void;
  /** Callback when import completes successfully */
  onImportComplete: () => void;
}

/**
 * Extract filename from full path.
 */
function getFilename(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

/**
 * Determine security tier based on file extension.
 */
function getSecurityTier(filename: string): SecurityTier {
  const lower = filename.toLowerCase();
  if (lower.endsWith('.safetensors') || lower.endsWith('.gguf') || lower.endsWith('.onnx')) {
    return 'safe';
  }
  if (lower.endsWith('.ckpt') || lower.endsWith('.pt') || lower.endsWith('.bin') || lower.endsWith('.pth')) {
    return 'pickle';
  }
  return 'unknown';
}

/**
 * Get security tier badge styling and text.
 */
function getSecurityBadge(tier: SecurityTier): { className: string; text: string; Icon: typeof Shield } {
  switch (tier) {
    case 'safe':
      return {
        className: 'bg-[hsl(var(--launcher-accent-success)/0.2)] text-[hsl(var(--launcher-accent-success))]',
        text: 'Safe Format',
        Icon: Shield,
      };
    case 'pickle':
      return {
        className: 'bg-[hsl(var(--launcher-accent-error)/0.2)] text-[hsl(var(--launcher-accent-error))]',
        text: 'Pickle Format',
        Icon: ShieldAlert,
      };
    default:
      return {
        className: 'bg-[hsl(var(--launcher-accent-warning)/0.2)] text-[hsl(var(--launcher-accent-warning))]',
        text: 'Unknown Format',
        Icon: ShieldQuestion,
      };
  }
}

export const ModelImportDialog: React.FC<ModelImportDialogProps> = ({
  filePaths,
  onClose,
  onImportComplete,
}) => {
  const [step, setStep] = useState<ImportStep>('review');
  const [files, setFiles] = useState<FileImportStatus[]>([]);
  const [importedCount, setImportedCount] = useState(0);
  const [failedCount, setFailedCount] = useState(0);

  // Initialize file statuses
  useEffect(() => {
    const fileStatuses: FileImportStatus[] = filePaths.map((path) => {
      const filename = getFilename(path);
      const securityTier = getSecurityTier(filename);
      return {
        path,
        filename,
        status: 'pending',
        securityTier,
        securityAcknowledged: securityTier !== 'pickle', // Auto-acknowledge safe formats
      };
    });
    setFiles(fileStatuses);
  }, [filePaths]);

  // Check if all pickle files are acknowledged
  const allPickleAcknowledged = files.every(
    (f) => f.securityTier !== 'pickle' || f.securityAcknowledged
  );

  // Toggle security acknowledgment for a file
  const toggleSecurityAck = useCallback((index: number) => {
    setFiles((prev) => {
      const file = prev[index];
      if (!file) return prev;
      const updated = [...prev];
      updated[index] = {
        ...file,
        securityAcknowledged: !file.securityAcknowledged,
      };
      return updated;
    });
  }, []);

  // Remove a file from the import list
  const removeFile = useCallback((index: number) => {
    setFiles((prev) => prev.filter((_, i) => i !== index));
  }, []);

  // Start the import process
  const startImport = useCallback(async () => {
    if (!allPickleAcknowledged || files.length === 0) return;

    setStep('importing');

    // Build import specs
    const specs: ModelImportSpec[] = files.map((f) => ({
      path: f.path,
      family: 'imported', // Will be determined by backend metadata lookup
      official_name: f.filename.replace(/\.[^.]+$/, ''), // Remove extension
      security_acknowledged: f.securityAcknowledged,
    }));

    try {
      // Mark all as importing
      setFiles((prev) => prev.map((f) => ({ ...f, status: 'importing' })));

      const result = await importAPI.importBatch(specs);

      // Update statuses based on results
      setFiles((prev) =>
        prev.map((f) => {
          const importResult = result.results.find((r) => r.path === f.path);
          if (importResult) {
            return {
              ...f,
              status: importResult.success ? 'success' : 'error',
              error: importResult.error,
              securityTier: importResult.security_tier || f.securityTier,
            };
          }
          return f;
        })
      );

      setImportedCount(result.imported);
      setFailedCount(result.failed);
      setStep('complete');

      if (result.imported > 0) {
        onImportComplete();
      }
    } catch (error) {
      logger.error('Import batch failed', { error });
      setFiles((prev) =>
        prev.map((f) => ({
          ...f,
          status: 'error',
          error: error instanceof Error ? error.message : 'Import failed',
        }))
      );
      setFailedCount(files.length);
      setStep('complete');
    }
  }, [allPickleAcknowledged, files, onImportComplete]);

  // Handle close with confirmation if import in progress
  const handleClose = useCallback(() => {
    if (step === 'importing') {
      // Don't allow closing during import
      return;
    }
    onClose();
  }, [step, onClose]);

  // Count pickle files that need acknowledgment
  const pickleFilesCount = files.filter((f) => f.securityTier === 'pickle').length;
  const acknowledgedCount = files.filter((f) => f.securityTier === 'pickle' && f.securityAcknowledged).length;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-2xl bg-[hsl(var(--launcher-bg-secondary))] rounded-xl shadow-2xl border border-[hsl(var(--launcher-border))] overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-[hsl(var(--launcher-border))]">
          <div className="flex items-center gap-3">
            <FileBox className="w-5 h-5 text-[hsl(var(--launcher-accent-primary))]" />
            <h2 className="text-lg font-semibold text-[hsl(var(--launcher-text-primary))]">
              {step === 'review' && 'Import Models'}
              {step === 'importing' && 'Importing...'}
              {step === 'complete' && 'Import Complete'}
            </h2>
          </div>
          <button
            onClick={handleClose}
            disabled={step === 'importing'}
            className="p-1 rounded-md text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-primary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="px-6 py-4 max-h-[60vh] overflow-y-auto">
          {/* Step 1: Review Files */}
          {step === 'review' && (
            <div className="space-y-4">
              {/* Security warning for pickle files */}
              {pickleFilesCount > 0 && (
                <div className="p-4 rounded-lg border-l-4 border-[hsl(var(--launcher-accent-error))] bg-[hsl(var(--launcher-accent-error)/0.1)]">
                  <div className="flex items-start gap-3">
                    <AlertTriangle className="w-5 h-5 text-[hsl(var(--launcher-accent-error))] flex-shrink-0 mt-0.5" />
                    <div>
                      <p className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
                        {pickleFilesCount} file{pickleFilesCount > 1 ? 's use' : ' uses'} PyTorch pickle format
                      </p>
                      <p className="text-xs text-[hsl(var(--launcher-text-muted))] mt-1">
                        Pickle files can execute arbitrary code. Only import from trusted sources.
                        Check the acknowledgment box for each file to proceed.
                      </p>
                    </div>
                  </div>
                </div>
              )}

              {/* File list */}
              <div className="space-y-2">
                {files.map((file, index) => {
                  const badge = getSecurityBadge(file.securityTier || 'unknown');
                  const BadgeIcon = badge.Icon;

                  return (
                    <div
                      key={file.path}
                      className="flex items-center gap-3 p-3 rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
                    >
                      <FileBox className="w-5 h-5 text-[hsl(var(--launcher-text-muted))] flex-shrink-0" />
                      <div className="flex-1 min-w-0">
                        <p className="text-sm font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                          {file.filename}
                        </p>
                        <p className="text-xs text-[hsl(var(--launcher-text-muted))] truncate">
                          {file.path}
                        </p>
                      </div>
                      <span className={`px-2 py-0.5 rounded text-xs font-medium flex items-center gap-1 ${badge.className}`}>
                        <BadgeIcon className="w-3 h-3" />
                        {badge.text}
                      </span>
                      {file.securityTier === 'pickle' && (
                        <label className="flex items-center gap-2 cursor-pointer">
                          <input
                            type="checkbox"
                            checked={file.securityAcknowledged}
                            onChange={() => toggleSecurityAck(index)}
                            className="w-4 h-4 rounded border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-control))] text-[hsl(var(--launcher-accent-primary))] focus:ring-[hsl(var(--launcher-accent-primary))]"
                          />
                          <span className="text-xs text-[hsl(var(--launcher-text-muted))]">I understand</span>
                        </label>
                      )}
                      <button
                        onClick={() => removeFile(index)}
                        className="p-1 rounded text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-accent-error))] hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors"
                        title="Remove from import"
                      >
                        <X className="w-4 h-4" />
                      </button>
                    </div>
                  );
                })}
              </div>

              {files.length === 0 && (
                <div className="flex flex-col items-center justify-center py-12 text-[hsl(var(--launcher-text-muted))]">
                  <FileBox className="w-12 h-12 mb-3 opacity-50" />
                  <p className="text-sm">No files to import</p>
                </div>
              )}
            </div>
          )}

          {/* Step 2: Importing */}
          {step === 'importing' && (
            <div className="space-y-4">
              <div className="flex items-center justify-center py-8">
                <Loader2 className="w-12 h-12 text-[hsl(var(--launcher-accent-primary))] animate-spin" />
              </div>
              <div className="space-y-2">
                {files.map((file) => (
                  <div
                    key={file.path}
                    className="flex items-center gap-3 p-3 rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
                  >
                    {file.status === 'importing' && (
                      <Loader2 className="w-4 h-4 text-[hsl(var(--launcher-accent-primary))] animate-spin flex-shrink-0" />
                    )}
                    {file.status === 'success' && (
                      <CheckCircle2 className="w-4 h-4 text-[hsl(var(--launcher-accent-success))] flex-shrink-0" />
                    )}
                    {file.status === 'error' && (
                      <AlertCircle className="w-4 h-4 text-[hsl(var(--launcher-accent-error))] flex-shrink-0" />
                    )}
                    {file.status === 'pending' && (
                      <div className="w-4 h-4 rounded-full border-2 border-[hsl(var(--launcher-border))] flex-shrink-0" />
                    )}
                    <span className="text-sm text-[hsl(var(--launcher-text-secondary))] truncate flex-1">
                      {file.filename}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Step 3: Complete */}
          {step === 'complete' && (
            <div className="space-y-4">
              {/* Summary */}
              <div className="flex items-center justify-center py-6">
                {failedCount === 0 ? (
                  <div className="flex flex-col items-center">
                    <CheckCircle2 className="w-16 h-16 text-[hsl(var(--launcher-accent-success))] mb-3" />
                    <p className="text-lg font-medium text-[hsl(var(--launcher-text-primary))]">
                      {importedCount} model{importedCount !== 1 ? 's' : ''} imported successfully
                    </p>
                  </div>
                ) : importedCount === 0 ? (
                  <div className="flex flex-col items-center">
                    <AlertCircle className="w-16 h-16 text-[hsl(var(--launcher-accent-error))] mb-3" />
                    <p className="text-lg font-medium text-[hsl(var(--launcher-text-primary))]">
                      Import failed
                    </p>
                    <p className="text-sm text-[hsl(var(--launcher-text-muted))]">
                      {failedCount} file{failedCount !== 1 ? 's' : ''} could not be imported
                    </p>
                  </div>
                ) : (
                  <div className="flex flex-col items-center">
                    <AlertTriangle className="w-16 h-16 text-[hsl(var(--launcher-accent-warning))] mb-3" />
                    <p className="text-lg font-medium text-[hsl(var(--launcher-text-primary))]">
                      Partial import
                    </p>
                    <p className="text-sm text-[hsl(var(--launcher-text-muted))]">
                      {importedCount} imported, {failedCount} failed
                    </p>
                  </div>
                )}
              </div>

              {/* Results list */}
              <div className="space-y-2">
                {files.map((file) => (
                  <div
                    key={file.path}
                    className="flex items-center gap-3 p-3 rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
                  >
                    {file.status === 'success' && (
                      <CheckCircle2 className="w-4 h-4 text-[hsl(var(--launcher-accent-success))] flex-shrink-0" />
                    )}
                    {file.status === 'error' && (
                      <AlertCircle className="w-4 h-4 text-[hsl(var(--launcher-accent-error))] flex-shrink-0" />
                    )}
                    <div className="flex-1 min-w-0">
                      <p className="text-sm text-[hsl(var(--launcher-text-secondary))] truncate">
                        {file.filename}
                      </p>
                      {file.error && (
                        <p className="text-xs text-[hsl(var(--launcher-accent-error))] truncate">
                          {file.error}
                        </p>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-6 py-4 border-t border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-tertiary)/0.3)]">
          <div className="text-sm text-[hsl(var(--launcher-text-muted))]">
            {step === 'review' && (
              <>
                {files.length} file{files.length !== 1 ? 's' : ''} selected
                {pickleFilesCount > 0 && ` (${acknowledgedCount}/${pickleFilesCount} acknowledged)`}
              </>
            )}
            {step === 'importing' && 'Please wait...'}
            {step === 'complete' && 'Import finished'}
          </div>
          <div className="flex items-center gap-3">
            {step === 'review' && (
              <>
                <button
                  onClick={onClose}
                  className="px-4 py-2 text-sm font-medium text-[hsl(var(--launcher-text-secondary))] hover:text-[hsl(var(--launcher-text-primary))] transition-colors"
                >
                  Cancel
                </button>
                <button
                  onClick={startImport}
                  disabled={!allPickleAcknowledged || files.length === 0}
                  className="flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-lg bg-[hsl(var(--launcher-accent-primary))] text-[hsl(var(--launcher-bg-primary))] hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-opacity"
                >
                  Import
                  <ChevronRight className="w-4 h-4" />
                </button>
              </>
            )}
            {step === 'complete' && (
              <button
                onClick={onClose}
                className="flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-lg bg-[hsl(var(--launcher-accent-primary))] text-[hsl(var(--launcher-bg-primary))] hover:opacity-90 transition-opacity"
              >
                Done
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
