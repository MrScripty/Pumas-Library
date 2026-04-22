import {
  AlertCircle,
  Cloud,
  FileText,
  Loader2,
  ToggleLeft,
  ToggleRight,
} from 'lucide-react';
import type { HFMetadataLookupResult } from '../../types/api';
import type { ImportEntryStatus } from './modelImportWorkflowTypes';
import {
  constructQuantUrl,
  EXCLUDED_FIELDS,
  formatFieldName,
  formatMetadataValue,
  isHiddenGgufField,
  isPriorityGgufField,
  LINKED_GGUF_FIELDS,
  sortMetadataFields,
} from './metadataUtils';

interface ImportMetadataDetailsProps {
  entry: ImportEntryStatus;
  isShowingAllEmbedded: boolean;
  isShowingEmbedded: boolean;
  onToggleMetadataSource: (path: string) => Promise<void>;
  onToggleShowAllEmbeddedMetadata: (path: string) => void;
}

export function ImportMetadataDetails({
  entry,
  isShowingAllEmbedded,
  isShowingEmbedded,
  onToggleMetadataSource,
  onToggleShowAllEmbeddedMetadata,
}: ImportMetadataDetailsProps) {
  const hasMetadata = Boolean(entry.hfMetadata && entry.metadataStatus === 'found');
  const canShowEmbedded =
    entry.detectedFileType === 'gguf' || entry.detectedFileType === 'safetensors';

  const hfMetadataEntries = hasMetadata
    ? sortMetadataFields(
        Object.keys(entry.hfMetadata!).filter(
          (key) =>
            !EXCLUDED_FIELDS.has(key) &&
            entry.hfMetadata![key as keyof HFMetadataLookupResult] != null &&
            entry.hfMetadata![key as keyof HFMetadataLookupResult] !== ''
        )
      ).map((key) => ({
        key,
        label: formatFieldName(key),
        value: formatMetadataValue(key, entry.hfMetadata![key as keyof HFMetadataLookupResult]),
      }))
    : [];

  const allEmbeddedEntries = entry.embeddedMetadata
    ? Object.entries(entry.embeddedMetadata)
        .filter(([, value]) => value != null && value !== '')
        .map(([key, value]) => ({
          key,
          label: formatFieldName(key),
          value: formatMetadataValue(key, value),
          isPriority: isPriorityGgufField(key),
          isHidden: isHiddenGgufField(key, value),
        }))
    : [];

  const embeddedMetadataEntries = allEmbeddedEntries
    .filter((candidate) => isShowingAllEmbedded || (candidate.isPriority && !candidate.isHidden))
    .sort((left, right) => {
      if (left.isPriority !== right.isPriority) return left.isPriority ? -1 : 1;
      return left.key.localeCompare(right.key);
    });

  const hiddenEmbeddedCount = allEmbeddedEntries.length - embeddedMetadataEntries.length;
  const metadataEntries = isShowingEmbedded ? embeddedMetadataEntries : hfMetadataEntries;

  return (
    <div className="ml-8 border-t border-[hsl(var(--launcher-border)/0.5)] px-3 pb-3 pt-1">
      {canShowEmbedded && (
        <div className="mb-3 flex items-center justify-between border-b border-[hsl(var(--launcher-border)/0.3)] pb-2">
          <span className="text-xs text-[hsl(var(--launcher-text-muted))]">Metadata Source</span>
          <button
            onClick={(event) => {
              event.stopPropagation();
              void onToggleMetadataSource(entry.path);
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
      )}

      {isShowingEmbedded && entry.embeddedMetadataStatus === 'pending' && (
        <div className="flex items-center justify-center py-4">
          <Loader2 className="h-5 w-5 animate-spin text-[hsl(var(--launcher-accent-primary))]" />
          <span className="ml-2 text-xs text-[hsl(var(--launcher-text-muted))]">
            Loading embedded metadata...
          </span>
        </div>
      )}

      {isShowingEmbedded && entry.embeddedMetadataStatus === 'error' && (
        <div className="flex items-center gap-2 py-2 text-xs text-[hsl(var(--launcher-accent-error))]">
          <AlertCircle className="h-4 w-4" />
          Failed to load embedded metadata
        </div>
      )}

      {isShowingEmbedded && entry.embeddedMetadataStatus === 'unsupported' && (
        <div className="flex items-center gap-2 py-2 text-xs text-[hsl(var(--launcher-text-muted))]">
          <AlertCircle className="h-4 w-4" />
          This file format does not support embedded metadata
        </div>
      )}

      {metadataEntries.length > 0 ? (
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
                  <span
                    className="truncate text-[hsl(var(--launcher-text-secondary))]"
                    title={value}
                  >
                    {value}
                  </span>
                )}
              </div>
            );
          })}
        </div>
      ) : (
        !isShowingEmbedded &&
        !hasMetadata && (
          <div className="py-2 text-xs text-[hsl(var(--launcher-text-muted))]">
            No metadata available
          </div>
        )
      )}

      {isShowingEmbedded &&
        entry.embeddedMetadataStatus === 'loaded' &&
        hiddenEmbeddedCount > 0 && (
          <button
            onClick={(event) => {
              event.stopPropagation();
              onToggleShowAllEmbeddedMetadata(entry.path);
            }}
            className="mt-2 text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline"
          >
            {isShowingAllEmbedded
              ? 'Show less'
              : `Show ${hiddenEmbeddedCount} more field${hiddenEmbeddedCount === 1 ? '' : 's'}`}
          </button>
        )}

      {isShowingEmbedded &&
        entry.embeddedMetadataStatus === 'loaded' &&
        allEmbeddedEntries.length === 0 && (
          <div className="py-2 text-xs text-[hsl(var(--launcher-text-muted))]">
            No embedded metadata found in file
          </div>
        )}
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
