import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Cloud,
  ExternalLink,
  FileBox,
  FileText,
  Folder,
  Loader2,
  Package2,
  ToggleLeft,
  ToggleRight,
} from 'lucide-react';
import type { HFMetadataLookupResult } from '../../types/api';
import {
  constructQuantUrl,
  EXCLUDED_FIELDS,
  formatFieldName,
  formatMetadataValue,
  getTrustBadge,
  isHiddenGgufField,
  isPriorityGgufField,
  LINKED_GGUF_FIELDS,
  sortMetadataFields,
} from './metadataUtils';
import type { ImportEntryStatus } from './useModelImportWorkflow';
import { ImportBundleComponents } from './ImportBundleComponents';

interface ImportLookupCardProps {
  entry: ImportEntryStatus;
  expandedMetadata: Set<string>;
  showEmbeddedMetadata: Set<string>;
  showAllEmbeddedMetadata: Set<string>;
  toggleMetadataExpand: (path: string) => void;
  toggleMetadataSource: (path: string) => Promise<void>;
  toggleShowAllEmbeddedMetadata: (path: string) => void;
}

function EntryIcon({ entry }: { entry: ImportEntryStatus }) {
  if (entry.kind === 'external_diffusers_bundle') {
    return (
      <Package2 className="h-5 w-5 flex-shrink-0 text-[hsl(var(--launcher-accent-primary))]" />
    );
  }
  if (entry.kind === 'directory_model') {
    return <Folder className="h-5 w-5 flex-shrink-0 text-[hsl(var(--launcher-accent-primary))]" />;
  }
  return <FileBox className="h-5 w-5 flex-shrink-0 text-[hsl(var(--launcher-text-muted))]" />;
}

