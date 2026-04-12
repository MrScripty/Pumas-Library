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
  Loader2,
} from 'lucide-react';
import { ImportDialogFooter } from './model-import/ImportDialogFooter';
import { ImportLookupStep } from './model-import/ImportLookupStep';
import { ImportResultStep } from './model-import/ImportResultStep';
import { ImportReviewStep } from './model-import/ImportReviewStep';
import { useModelImportWorkflow } from './model-import/useModelImportWorkflow';

interface ModelImportDialogProps {
  /** Paths to import */
  importPaths: string[];
  /** Callback when dialog is closed */
  onClose: () => void;
  /** Callback when import completes successfully */
  onImportComplete: () => void;
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
            <ImportReviewStep
              blockedFindings={blockedFindings}
              classificationError={classificationError}
              containerFindings={containerFindings}
              entries={entries}
              pickleFilesCount={pickleFilesCount}
              removeEntry={removeEntry}
              shardedSets={shardedSets}
              standaloneEntries={standaloneEntries}
              toggleSecurityAck={toggleSecurityAck}
              toggleShardedSet={toggleShardedSet}
            />
          )}

          {step === 'lookup' && (
            <ImportLookupStep
              expandedMetadata={expandedMetadata}
              fileEntries={fileEntries}
              lookupProgress={lookupProgress}
              nonFileEntries={nonFileEntries}
              showAllEmbeddedMetadata={showAllEmbeddedMetadata}
              showEmbeddedMetadata={showEmbeddedMetadata}
              toggleMetadataExpand={toggleMetadataExpand}
              toggleMetadataSource={toggleMetadataSource}
              toggleShowAllEmbeddedMetadata={toggleShowAllEmbeddedMetadata}
            />
          )}

          {step === 'importing' && (
            <ImportResultStep
              entries={entries}
              failedCount={failedCount}
              importedCount={importedCount}
              mode="importing"
              verifiedCount={verifiedCount}
            />
          )}

          {step === 'complete' && (
            <ImportResultStep
              entries={entries}
              failedCount={failedCount}
              importedCount={importedCount}
              mode="complete"
              verifiedCount={verifiedCount}
            />
          )}
        </div>

        <ImportDialogFooter
          acknowledgedCount={acknowledgedCount}
          allPickleAcknowledged={allPickleAcknowledged}
          blockedFindings={blockedFindings}
          containerFindings={containerFindings}
          entries={entries}
          invalidFileCount={invalidFileCount}
          lookupProgress={lookupProgress}
          onClose={onClose}
          onProceedToLookup={proceedToLookup}
          onStartImport={startImport}
          pickleFilesCount={pickleFilesCount}
          shardedSets={shardedSets}
          step={step}
        />
      </div>
    </div>
  );
};
