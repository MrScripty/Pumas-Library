import { Loader2 } from 'lucide-react';
import { ImportLookupCard } from './ImportLookupCard';
import type { ImportEntryStatus } from './useModelImportWorkflow';

interface ImportLookupStepProps {
  expandedMetadata: Set<string>;
  fileEntries: ImportEntryStatus[];
  lookupProgress: { current: number; total: number };
  nonFileEntries: ImportEntryStatus[];
  showAllEmbeddedMetadata: Set<string>;
  showEmbeddedMetadata: Set<string>;
  toggleMetadataExpand: (path: string) => void;
  toggleMetadataSource: (path: string) => Promise<void>;
  toggleShowAllEmbeddedMetadata: (path: string) => void;
}

export function ImportLookupStep({
  expandedMetadata,
  fileEntries,
  lookupProgress,
  nonFileEntries,
  showAllEmbeddedMetadata,
  showEmbeddedMetadata,
  toggleMetadataExpand,
  toggleMetadataSource,
  toggleShowAllEmbeddedMetadata,
}: ImportLookupStepProps) {
  return (
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
  );
}
