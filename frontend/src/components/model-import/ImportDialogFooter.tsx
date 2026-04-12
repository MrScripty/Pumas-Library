import { ChevronRight } from 'lucide-react';
import type { DirectoryReviewFinding, ImportEntryStatus, ImportStep, ShardedSetInfo } from './useModelImportWorkflow';

interface ImportDialogFooterProps {
  acknowledgedCount: number;
  allPickleAcknowledged: boolean;
  blockedFindings: DirectoryReviewFinding[];
  containerFindings: DirectoryReviewFinding[];
  entries: ImportEntryStatus[];
  invalidFileCount: number;
  lookupProgress: { current: number; total: number };
  onClose: () => void;
  onProceedToLookup: () => void;
  onStartImport: () => void;
  pickleFilesCount: number;
  shardedSets: ShardedSetInfo[];
  step: ImportStep;
}

export function ImportDialogFooter({
  acknowledgedCount,
  allPickleAcknowledged,
  blockedFindings,
  containerFindings,
  entries,
  invalidFileCount,
  lookupProgress,
  onClose,
  onProceedToLookup,
  onStartImport,
  pickleFilesCount,
  shardedSets,
  step,
}: ImportDialogFooterProps) {
  return (
    <div className="flex items-center justify-between px-6 py-4 border-t border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-tertiary)/0.3)]">
      <div className="text-sm text-[hsl(var(--launcher-text-muted))]">
        {step === 'classifying' && 'Inspecting import paths...'}
        {step === 'review' && (
          <>
            {entries.length} import item{entries.length !== 1 ? 's' : ''} selected
            {pickleFilesCount > 0 && ` (${acknowledgedCount}/${pickleFilesCount} acknowledged)`}
            {shardedSets.length > 0
              && ` • ${shardedSets.length} sharded set${shardedSets.length > 1 ? 's' : ''}`}
            {containerFindings.length > 0
              && ` • ${containerFindings.length} container${containerFindings.length === 1 ? '' : 's'} expanded`}
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
              onClick={onProceedToLookup}
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
            onClick={onStartImport}
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
  );
}
