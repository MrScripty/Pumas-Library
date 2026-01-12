/**
 * Model Import Dialog Component
 *
 * Multi-step wizard for importing model files into the library.
 * Steps: File Review -> Metadata Lookup -> Import Progress -> Complete
 */

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import {
  X,
  FileBox,
  Loader2,
  CheckCircle2,
  AlertCircle,
  AlertTriangle,
  ChevronRight,
  ChevronDown,
  Shield,
  ShieldAlert,
  ShieldQuestion,
  ShieldCheck,
  Link,
  Unlink,
  Eye,
  Folder,
  ExternalLink,
  ToggleLeft,
  ToggleRight,
  FileText,
  Cloud,
} from 'lucide-react';
import { importAPI } from '../api/import';
import type {
  ModelImportSpec,
  SecurityTier,
  HFMetadataLookupResult,
} from '../types/api';
import { getLogger } from '../utils/logger';

const logger = getLogger('ModelImportDialog');

/** Import step enumeration */
type ImportStep = 'review' | 'lookup' | 'importing' | 'complete';

/** Metadata match status */
type MetadataStatus = 'pending' | 'found' | 'not_found' | 'error' | 'manual';

/** Individual file status during import */
interface FileImportStatus {
  path: string;
  filename: string;
  status: 'pending' | 'importing' | 'success' | 'error';
  error?: string;
  securityTier?: SecurityTier;
  securityAcknowledged?: boolean;
  /** HuggingFace metadata lookup result */
  hfMetadata?: HFMetadataLookupResult;
  metadataStatus?: MetadataStatus;
  /** Whether this file is part of a sharded set */
  shardedSetKey?: string;
  /** Whether file type is valid */
  validFileType?: boolean;
  detectedFileType?: string;
  /** Embedded metadata from the file itself (GGUF/safetensors) */
  embeddedMetadata?: Record<string, unknown>;
  /** Status of embedded metadata loading */
  embeddedMetadataStatus?: 'pending' | 'loaded' | 'error' | 'unsupported';
}

