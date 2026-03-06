/**
 * Model Import Dialog Component
 *
 * Multi-step wizard for importing model files into the library.
 * Steps: File Review -> Metadata Lookup -> Import Progress -> Complete
 */

import React from 'react';
import {
  X,
  FileBox,
  Loader2,
  CheckCircle2,
  AlertCircle,
  AlertTriangle,
  ChevronRight,
  ChevronDown,
  ShieldCheck,
  Unlink,
  Folder,
  ExternalLink,
  ToggleLeft,
  ToggleRight,
  FileText,
  Cloud,
} from 'lucide-react';
import type { HFMetadataLookupResult } from '../types/api';
import {
  constructQuantUrl,
  EXCLUDED_FIELDS,
  formatFieldName,
  formatMetadataValue,
  getSecurityBadge,
  getTrustBadge,
  isHiddenGgufField,
  isPriorityGgufField,
  LINKED_GGUF_FIELDS,
  sortMetadataFields,
} from './model-import/metadataUtils';
import { useModelImportWorkflow } from './model-import/useModelImportWorkflow';

interface ModelImportDialogProps {
  /** File paths to import */
  filePaths: string[];
  /** Callback when dialog is closed */
  onClose: () => void;
  /** Callback when import completes successfully */
  onImportComplete: () => void;
}

export const ModelImportDialog: React.FC<ModelImportDialogProps> = ({
  filePaths,
  onClose,
  onImportComplete,
}) => {
  const {
    step,
    files,
    importedCount,
    failedCount,
    shardedSets,
    lookupProgress,
    expandedMetadata,
    showEmbeddedMetadata,
    showAllEmbeddedMetadata,
    allPickleAcknowledged,
    toggleMetadataExpand,
    toggleMetadataSource,
    toggleShowAllEmbeddedMetadata,
    toggleSecurityAck,
    removeFile,
    toggleShardedSet,
    proceedToLookup,
    startImport,
    pickleFilesCount,
    acknowledgedCount,
    invalidFileCount,
    verifiedCount,
    standaloneFiles,
  } = useModelImportWorkflow({ filePaths, onImportComplete });

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
            onClick={onClose}
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
                  const isShowingAllEmbedded = showAllEmbeddedMetadata.has(file.path);
                  const allEmbeddedEntries = file.embeddedMetadata
                    ? Object.entries(file.embeddedMetadata)
                        .filter(([, value]) => value != null && value !== '')
                        .map(([key, value]) => ({
                          key,
                          label: formatFieldName(key),
                          value: formatMetadataValue(key, value),
                          isPriority: isPriorityGgufField(key),
                          isHidden: isHiddenGgufField(key, value),
                        }))
                    : [];

                  // Filter embedded entries based on show all state
                  const embeddedMetadataEntries = allEmbeddedEntries
                    .filter((entry) => isShowingAllEmbedded || (entry.isPriority && !entry.isHidden))
                    .sort((a, b) => {
                      // Priority fields first, then alphabetically
                      if (a.isPriority !== b.isPriority) return a.isPriority ? -1 : 1;
                      return a.key.localeCompare(b.key);
                    });

                  // Count hidden fields for "show more" button
                  const hiddenEmbeddedCount = allEmbeddedEntries.length - embeddedMetadataEntries.length;

                  // Choose which entries to display
                  const metadataEntries = isShowingEmbedded ? embeddedMetadataEntries : hfMetadataEntries;

                  return (
                    <div key={file.path} className="rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]">
                      {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- expandable metadata row */}
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
                                  void toggleMetadataSource(file.path);
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
                              {metadataEntries.map(({ key, label, value }) => {
                                const lowerKey = key.toLowerCase();
                                let linkedUrl = '';

                                if (isShowingEmbedded && file.embeddedMetadata) {
                                  // Check for direct linked field first
                                  const linkedUrlKey = LINKED_GGUF_FIELDS[lowerKey];
                                  if (linkedUrlKey) {
                                    linkedUrl = String(file.embeddedMetadata[linkedUrlKey] ?? '');
                                  }
                                  // For general.name, compute the quant URL from quantized_by + name
                                  else if (lowerKey === 'general.name') {
                                    linkedUrl = constructQuantUrl(file.embeddedMetadata) ?? '';
                                  }
                                }

                                return (
                                  <div key={key} className="contents">
                                    <span className="text-[hsl(var(--launcher-text-muted))]">{label}</span>
                                    {linkedUrl ? (
                                      <a
                                        href={linkedUrl}
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        onClick={(e) => e.stopPropagation()}
                                        className="text-[hsl(var(--launcher-accent-primary))] hover:underline truncate"
                                        title={`${value} (${linkedUrl})`}
                                      >
                                        {value}
                                      </a>
                                    ) : (
                                      <span className="text-[hsl(var(--launcher-text-secondary))] truncate" title={value}>
                                        {value}
                                      </span>
                                    )}
                                  </div>
                                );
                              })}
                            </div>
                          ) : (
                            !isShowingEmbedded && !hasMetadata && (
                              <div className="text-xs text-[hsl(var(--launcher-text-muted))] py-2">
                                No metadata available
                              </div>
                            )
                          )}

                          {/* Show more/less button for embedded metadata */}
                          {isShowingEmbedded && file.embeddedMetadataStatus === 'loaded' && hiddenEmbeddedCount > 0 && (
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                toggleShowAllEmbeddedMetadata(file.path);
                              }}
                              className="mt-2 text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline"
                            >
                              {isShowingAllEmbedded
                                ? 'Show less'
                                : `Show ${hiddenEmbeddedCount} more field${hiddenEmbeddedCount === 1 ? '' : 's'}`}
                            </button>
                          )}

                          {/* Empty embedded metadata */}
                          {isShowingEmbedded && file.embeddedMetadataStatus === 'loaded' && allEmbeddedEntries.length === 0 && (
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
