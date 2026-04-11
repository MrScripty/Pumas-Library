/**
 * Model Import Dialog Component
 *
 * Multi-step wizard for importing model files and directories into the library.
 * Steps: Classification -> Review -> Metadata Lookup -> Import Progress -> Complete
 */

import React from 'react';
import {
  X,
  FileBox,
  Folder,
  Loader2,
  CheckCircle2,
  AlertCircle,
  AlertTriangle,
  ChevronRight,
  ShieldCheck,
  Unlink,
  Package2,
} from 'lucide-react';
import {
  getSecurityBadge,
  getTrustBadge,
} from './model-import/metadataUtils';
import { ImportBundleComponents } from './model-import/ImportBundleComponents';
import { ImportLookupCard } from './model-import/ImportLookupCard';
import { useModelImportWorkflow, type ImportEntryStatus } from './model-import/useModelImportWorkflow';

interface ModelImportDialogProps {
  /** Paths to import */
  importPaths: string[];
  /** Callback when dialog is closed */
  onClose: () => void;
  /** Callback when import completes successfully */
  onImportComplete: () => void;
}

function EntryIcon({ entry }: { entry: ImportEntryStatus }) {
  if (entry.kind === 'external_diffusers_bundle') {
    return <Package2 className="w-5 h-5 text-[hsl(var(--launcher-accent-primary))] flex-shrink-0" />;
  }
  if (entry.kind === 'directory_model') {
    return <Folder className="w-5 h-5 text-[hsl(var(--launcher-accent-primary))] flex-shrink-0" />;
  }
  return <FileBox className="w-5 h-5 text-[hsl(var(--launcher-text-muted))] flex-shrink-0" />;
}

function EntryBadge({ entry }: { entry: ImportEntryStatus }) {
  if (entry.kind === 'external_diffusers_bundle') {
    return (
      <span className="px-2 py-0.5 rounded text-xs font-medium bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--launcher-accent-primary))]">
        Bundle
      </span>
    );
  }
  if (entry.kind === 'directory_model') {
    return (
      <span className="px-2 py-0.5 rounded text-xs font-medium bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--launcher-accent-primary))]">
        Directory
      </span>
    );
  }

  const badge = getSecurityBadge(entry.securityTier || 'unknown');
  const BadgeIcon = badge.Icon;
  return (
    <span className={`px-2 py-0.5 rounded text-xs font-medium flex items-center gap-1 ${badge.className}`}>
      <BadgeIcon className="w-3 h-3" />
      {badge.text}
    </span>
  );
}