/** Sharded set info for UI display */
interface ShardedSetInfo {
  key: string;
  files: string[];
  complete: boolean;
  missingShards: number[];
  expanded: boolean;
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

/**
 * Get trust badge based on HF metadata match.
 */
function getTrustBadge(metadata?: HFMetadataLookupResult): {
  className: string;
  text: string;
  Icon: typeof ShieldCheck;
  tooltip: string;
} | null {
  if (!metadata || !metadata.match_method) return null;

  if (metadata.match_method === 'hash' && metadata.match_confidence === 1.0) {
    return {
      className: 'bg-[hsl(var(--launcher-accent-success)/0.2)] text-[hsl(var(--launcher-accent-success))]',
      text: 'Verified',
      Icon: ShieldCheck,
      tooltip: 'Hash matches HuggingFace - file is authentic',
    };
  }

  if (metadata.match_method === 'filename_exact') {
    return {
      className: 'bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--launcher-accent-primary))]',
      text: 'Matched',
      Icon: Link,
      tooltip: `Filename matched: ${metadata.repo_id}`,
    };
  }

  if (metadata.match_method === 'filename_fuzzy') {
    const confidence = metadata.match_confidence ?? 0;
    return {
      className: 'bg-[hsl(var(--launcher-accent-warning)/0.2)] text-[hsl(var(--launcher-accent-warning))]',
      text: 'Possible Match',
      Icon: Eye,
      tooltip: `Possible match: ${metadata.repo_id} (${Math.round(confidence * 100)}% confidence)`,
    };
  }

  return null;
}

/** Priority order for metadata fields (lower = higher priority) */
const FIELD_PRIORITY: Record<string, number> = {
  official_name: 1,
  family: 2,
  model_type: 3,
  subtype: 4,
  variant: 5,
  precision: 6,
  base_model: 7,
  tags: 8,
  description: 9,
  match_confidence: 10,
  match_method: 11,
  matched_filename: 12,
};

/** Fields to exclude from metadata display */
const EXCLUDED_FIELDS = new Set([
  'repo_id', // Shown as clickable link
  'requires_confirmation', // Internal flag
  'hash_mismatch', // Internal flag
  'pending_full_verification', // Internal flag
  'fast_hash', // Technical detail
  'expected_sha256', // Technical detail
  'download_url', // Not relevant for import
]);

/** Sort metadata fields by priority, then alphabetically */
function sortMetadataFields(keys: string[]): string[] {
  return [...keys].sort((a, b) => {
    const priorityA = FIELD_PRIORITY[a] ?? 999;
    const priorityB = FIELD_PRIORITY[b] ?? 999;
    if (priorityA !== priorityB) return priorityA - priorityB;
    return a.localeCompare(b);
  });
}

/** Format field name from snake_case to Title Case */
function formatFieldName(key: string): string {
  return key
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

/** Format metadata value for display */
function formatMetadataValue(key: string, value: unknown): string {
  if (value == null) return '';
  if (Array.isArray(value)) return value.join(', ');
  if (key === 'match_confidence' && typeof value === 'number') {
    return `${Math.round(value * 100)}%`;
  }
  if (typeof value === 'boolean') return value ? 'Yes' : 'No';
  return String(value);
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
  const [shardedSets, setShardedSets] = useState<ShardedSetInfo[]>([]);
  const [lookupProgress, setLookupProgress] = useState({ current: 0, total: 0 });
  const [expandedMetadata, setExpandedMetadata] = useState<Set<string>>(new Set());
  /** Track which files are showing embedded metadata vs HuggingFace metadata */
  const [showEmbeddedMetadata, setShowEmbeddedMetadata] = useState<Set<string>>(new Set());

  /** Toggle expanded state for a file's metadata */
  const toggleMetadataExpand = useCallback((path: string) => {
    setExpandedMetadata((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  /** Toggle between HuggingFace and embedded metadata view for a file */
  const toggleMetadataSource = useCallback(async (path: string) => {
    // Check current state using refs to avoid stale closure issues
    let needsLoad = false;
    setFiles(prev => {
      const file = prev.find(f => f.path === path);
      if (!file) return prev;
      // Check if we need to load embedded metadata
      if (!file.embeddedMetadata && file.embeddedMetadataStatus !== 'error' && file.embeddedMetadataStatus !== 'unsupported' && file.embeddedMetadataStatus !== 'pending') {
        needsLoad = true;
        // Set loading state
        return prev.map(f =>
          f.path === path ? { ...f, embeddedMetadataStatus: 'pending' } : f
        );
      }
      return prev;
    });

    // Only toggle to embedded view - check if we're currently NOT showing embedded
    setShowEmbeddedMetadata(prev => {
      const isCurrentlyShowingEmbedded = prev.has(path);

      // If switching TO embedded and needs to load, trigger the load
      if (!isCurrentlyShowingEmbedded && needsLoad) {
        // Fire off the load (don't await in the state update)
        importAPI.getEmbeddedMetadata(path).then(result => {
          setFiles(prevFiles => prevFiles.map(f => {
            if (f.path !== path) return f;
            if (result.success && result.metadata) {
              return {
                ...f,
                embeddedMetadata: result.metadata,
                embeddedMetadataStatus: 'loaded',
              };
            } else if (result.file_type === 'unsupported') {
              return { ...f, embeddedMetadataStatus: 'unsupported' };
            } else {
              return { ...f, embeddedMetadataStatus: 'error' };
            }
          }));
        }).catch(error => {
          logger.error('Failed to fetch embedded metadata', { path, error });
          setFiles(prevFiles => prevFiles.map(f =>
            f.path === path ? { ...f, embeddedMetadataStatus: 'error' } : f
          ));
        });
      }

      // Toggle the view
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

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
        metadataStatus: 'pending',
      };
    });
    setFiles(fileStatuses);
  }, [filePaths]);

  // Detect sharded sets when files change
  useEffect(() => {
    if (files.length === 0) return;

    const detectShards = async () => {
      try {
        const paths = files.map(f => f.path);
        const result = await importAPI.detectShardedSets(paths);

        if (result.success && result.groups) {
          const sets: ShardedSetInfo[] = [];
          const fileToSetMap: Record<string, string> = {};

          Object.entries(result.groups).forEach(([key, group]) => {
            if (group.files.length > 1) {
              sets.push({
                key,
                files: group.files,
                complete: group.validation.complete,
                missingShards: group.validation.missing_shards,
                expanded: false,
              });
              group.files.forEach(file => {
                fileToSetMap[file] = key;
              });
            }
          });

          setShardedSets(sets);
          setFiles(prev => prev.map(f => ({
            ...f,
            shardedSetKey: fileToSetMap[f.path],
          })));
        }
      } catch (error) {
        logger.error('Failed to detect sharded sets', { error });
      }
    };

    detectShards();
  }, [files.length]); // Only run when file count changes

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

  // Toggle sharded set expansion
  const toggleShardedSet = useCallback((key: string) => {
    setShardedSets(prev => prev.map(set =>
      set.key === key ? { ...set, expanded: !set.expanded } : set
    ));
  }, []);

  // Perform HuggingFace metadata lookup
  const performMetadataLookup = useCallback(async () => {
    setStep('lookup');
    const totalFiles = files.length;
    setLookupProgress({ current: 0, total: totalFiles });

    // Get paths to process (snapshot at start)
    const filesToProcess = files.map(f => ({ path: f.path, filename: f.filename }));

    for (let i = 0; i < filesToProcess.length; i++) {
      const file = filesToProcess[i];
      if (!file) continue;

      setLookupProgress({ current: i + 1, total: totalFiles });

      try {
        // First validate file type
        const typeResult = await importAPI.validateFileType(file.path);

        // Update this file incrementally (preserves other state including embedded metadata)
        setFiles(prev => prev.map(f => {
          if (f.path !== file.path) return f;
          return {
            ...f,
            validFileType: typeResult.valid,
            detectedFileType: typeResult.detected_type,
            metadataStatus: typeResult.valid ? f.metadataStatus : 'error',
          };
        }));

        if (!typeResult.valid) {
          continue;
        }

        // Then lookup HF metadata
        const result = await importAPI.lookupHFMetadata(file.filename, file.path);

        // Update with HF metadata (preserves other state including embedded metadata)
        setFiles(prev => prev.map(f => {
          if (f.path !== file.path) return f;
          if (result.success && result.found && result.metadata) {
            return {
              ...f,
              hfMetadata: result.metadata,
              metadataStatus: 'found',
            };
          } else {
            return {
              ...f,
              metadataStatus: 'not_found',
            };
          }
        }));
      } catch (error) {
        logger.error('Metadata lookup failed', { file: file.filename, error });
        setFiles(prev => prev.map(f =>
          f.path === file.path ? { ...f, metadataStatus: 'error' } : f
        ));
      }
    }
  }, [files]);

  // Start the import process
  const startImport = useCallback(async () => {
    if (!allPickleAcknowledged || files.length === 0) return;

    setStep('importing');

    // Build import specs with HF metadata
    const specs: ModelImportSpec[] = files
      .filter(f => f.validFileType !== false) // Skip invalid file types
      .map((f) => ({
        path: f.path,
        family: f.hfMetadata?.family || 'imported',
        official_name: f.hfMetadata?.official_name || f.filename.replace(/\.[^.]+$/, ''),
        repo_id: f.hfMetadata?.repo_id,
        model_type: f.hfMetadata?.model_type,
        subtype: f.hfMetadata?.subtype,
        tags: f.hfMetadata?.tags,
        security_acknowledged: f.securityAcknowledged,
      }));

    try {
      // Mark all as importing
      setFiles((prev) => prev.map((f) => ({
        ...f,
        status: f.validFileType !== false ? 'importing' : 'error',
      })));

      const result = await importAPI.importBatch(specs);

      // Update statuses based on results
      setFiles((prev) =>
        prev.map((f) => {
          if (f.validFileType === false) {
            return {
              ...f,
              status: 'error',
              error: `Invalid file type: ${f.detectedFileType}`,
            };
          }
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
      setFailedCount(result.failed + files.filter(f => f.validFileType === false).length);
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

  // Proceed from review to lookup
  const proceedToLookup = useCallback(() => {
    performMetadataLookup();
  }, [performMetadataLookup]);

  // Handle close - closes immediately, async operations will be ignored when component unmounts
  const handleClose = useCallback(() => {
    onClose();
  }, [onClose]);

  // Count pickle files that need acknowledgment
  const pickleFilesCount = files.filter((f) => f.securityTier === 'pickle').length;
  const acknowledgedCount = files.filter((f) => f.securityTier === 'pickle' && f.securityAcknowledged).length;

  // Count files with invalid file types
  const invalidFileCount = files.filter(f => f.validFileType === false).length;

  // Count verified vs unverified files
  const verifiedCount = files.filter(f =>
    f.hfMetadata?.match_method === 'hash' && f.hfMetadata?.match_confidence === 1.0
  ).length;

  // Group files by sharded set for display
  const standaloneFiles = useMemo(() =>
    files.filter(f => !f.shardedSetKey),
    [files]
  );

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-2xl bg-[hsl(var(--launcher-bg-secondary))] rounded-xl shadow-2xl border border-[hsl(var(--launcher-border))] overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-[hsl(var(--launcher-border))]">
          <div className="flex items-center gap-3">
            <FileBox className="w-5 h-5 text-[hsl(var(--launcher-accent-primary))]" />
            <h2 className="text-lg font-semibold text-[hsl(var(--launcher-text-primary))]">
              {step === 'review' && 'Import Models'}
              {step === 'lookup' && 'Looking up metadata...'}
              {step === 'importing' && 'Importing...'}
              {step === 'complete' && 'Import Complete'}
            </h2>
          </div>
          <button
            onClick={handleClose}
            className="p-1 rounded-md text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-primary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors"
            title={(step === 'importing' || step === 'lookup') ? 'Click to cancel' : 'Close'}
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="px-6 py-4 max-h-[60vh] overflow-y-auto">
          {/* Step 1: Review Files */}
          {step === 'review' && (
            <div className="space-y-4">
              {/* Sharded sets warning */}
              {shardedSets.some(s => !s.complete) && (
                <div className="p-4 rounded-lg border-l-4 border-[hsl(var(--launcher-accent-warning))] bg-[hsl(var(--launcher-accent-warning)/0.1)]">
                  <div className="flex items-start gap-3">
                    <Unlink className="w-5 h-5 text-[hsl(var(--launcher-accent-warning))] flex-shrink-0 mt-0.5" />
                    <div>
                      <p className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
                        Incomplete sharded model detected
                      </p>
                      <p className="text-xs text-[hsl(var(--launcher-text-muted))] mt-1">
                        Some model shards are missing. The model may not work correctly.
                      </p>
                    </div>
                  </div>
                </div>
              )}

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

              {/* Sharded sets */}
              {shardedSets.length > 0 && (
                <div className="space-y-2">
                  <h3 className="text-sm font-medium text-[hsl(var(--launcher-text-secondary))] flex items-center gap-2">
                    <Folder className="w-4 h-4" />
                    Sharded Models ({shardedSets.length})
                  </h3>
                  {shardedSets.map((set) => (
                    <div
                      key={set.key}
                      className="rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)] overflow-hidden"
                    >
                      <button
                        onClick={() => toggleShardedSet(set.key)}
                        className="w-full flex items-center gap-3 p-3 hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors"
                      >
                        <Folder className="w-5 h-5 text-[hsl(var(--launcher-accent-primary))] flex-shrink-0" />
                        <div className="flex-1 text-left">
                          <p className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
                            {set.key}
                          </p>
                          <p className="text-xs text-[hsl(var(--launcher-text-muted))]">
                            {set.files.length} shards
                          </p>
                        </div>
                        {set.complete ? (
                          <span className="px-2 py-0.5 rounded text-xs font-medium bg-[hsl(var(--launcher-accent-success)/0.2)] text-[hsl(var(--launcher-accent-success))] flex items-center gap-1">
                            <CheckCircle2 className="w-3 h-3" />
                            Complete
                          </span>
                        ) : (
                          <span className="px-2 py-0.5 rounded text-xs font-medium bg-[hsl(var(--launcher-accent-warning)/0.2)] text-[hsl(var(--launcher-accent-warning))] flex items-center gap-1">
                            <AlertTriangle className="w-3 h-3" />
                            Missing {set.missingShards.length} shard(s)
                          </span>
                        )}
                        <ChevronRight className={`w-4 h-4 text-[hsl(var(--launcher-text-muted))] transition-transform ${set.expanded ? 'rotate-90' : ''}`} />
                      </button>
                      {set.expanded && (
                        <div className="px-3 pb-3 space-y-1">
                          {set.files.map((filePath) => {
                            const file = files.find(f => f.path === filePath);
                            if (!file) return null;
                            return (
                              <div
                                key={filePath}
                                className="flex items-center gap-2 p-2 rounded bg-[hsl(var(--launcher-bg-secondary))] text-xs text-[hsl(var(--launcher-text-muted))]"
                              >
                                <FileBox className="w-3 h-3" />
                                {file.filename}
                              </div>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}

              {/* Standalone file list */}
              {standaloneFiles.length > 0 && (
                <div className="space-y-2">
                  {shardedSets.length > 0 && (
                    <h3 className="text-sm font-medium text-[hsl(var(--launcher-text-secondary))]">
                      Individual Files ({standaloneFiles.length})
                    </h3>
                  )}
                  {standaloneFiles.map((file) => {
                    const realIndex = files.findIndex(f => f.path === file.path);
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
                              onChange={() => toggleSecurityAck(realIndex)}
                              className="w-4 h-4 rounded border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-control))] text-[hsl(var(--launcher-accent-primary))] focus:ring-[hsl(var(--launcher-accent-primary))]"
                            />
                            <span className="text-xs text-[hsl(var(--launcher-text-muted))]">I understand</span>
                          </label>
                        )}
                        <button
                          onClick={() => removeFile(realIndex)}
                          className="p-1 rounded text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-accent-error))] hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors"
                          title="Remove from import"
                        >
                          <X className="w-4 h-4" />
                        </button>
                      </div>
                    );
                  })}
                </div>
              )}

              {files.length === 0 && (
                <div className="flex flex-col items-center justify-center py-12 text-[hsl(var(--launcher-text-muted))]">
                  <FileBox className="w-12 h-12 mb-3 opacity-50" />
                  <p className="text-sm">No files to import</p>
                </div>
              )}
            </div>
          )}

          {/* Step 2: Metadata Lookup */}
          {step === 'lookup' && (
            <div className="space-y-4">
              <div className="flex flex-col items-center justify-center py-8">
                <Loader2 className="w-12 h-12 text-[hsl(var(--launcher-accent-primary))] animate-spin mb-4" />
                <p className="text-sm text-[hsl(var(--launcher-text-secondary))]">
                  Looking up metadata ({lookupProgress.current}/{lookupProgress.total})
                </p>
              </div>

              <div className="space-y-2">
                {files.map((file) => {
                  const trustBadge = getTrustBadge(file.hfMetadata);
                  const isExpanded = expandedMetadata.has(file.path);
                  const hasMetadata = file.hfMetadata && file.metadataStatus === 'found';
                  const isShowingEmbedded = showEmbeddedMetadata.has(file.path);
                  const canShowEmbedded = file.detectedFileType === 'gguf' || file.detectedFileType === 'safetensors';

                  // Get displayable HuggingFace metadata fields
                  const hfMetadataEntries = hasMetadata
                    ? sortMetadataFields(
                        Object.keys(file.hfMetadata!).filter(
                          (key) =>
                            !EXCLUDED_FIELDS.has(key) &&
                            file.hfMetadata![key as keyof HFMetadataLookupResult] != null &&
                            file.hfMetadata![key as keyof HFMetadataLookupResult] !== ''
                        )
                      ).map((key) => ({
                        key,
                        label: formatFieldName(key),
                        value: formatMetadataValue(
                          key,
                          file.hfMetadata![key as keyof HFMetadataLookupResult]
                        ),
                      }))
                    : [];

                  // Get displayable embedded metadata fields
                  const embeddedMetadataEntries = file.embeddedMetadata
                    ? Object.entries(file.embeddedMetadata)
                        .filter(([, value]) => value != null && value !== '')
                        .sort(([a], [b]) => a.localeCompare(b))
                        .map(([key, value]) => ({
                          key,
                          label: formatFieldName(key),
                          value: formatMetadataValue(key, value),
                        }))
                    : [];

                  // Choose which entries to display
                  const metadataEntries = isShowingEmbedded ? embeddedMetadataEntries : hfMetadataEntries;

                  return (
                    <div key={file.path} className="rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]">
                      <div
                        className={`flex items-center gap-3 p-3 ${hasMetadata || canShowEmbedded ? 'cursor-pointer hover:bg-[hsl(var(--launcher-bg-tertiary)/0.8)]' : ''}`}
                        onClick={(hasMetadata || canShowEmbedded) ? () => toggleMetadataExpand(file.path) : undefined}
                      >
                        {/* Expand/collapse chevron */}
                        {(hasMetadata || canShowEmbedded) ? (
                          <button
                            className="w-4 h-4 flex-shrink-0 text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))]"
                            onClick={(e) => {
                              e.stopPropagation();
                              toggleMetadataExpand(file.path);
                            }}
                          >
                            {isExpanded ? (
                              <ChevronDown className="w-4 h-4" />
                            ) : (
                              <ChevronRight className="w-4 h-4" />
                            )}
                          </button>
                        ) : (
                          <div className="w-4 h-4 flex-shrink-0" />
                        )}

                        {/* Status icon */}
                        {file.metadataStatus === 'pending' && (
                          <div className="w-4 h-4 rounded-full border-2 border-[hsl(var(--launcher-border))] flex-shrink-0" />
                        )}
                        {file.metadataStatus === 'found' && (
                          <CheckCircle2 className="w-4 h-4 text-[hsl(var(--launcher-accent-success))] flex-shrink-0" />
                        )}
                        {file.metadataStatus === 'not_found' && (
                          <AlertCircle className="w-4 h-4 text-[hsl(var(--launcher-accent-warning))] flex-shrink-0" />
                        )}
                        {file.metadataStatus === 'error' && (
                          <AlertCircle className="w-4 h-4 text-[hsl(var(--launcher-accent-error))] flex-shrink-0" />
                        )}

                        {/* File info */}
                        <div className="flex-1 min-w-0">
                          <p className="text-sm text-[hsl(var(--launcher-text-secondary))] truncate">
                            {file.filename}
                          </p>
                          {file.hfMetadata?.repo_id && (
                            <a
                              href={`https://huggingface.co/${file.hfMetadata.repo_id}`}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline truncate flex items-center gap-1"
                              onClick={(e) => e.stopPropagation()}
                            >
                              {file.hfMetadata.repo_id}
                              <ExternalLink className="w-3 h-3 flex-shrink-0" />
                            </a>
                          )}
                        </div>

                        {/* Trust badge */}
                        {trustBadge && (
                          <span
                            className={`px-2 py-0.5 rounded text-xs font-medium flex items-center gap-1 ${trustBadge.className}`}
                            title={trustBadge.tooltip}
                          >
                            <trustBadge.Icon className="w-3 h-3" />
                            {trustBadge.text}
                          </span>
                        )}
                      </div>

                      {/* Expanded metadata panel */}
                      {isExpanded && (
                        <div className="px-3 pb-3 pt-1 ml-8 border-t border-[hsl(var(--launcher-border)/0.5)]">
                          {/* Metadata source toggle - only show for GGUF/safetensors files */}
                          {canShowEmbedded && (
                            <div className="flex items-center justify-between mb-3 pb-2 border-b border-[hsl(var(--launcher-border)/0.3)]">
                              <span className="text-xs text-[hsl(var(--launcher-text-muted))]">Metadata Source</span>
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  toggleMetadataSource(file.path);
                                }}
                                className="flex items-center gap-2 px-2 py-1 rounded-md text-xs font-medium transition-colors hover:bg-[hsl(var(--launcher-bg-tertiary))]"
                                title={isShowingEmbedded ? 'Switch to HuggingFace metadata' : 'Switch to embedded file metadata'}
                              >
                                {isShowingEmbedded ? (
                                  <>
                                    <FileText className="w-3 h-3 text-[hsl(var(--launcher-accent-warning))]" />
                                    <span className="text-[hsl(var(--launcher-accent-warning))]">Embedded</span>
                                    <ToggleRight className="w-4 h-4 text-[hsl(var(--launcher-accent-warning))]" />
                                  </>
                                ) : (
                                  <>
                                    <Cloud className="w-3 h-3 text-[hsl(var(--launcher-accent-primary))]" />
                                    <span className="text-[hsl(var(--launcher-accent-primary))]">HuggingFace</span>
                                    <ToggleLeft className="w-4 h-4 text-[hsl(var(--launcher-accent-primary))]" />
                                  </>
                                )}
                              </button>
                            </div>
                          )}

                          {/* Loading state for embedded metadata */}
                          {isShowingEmbedded && file.embeddedMetadataStatus === 'pending' && (
                            <div className="flex items-center justify-center py-4">
                              <Loader2 className="w-5 h-5 text-[hsl(var(--launcher-accent-primary))] animate-spin" />
                              <span className="ml-2 text-xs text-[hsl(var(--launcher-text-muted))]">Loading embedded metadata...</span>
                            </div>
                          )}

                          {/* Error state for embedded metadata */}
                          {isShowingEmbedded && file.embeddedMetadataStatus === 'error' && (
                            <div className="flex items-center gap-2 py-2 text-xs text-[hsl(var(--launcher-accent-error))]">
                              <AlertCircle className="w-4 h-4" />
                              Failed to load embedded metadata
                            </div>
                          )}

                          {/* Unsupported state */}
                          {isShowingEmbedded && file.embeddedMetadataStatus === 'unsupported' && (
                            <div className="flex items-center gap-2 py-2 text-xs text-[hsl(var(--launcher-text-muted))]">
                              <AlertCircle className="w-4 h-4" />
                              This file format does not support embedded metadata
                            </div>
                          )}

                          {/* Metadata grid */}
                          {metadataEntries.length > 0 ? (
                            <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs max-h-48 overflow-y-auto">
                              {metadataEntries.map(({ key, label, value }) => (
                                <div key={key} className="contents">
                                  <span className="text-[hsl(var(--launcher-text-muted))]">{label}</span>
                                  <span className="text-[hsl(var(--launcher-text-secondary))] truncate" title={value}>
                                    {value}
                                  </span>
                                </div>
                              ))}
                            </div>
                          ) : (
                            !isShowingEmbedded && !hasMetadata && (
                              <div className="text-xs text-[hsl(var(--launcher-text-muted))] py-2">
                                No metadata available
                              </div>
                            )
                          )}

                          {/* Empty embedded metadata */}
                          {isShowingEmbedded && file.embeddedMetadataStatus === 'loaded' && embeddedMetadataEntries.length === 0 && (
                            <div className="text-xs text-[hsl(var(--launcher-text-muted))] py-2">
                              No embedded metadata found in file
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Step 3: Importing */}
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

          {/* Step 4: Complete */}
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
                    {verifiedCount > 0 && (
                      <p className="text-sm text-[hsl(var(--launcher-text-muted))] flex items-center gap-1 mt-1">
                        <ShieldCheck className="w-4 h-4 text-[hsl(var(--launcher-accent-success))]" />
                        {verifiedCount} verified from HuggingFace
                      </p>
                    )}
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
                {files.map((file) => {
                  const trustBadge = getTrustBadge(file.hfMetadata);

                  return (
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
                      {trustBadge && file.status === 'success' && (
                        <span
                          className={`px-2 py-0.5 rounded text-xs font-medium flex items-center gap-1 ${trustBadge.className}`}
                          title={trustBadge.tooltip}
                        >
                          <trustBadge.Icon className="w-3 h-3" />
                          {trustBadge.text}
                        </span>
                      )}
                    </div>
                  );
                })}
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
                {shardedSets.length > 0 && ` • ${shardedSets.length} sharded set${shardedSets.length > 1 ? 's' : ''}`}
              </>
            )}
            {step === 'lookup' && `Looking up ${lookupProgress.current}/${lookupProgress.total}...`}
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
                  onClick={proceedToLookup}
                  disabled={!allPickleAcknowledged || files.length === 0}
                  className="flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-lg bg-[hsl(var(--launcher-accent-primary))] text-[hsl(var(--launcher-bg-primary))] hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-opacity"
                >
                  Continue
                  <ChevronRight className="w-4 h-4" />
                </button>
              </>
            )}
            {step === 'lookup' && (
              <button
                onClick={startImport}
                disabled={lookupProgress.current < lookupProgress.total}
                className="flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-lg bg-[hsl(var(--launcher-accent-primary))] text-[hsl(var(--launcher-bg-primary))] hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-opacity"
              >
                Import{invalidFileCount > 0 ? ` (${files.length - invalidFileCount})` : ''}
                <ChevronRight className="w-4 h-4" />
              </button>
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