export function ImportLookupCard({
  entry,
  expandedMetadata,
  showEmbeddedMetadata,
  showAllEmbeddedMetadata,
  toggleMetadataExpand,
  toggleMetadataSource,
  toggleShowAllEmbeddedMetadata,
}: ImportLookupCardProps) {
  if (entry.kind !== 'single_file') {
    const trustBadge = getTrustBadge(entry.hfMetadata);
    return (
      <div className="flex items-center gap-3 rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)] p-3">
        <EntryIcon entry={entry} />
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm text-[hsl(var(--launcher-text-secondary))]">
            {entry.filename}
          </p>
          {entry.hfMetadata?.repo_id ? (
            <a
              href={`https://huggingface.co/${entry.hfMetadata.repo_id}`}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-1 truncate text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline"
            >
              {entry.hfMetadata.repo_id}
              <ExternalLink className="h-3 w-3 flex-shrink-0" />
            </a>
          ) : (
            <p className="truncate text-xs text-[hsl(var(--launcher-text-muted))]">
              {entry.kind === 'external_diffusers_bundle'
                ? `Bundle root${entry.pipelineClass ? ` • ${entry.pipelineClass}` : ''}`
                : 'Directory model import'}
            </p>
          )}
          <ImportBundleComponents entry={entry} />
        </div>
        {trustBadge ? (
          <span
            className={`flex items-center gap-1 rounded px-2 py-0.5 text-xs font-medium ${trustBadge.className}`}
            title={trustBadge.tooltip}
          >
            <trustBadge.Icon className="h-3 w-3" />
            {trustBadge.text}
          </span>
        ) : (
          <span
            className={`rounded px-2 py-0.5 text-xs font-medium ${
              entry.metadataStatus === 'error'
                ? 'bg-[hsl(var(--launcher-accent-error)/0.2)] text-[hsl(var(--launcher-accent-error))]'
                : entry.metadataStatus === 'not_found'
                  ? 'bg-[hsl(var(--launcher-accent-warning)/0.2)] text-[hsl(var(--launcher-accent-warning))]'
                  : 'bg-[hsl(var(--launcher-accent-success)/0.2)] text-[hsl(var(--launcher-accent-success))]'
            }`}
          >
            {entry.metadataStatus === 'error'
              ? 'Lookup Failed'
              : entry.metadataStatus === 'not_found'
                ? 'No Match'
                : entry.metadataStatus === 'found'
                  ? 'Matched'
                  : 'Ready'}
          </span>
        )}
      </div>
    );
  }

  const trustBadge = getTrustBadge(entry.hfMetadata);
  const isExpanded = expandedMetadata.has(entry.path);
  const hasMetadata = entry.hfMetadata && entry.metadataStatus === 'found';
  const isShowingEmbedded = showEmbeddedMetadata.has(entry.path);
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

  const isShowingAllEmbedded = showAllEmbeddedMetadata.has(entry.path);
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
  const canInteract = hasMetadata || canShowEmbedded;

  return (
    <div className="rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]">
      <div
        className={`flex items-center gap-3 p-3 ${canInteract ? 'cursor-pointer hover:bg-[hsl(var(--launcher-bg-tertiary)/0.8)]' : ''}`}
        role={canInteract ? 'button' : undefined}
        tabIndex={canInteract ? 0 : undefined}
        aria-expanded={canInteract ? isExpanded : undefined}
        onClick={canInteract ? () => toggleMetadataExpand(entry.path) : undefined}
        onKeyDown={
          canInteract
            ? (event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  toggleMetadataExpand(entry.path);
                }
              }
            : undefined
        }
      >
        {canInteract ? (
          <button
            className="h-4 w-4 flex-shrink-0 text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))]"
            onClick={(event) => {
              event.stopPropagation();
              toggleMetadataExpand(entry.path);
            }}
          >
            {isExpanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
          </button>
        ) : (
          <div className="h-4 w-4 flex-shrink-0" />
        )}

        {entry.metadataStatus === 'pending' && (
          <div className="h-4 w-4 flex-shrink-0 rounded-full border-2 border-[hsl(var(--launcher-border))]" />
        )}
        {entry.metadataStatus === 'found' && (
          <CheckCircle2 className="h-4 w-4 flex-shrink-0 text-[hsl(var(--launcher-accent-success))]" />
        )}
        {entry.metadataStatus === 'not_found' && (
          <AlertCircle className="h-4 w-4 flex-shrink-0 text-[hsl(var(--launcher-accent-warning))]" />
        )}
        {entry.metadataStatus === 'error' && (
          <AlertCircle className="h-4 w-4 flex-shrink-0 text-[hsl(var(--launcher-accent-error))]" />
        )}

        <div className="min-w-0 flex-1">
          <p className="truncate text-sm text-[hsl(var(--launcher-text-secondary))]">
            {entry.filename}
          </p>
          {entry.hfMetadata?.repo_id ? (
            <a
              href={`https://huggingface.co/${entry.hfMetadata.repo_id}`}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-1 truncate text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline"
              onClick={(event) => event.stopPropagation()}
            >
              {entry.hfMetadata.repo_id}
              <ExternalLink className="h-3 w-3 flex-shrink-0" />
            </a>
          ) : (
            <p className="truncate text-xs text-[hsl(var(--launcher-text-muted))]">{entry.path}</p>
          )}
        </div>

        {trustBadge && (
          <span
            className={`flex items-center gap-1 rounded px-2 py-0.5 text-xs font-medium ${trustBadge.className}`}
            title={trustBadge.tooltip}
          >
            <trustBadge.Icon className="h-3 w-3" />
            {trustBadge.text}
          </span>
        )}
      </div>

      {isExpanded && (
        <div className="ml-8 border-t border-[hsl(var(--launcher-border)/0.5)] px-3 pb-3 pt-1">
          {canShowEmbedded && (
            <div className="mb-3 flex items-center justify-between border-b border-[hsl(var(--launcher-border)/0.3)] pb-2">
              <span className="text-xs text-[hsl(var(--launcher-text-muted))]">Metadata Source</span>
              <button
                onClick={(event) => {
                  event.stopPropagation();
                  void toggleMetadataSource(entry.path);
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
                const lowerKey = key.toLowerCase();
                let linkedUrl = '';

                if (isShowingEmbedded && entry.embeddedMetadata) {
                  const linkedUrlKey = LINKED_GGUF_FIELDS[lowerKey];
                  if (linkedUrlKey) {
                    linkedUrl = String(entry.embeddedMetadata[linkedUrlKey] ?? '');
                  } else if (lowerKey === 'general.name') {
                    linkedUrl = constructQuantUrl(entry.embeddedMetadata) ?? '';
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
                  toggleShowAllEmbeddedMetadata(entry.path);
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
      )}
    </div>
  );
}