export const ModelImportDialog: React.FC<ModelImportDialogProps> = ({
  importPaths,
  onClose,
  onImportComplete,
}) => {
  const {
    step,
    entries,
    fileEntries,
    nonFileEntries,
    blockedFindings,
    containerFindings,
    classificationError,
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
    removeEntry,
    toggleShardedSet,
    proceedToLookup,
    startImport,
    pickleFilesCount,
    acknowledgedCount,
    invalidFileCount,
    verifiedCount,
    standaloneEntries,
  } = useModelImportWorkflow({ importPaths, onImportComplete });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-3xl bg-[hsl(var(--launcher-bg-secondary))] rounded-xl shadow-2xl border border-[hsl(var(--launcher-border))] overflow-hidden">
        <div className="flex items-center justify-between px-6 py-4 border-b border-[hsl(var(--launcher-border))]">
          <div className="flex items-center gap-3">
            <FileBox className="w-5 h-5 text-[hsl(var(--launcher-accent-primary))]" />
            <h2 className="text-lg font-semibold text-[hsl(var(--launcher-text-primary))]">
              {step === 'classifying' && 'Inspecting import paths...'}
              {step === 'review' && 'Import Models'}
              {step === 'lookup' && 'Looking up metadata...'}
              {step === 'importing' && 'Importing...'}
              {step === 'complete' && 'Import Complete'}
            </h2>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded-md text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-primary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors"
            title={(step === 'importing' || step === 'lookup' || step === 'classifying') ? 'Close' : 'Close'}
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="px-6 py-4 max-h-[60vh] overflow-y-auto">
          {step === 'classifying' && (
            <div className="flex flex-col items-center justify-center py-12">
              <Loader2 className="w-12 h-12 text-[hsl(var(--launcher-accent-primary))] animate-spin mb-4" />
              <p className="text-sm text-[hsl(var(--launcher-text-secondary))]">
                Classifying files, bundle roots, and model folders...
              </p>
            </div>
          )}

          {step === 'review' && (
            <div className="space-y-4">
              {classificationError && (
                <div className="p-4 rounded-lg border-l-4 border-[hsl(var(--launcher-accent-error))] bg-[hsl(var(--launcher-accent-error)/0.1)]">
                  <div className="flex items-start gap-3">
                    <AlertCircle className="w-5 h-5 text-[hsl(var(--launcher-accent-error))] flex-shrink-0 mt-0.5" />
                    <div>
                      <p className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
                        Failed to classify import paths
                      </p>
                      <p className="text-xs text-[hsl(var(--launcher-text-muted))] mt-1">
                        {classificationError}
                      </p>
                    </div>
                  </div>
                </div>
              )}

              {blockedFindings.length > 0 && (
                <div className="space-y-2">
                  <h3 className="text-sm font-medium text-[hsl(var(--launcher-text-secondary))]">
                    Blocked Paths ({blockedFindings.length})
                  </h3>
                  {blockedFindings.map((finding) => (
                    <div
                      key={finding.path}
                      className="p-4 rounded-lg border-l-4 border-[hsl(var(--launcher-accent-warning))] bg-[hsl(var(--launcher-accent-warning)/0.1)]"
                    >
                      <div className="flex items-start gap-3">
                        <AlertTriangle className="w-5 h-5 text-[hsl(var(--launcher-accent-warning))] flex-shrink-0 mt-0.5" />
                        <div className="min-w-0">
                          <p className="text-sm font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                            {finding.path}
                          </p>
                          {finding.reasons.map((reason) => (
                            <p key={reason} className="text-xs text-[hsl(var(--launcher-text-muted))] mt-1">
                              {reason}
                            </p>
                          ))}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}

              {containerFindings.length > 0 && (
                <div className="space-y-2">
                  <h3 className="text-sm font-medium text-[hsl(var(--launcher-text-secondary))]">
                    Expanded Containers ({containerFindings.length})
                  </h3>
                  {containerFindings.map((finding) => (
                    <div
                      key={finding.path}
                      className="p-4 rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
                    >
                      <div className="flex items-start gap-3">
                        <Folder className="w-5 h-5 text-[hsl(var(--launcher-accent-primary))] flex-shrink-0 mt-0.5" />
                        <div className="min-w-0">
                          <p className="text-sm font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                            {finding.path}
                          </p>
                          <p className="text-xs text-[hsl(var(--launcher-text-muted))] mt-1">
                            Expanded into {finding.candidates.length} import candidate{finding.candidates.length === 1 ? '' : 's'}.
                          </p>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}

              {shardedSets.some((set) => !set.complete) && (
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

              {pickleFilesCount > 0 && (
                <div className="p-4 rounded-lg border-l-4 border-[hsl(var(--launcher-accent-error))] bg-[hsl(var(--launcher-accent-error)/0.1)]">
                  <div className="flex items-start gap-3">
                    <AlertTriangle className="w-5 h-5 text-[hsl(var(--launcher-accent-error))] flex-shrink-0 mt-0.5" />
                    <div>
                      <p className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
                        {pickleFilesCount} file{pickleFilesCount > 1 ? 's use' : ' uses'} PyTorch pickle format
                      </p>
                      <p className="text-xs text-[hsl(var(--launcher-text-muted))] mt-1">
                        Pickle files can execute arbitrary code. Check the acknowledgment box for each file to proceed.
                      </p>
                    </div>
                  </div>
                </div>
              )}

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
                            const entry = entries.find((candidate) => candidate.path === filePath);
                            if (!entry) return null;
                            return (
                              <div
                                key={filePath}
                                className="flex items-center gap-2 p-2 rounded bg-[hsl(var(--launcher-bg-secondary))] text-xs text-[hsl(var(--launcher-text-muted))]"
                              >
                                <FileBox className="w-3 h-3" />
                                {entry.filename}
                              </div>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}

              {standaloneEntries.length > 0 && (
                <div className="space-y-2">
                  <h3 className="text-sm font-medium text-[hsl(var(--launcher-text-secondary))]">
                    Import Items ({standaloneEntries.length})
                  </h3>
                  {standaloneEntries.map((entry) => (
                    <div
                      key={entry.path}
                      className="flex items-center gap-3 p-3 rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
                    >
                      <EntryIcon entry={entry} />
                      <div className="flex-1 min-w-0">
                        <p className="text-sm font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                          {entry.filename}
                        </p>
                        <p className="text-xs text-[hsl(var(--launcher-text-muted))] truncate">
                          {entry.path}
                        </p>
                        {entry.containerPath && (
                          <p className="text-xs text-[hsl(var(--launcher-text-muted))] truncate">
                            Expanded from {entry.containerPath}
                          </p>
                        )}
                        <ImportBundleComponents entry={entry} />
                      </div>
                      <EntryBadge entry={entry} />
                      {entry.securityTier === 'pickle' && (
                        <label className="flex items-center gap-2 cursor-pointer">
                          <input
                            type="checkbox"
                            checked={entry.securityAcknowledged}
                            onChange={() => toggleSecurityAck(entry.path)}
                            className="w-4 h-4 rounded border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-control))] text-[hsl(var(--launcher-accent-primary))] focus:ring-[hsl(var(--launcher-accent-primary))]"
                          />
                          <span className="text-xs text-[hsl(var(--launcher-text-muted))]">I understand</span>
                        </label>
                      )}
                      <button
                        onClick={() => removeEntry(entry.path)}
                        className="p-1 rounded text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-accent-error))] hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors"
                        title="Remove from import"
                      >
                        <X className="w-4 h-4" />
                      </button>
                    </div>
                  ))}
                </div>
              )}

              {entries.length === 0 && !classificationError && (
                <div className="flex flex-col items-center justify-center py-12 text-[hsl(var(--launcher-text-muted))]">
                  <FileBox className="w-12 h-12 mb-3 opacity-50" />
                  <p className="text-sm">No importable files or folders selected</p>
                </div>
              )}
            </div>
          )}

          {step === 'lookup' && (
            <div className="space-y-4">
              <div className="flex flex-col items-center justify-center py-8">
                <Loader2 className="w-12 h-12 text-[hsl(var(--launcher-accent-primary))] animate-spin mb-4" />
                <p className="text-sm text-[hsl(var(--launcher-text-secondary))]">
                  Looking up metadata ({lookupProgress.current}/{lookupProgress.total})
                </p>
              </div>

              {nonFileEntries.length > 0 && (
                <div className="space-y-2">
                  <h3 className="text-sm font-medium text-[hsl(var(--launcher-text-secondary))]">
                    Directory Imports ({nonFileEntries.length})
                  </h3>
                  {nonFileEntries.map((entry) => (
                    <ImportLookupCard
                      key={entry.path}
                      entry={entry}
                      expandedMetadata={expandedMetadata}
                      showEmbeddedMetadata={showEmbeddedMetadata}
                      showAllEmbeddedMetadata={showAllEmbeddedMetadata}
                      toggleMetadataExpand={toggleMetadataExpand}
                      toggleMetadataSource={toggleMetadataSource}
                      toggleShowAllEmbeddedMetadata={toggleShowAllEmbeddedMetadata}
                    />
                  ))}
                </div>
              )}

              {fileEntries.length > 0 ? (
                <div className="space-y-2">
                  <h3 className="text-sm font-medium text-[hsl(var(--launcher-text-secondary))]">
                    File Imports ({fileEntries.length})
                  </h3>
                  {fileEntries.map((entry) => (
                    <ImportLookupCard
                      key={entry.path}
                      entry={entry}
                      expandedMetadata={expandedMetadata}
                      showEmbeddedMetadata={showEmbeddedMetadata}
                      showAllEmbeddedMetadata={showAllEmbeddedMetadata}
                      toggleMetadataExpand={toggleMetadataExpand}
                      toggleMetadataSource={toggleMetadataSource}
                      toggleShowAllEmbeddedMetadata={toggleShowAllEmbeddedMetadata}
                    />
                  ))}
                </div>
              ) : (
                <div className="rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)] p-4 text-sm text-[hsl(var(--launcher-text-muted))]">
                  No file metadata lookup is required for the selected directory imports.
                </div>
              )}
            </div>
          )}

          {step === 'importing' && (
            <div className="space-y-4">
              <div className="flex items-center justify-center py-8">
                <Loader2 className="w-12 h-12 text-[hsl(var(--launcher-accent-primary))] animate-spin" />
              </div>
              <div className="space-y-2">
                {entries.map((entry) => (
                  <div
                    key={entry.path}
                    className="flex items-center gap-3 p-3 rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
                  >
                    {entry.status === 'importing' && (
                      <Loader2 className="w-4 h-4 text-[hsl(var(--launcher-accent-primary))] animate-spin flex-shrink-0" />
                    )}
                    {entry.status === 'success' && (
                      <CheckCircle2 className="w-4 h-4 text-[hsl(var(--launcher-accent-success))] flex-shrink-0" />
                    )}
                    {entry.status === 'error' && (
                      <AlertCircle className="w-4 h-4 text-[hsl(var(--launcher-accent-error))] flex-shrink-0" />
                    )}
                    {entry.status === 'pending' && (
                      <div className="w-4 h-4 rounded-full border-2 border-[hsl(var(--launcher-border))] flex-shrink-0" />
                    )}
                    <span className="text-sm text-[hsl(var(--launcher-text-secondary))] truncate flex-1">
                      {entry.filename}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {step === 'complete' && (
            <div className="space-y-4">
              <div className="flex items-center justify-center py-6">
                {failedCount === 0 ? (
                  <div className="flex flex-col items-center">
                    <CheckCircle2 className="w-16 h-16 text-[hsl(var(--launcher-accent-success))] mb-3" />
                    <p className="text-lg font-medium text-[hsl(var(--launcher-text-primary))]">
                      {importedCount} item{importedCount !== 1 ? 's' : ''} imported successfully
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
                      {failedCount} item{failedCount !== 1 ? 's' : ''} could not be imported
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

              <div className="space-y-2">
                {entries.map((entry) => {
                  const trustBadge = getTrustBadge(entry.hfMetadata);
                  return (
                    <div
                      key={entry.path}
                      className="flex items-center gap-3 p-3 rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
                    >
                      {entry.status === 'success' && (
                        <CheckCircle2 className="w-4 h-4 text-[hsl(var(--launcher-accent-success))] flex-shrink-0" />
                      )}
                      {entry.status === 'error' && (
                        <AlertCircle className="w-4 h-4 text-[hsl(var(--launcher-accent-error))] flex-shrink-0" />
                      )}
                      <div className="flex-1 min-w-0">
                        <p className="text-sm text-[hsl(var(--launcher-text-secondary))] truncate">
                          {entry.filename}
                        </p>
                        {entry.error && (
                          <p className="text-xs text-[hsl(var(--launcher-accent-error))] truncate">
                            {entry.error}
                          </p>
                        )}
                      </div>
                      {trustBadge && entry.status === 'success' && (
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

        <div className="flex items-center justify-between px-6 py-4 border-t border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-tertiary)/0.3)]">
          <div className="text-sm text-[hsl(var(--launcher-text-muted))]">
            {step === 'classifying' && 'Inspecting import paths...'}
            {step === 'review' && (
              <>
                {entries.length} import item{entries.length !== 1 ? 's' : ''} selected
                {pickleFilesCount > 0 && ` (${acknowledgedCount}/${pickleFilesCount} acknowledged)`}
                {shardedSets.length > 0 && ` • ${shardedSets.length} sharded set${shardedSets.length > 1 ? 's' : ''}`}
                {containerFindings.length > 0 && ` • ${containerFindings.length} container${containerFindings.length === 1 ? '' : 's'} expanded`}
                {blockedFindings.length > 0 && ` • ${blockedFindings.length} blocked`}
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
                  disabled={!allPickleAcknowledged || entries.length === 0}
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
                Import{invalidFileCount > 0 ? ` (${entries.length - invalidFileCount})` : ''}
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
