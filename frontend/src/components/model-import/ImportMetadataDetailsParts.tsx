import {
  AlertCircle,
  Cloud,
  FileText,
  Loader2,
  ToggleLeft,
  ToggleRight,
} from 'lucide-react';
import type { ImportEntryStatus } from './modelImportWorkflowTypes';
import {
  constructQuantUrl,
  LINKED_GGUF_FIELDS,
} from './metadataUtils';
import type { MetadataEntry } from './ImportMetadataDetailsState';

export function MetadataSourceToggle({
  isShowingEmbedded,
  path,
  onToggleMetadataSource,
}: {
  isShowingEmbedded: boolean;
  path: string;
  onToggleMetadataSource: (path: string) => Promise<void>;
}) {
  return (
    <div className="mb-3 flex items-center justify-between border-b border-[hsl(var(--launcher-border)/0.3)] pb-2">
      <span className="text-xs text-[hsl(var(--launcher-text-muted))]">Metadata Source</span>
      <button
        onClick={(event) => {
          event.stopPropagation();
          void onToggleMetadataSource(path);
        }}
        className="flex items-center gap-2 rounded-md px-2 py-1 text-xs font-medium transition-colors hover:bg-[hsl(var(--launcher-bg-tertiary))]"
      >
        {isShowingEmbedded ? (
          <>
            <FileText className="h-3 w-3 text-[hsl(var(--launcher-accent-warning))]" />
            <span className="text-[hsl(var(--launcher-accent-warning))]">Embedded</span>
            <ToggleRight className="h-4 w-4 text-[hsl(var(--launcher-accent-warning))]" />
          </>
        ) : (
          <>
            <Cloud className="h-3 w-3 text-[hsl(var(--launcher-accent-primary))]" />
            <span className="text-[hsl(var(--launcher-accent-primary))]">HuggingFace</span>
            <ToggleLeft className="h-4 w-4 text-[hsl(var(--launcher-accent-primary))]" />
          </>
        )}
      </button>
    </div>
  );
}

export function EmbeddedMetadataStatusMessage({
  isShowingEmbedded,
  status,
}: {
  isShowingEmbedded: boolean;
  status: ImportEntryStatus['embeddedMetadataStatus'];
}) {
  if (!isShowingEmbedded || status === 'loaded') {
    return null;
  }

  if (status === 'pending') {
    return (
      <div className="flex items-center justify-center py-4">
        <Loader2 className="h-5 w-5 animate-spin text-[hsl(var(--launcher-accent-primary))]" />
        <span className="ml-2 text-xs text-[hsl(var(--launcher-text-muted))]">
          Loading embedded metadata...
        </span>
      </div>
    );
  }

  if (status === 'error') {
    return (
      <div className="flex items-center gap-2 py-2 text-xs text-[hsl(var(--launcher-accent-error))]">
        <AlertCircle className="h-4 w-4" />
        Failed to load embedded metadata
      </div>
    );
  }

  if (status === 'unsupported') {
    return (
      <div className="flex items-center gap-2 py-2 text-xs text-[hsl(var(--launcher-text-muted))]">
        <AlertCircle className="h-4 w-4" />
        This file format does not support embedded metadata
      </div>
    );
  }

  return null;
}

export function MetadataEntriesGrid({
  entry,
  isShowingEmbedded,
  metadataEntries,
}: {
  entry: ImportEntryStatus;
  isShowingEmbedded: boolean;
  metadataEntries: MetadataEntry[];
}) {
  if (metadataEntries.length === 0) {
    return null;
  }

  return (
    <div className="grid max-h-48 grid-cols-2 gap-x-4 gap-y-1 overflow-y-auto text-xs">
      {metadataEntries.map(({ key, label, value }) => {
        const linkedUrl = resolveEmbeddedMetadataUrl(entry, isShowingEmbedded, key);

        return (
          <div key={key} className="contents">
            <span className="text-[hsl(var(--launcher-text-muted))]">{label}</span>
            {linkedUrl ? (
              <a
                href={linkedUrl}
                target="_blank"
                rel="noopener noreferrer"
                onClick={(event) => event.stopPropagation()}
                className="truncate text-[hsl(var(--launcher-accent-primary))] hover:underline"
                title={`${value} (${linkedUrl})`}
              >
                {value}
              </a>
            ) : (
              <span className="truncate text-[hsl(var(--launcher-text-secondary))]" title={value}>
                {value}
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
}

export function NoHfMetadataMessage({
  hasMetadata,
  isShowingEmbedded,
  metadataCount,
}: {
  hasMetadata: boolean;
  isShowingEmbedded: boolean;
  metadataCount: number;
}) {
  if (isShowingEmbedded || hasMetadata || metadataCount > 0) {
    return null;
  }

  return (
    <div className="py-2 text-xs text-[hsl(var(--launcher-text-muted))]">
      No metadata available
    </div>
  );
}

export function EmbeddedMetadataDisclosure({
  hiddenEmbeddedCount,
  isShowingAllEmbedded,
  isShowingEmbedded,
  path,
  status,
  onToggleShowAllEmbeddedMetadata,
}: {
  hiddenEmbeddedCount: number;
  isShowingAllEmbedded: boolean;
  isShowingEmbedded: boolean;
  path: string;
  status: ImportEntryStatus['embeddedMetadataStatus'];
  onToggleShowAllEmbeddedMetadata: (path: string) => void;
}) {
  if (!isShowingEmbedded || status !== 'loaded' || hiddenEmbeddedCount <= 0) {
    return null;
  }

  return (
    <button
      onClick={(event) => {
        event.stopPropagation();
        onToggleShowAllEmbeddedMetadata(path);
      }}
      className="mt-2 text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline"
    >
      {isShowingAllEmbedded
        ? 'Show less'
        : `Show ${hiddenEmbeddedCount} more field${hiddenEmbeddedCount === 1 ? '' : 's'}`}
    </button>
  );
}

export function NoEmbeddedMetadataMessage({
  allEmbeddedCount,
  isShowingEmbedded,
  status,
}: {
  allEmbeddedCount: number;
  isShowingEmbedded: boolean;
  status: ImportEntryStatus['embeddedMetadataStatus'];
}) {
  if (!isShowingEmbedded || status !== 'loaded' || allEmbeddedCount > 0) {
    return null;
  }

  return (
    <div className="py-2 text-xs text-[hsl(var(--launcher-text-muted))]">
      No embedded metadata found in file
    </div>
  );
}

function resolveEmbeddedMetadataUrl(
  entry: ImportEntryStatus,
  isShowingEmbedded: boolean,
  key: string
): string {
  if (!isShowingEmbedded || !entry.embeddedMetadata) {
    return '';
  }

  const lowerKey = key.toLowerCase();
  const linkedUrlKey = LINKED_GGUF_FIELDS[lowerKey];
  if (linkedUrlKey) {
    return String(entry.embeddedMetadata[linkedUrlKey] ?? '');
  }
  if (lowerKey === 'general.name') {
    return constructQuantUrl(entry.embeddedMetadata) ?? '';
  }
  return '';
}
