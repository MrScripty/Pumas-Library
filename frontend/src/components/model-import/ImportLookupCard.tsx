import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  FileBox,
  Folder,
  Package2,
} from 'lucide-react';
import {
  getTrustBadge,
} from './metadataUtils';
import type { ImportEntryStatus } from './useModelImportWorkflow';
import { ImportBundleComponents } from './ImportBundleComponents';
import { ImportMetadataDetails } from './ImportMetadataDetails';

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

  const isShowingAllEmbedded = showAllEmbeddedMetadata.has(entry.path);
  const canInteract = hasMetadata || canShowEmbedded;

  return (
    <div className="rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]">
      <div
        className={`flex items-center gap-3 p-3 ${canInteract ? 'hover:bg-[hsl(var(--launcher-bg-tertiary)/0.8)]' : ''}`}
      >
        {canInteract ? (
          <button
            type="button"
            className="h-4 w-4 flex-shrink-0 text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[hsl(var(--accent-primary))]"
            aria-expanded={isExpanded}
            aria-label={`${isExpanded ? 'Collapse' : 'Expand'} metadata for ${entry.filename}`}
            onClick={() => toggleMetadataExpand(entry.path)}
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
        <ImportMetadataDetails
          entry={entry}
          isShowingAllEmbedded={isShowingAllEmbedded}
          isShowingEmbedded={isShowingEmbedded}
          onToggleMetadataSource={toggleMetadataSource}
          onToggleShowAllEmbeddedMetadata={toggleShowAllEmbeddedMetadata}
        />
      )}
    </div>
  );
}
