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

function RepoLink({
  repoId,
  stopPropagation = false,
}: {
  repoId: string;
  stopPropagation?: boolean;
}) {
  return (
    <a
      href={`https://huggingface.co/${repoId}`}
      target="_blank"
      rel="noopener noreferrer"
      className="flex items-center gap-1 truncate text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline"
      onClick={stopPropagation ? (event) => event.stopPropagation() : undefined}
    >
      {repoId}
      <ExternalLink className="h-3 w-3 flex-shrink-0" />
    </a>
  );
}

function TrustBadge({ entry }: { entry: ImportEntryStatus }) {
  const trustBadge = getTrustBadge(entry.hfMetadata);
  if (!trustBadge) {
    return null;
  }

  return (
    <span
      className={`flex items-center gap-1 rounded px-2 py-0.5 text-xs font-medium ${trustBadge.className}`}
      title={trustBadge.tooltip}
    >
      <trustBadge.Icon className="h-3 w-3" />
      {trustBadge.text}
    </span>
  );
}

function MetadataStatusBadge({ status }: { status: ImportEntryStatus['metadataStatus'] }) {
  const className = status === 'error'
    ? 'bg-[hsl(var(--launcher-accent-error)/0.2)] text-[hsl(var(--launcher-accent-error))]'
    : status === 'not_found'
      ? 'bg-[hsl(var(--launcher-accent-warning)/0.2)] text-[hsl(var(--launcher-accent-warning))]'
      : 'bg-[hsl(var(--launcher-accent-success)/0.2)] text-[hsl(var(--launcher-accent-success))]';
  const label = status === 'error'
    ? 'Lookup Failed'
    : status === 'not_found'
      ? 'No Match'
      : status === 'found'
        ? 'Matched'
        : 'Ready';

  return (
    <span className={`rounded px-2 py-0.5 text-xs font-medium ${className}`}>
      {label}
    </span>
  );
}

function NonFileLookupCard({ entry }: { entry: ImportEntryStatus }) {
  return (
    <div className="flex items-center gap-3 rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)] p-3">
      <EntryIcon entry={entry} />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm text-[hsl(var(--launcher-text-secondary))]">
          {entry.filename}
        </p>
        {entry.hfMetadata?.repo_id ? (
          <RepoLink repoId={entry.hfMetadata.repo_id} />
        ) : (
          <p className="truncate text-xs text-[hsl(var(--launcher-text-muted))]">
            {entry.kind === 'external_diffusers_bundle'
              ? `Bundle root${entry.pipelineClass ? ` • ${entry.pipelineClass}` : ''}`
              : 'Directory model import'}
          </p>
        )}
        <ImportBundleComponents entry={entry} />
      </div>
      <TrustBadge entry={entry} />
      {!getTrustBadge(entry.hfMetadata) && <MetadataStatusBadge status={entry.metadataStatus} />}
    </div>
  );
}

function MetadataExpandButton({
  entry,
  isExpanded,
  onToggle,
}: {
  entry: ImportEntryStatus;
  isExpanded: boolean;
  onToggle: (path: string) => void;
}) {
  return (
    <button
      type="button"
      className="h-4 w-4 flex-shrink-0 text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[hsl(var(--accent-primary))]"
      aria-expanded={isExpanded}
      aria-label={`${isExpanded ? 'Collapse' : 'Expand'} metadata for ${entry.filename}`}
      onClick={() => onToggle(entry.path)}
    >
      {isExpanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
    </button>
  );
}

function MetadataStatusIcon({ status }: { status: ImportEntryStatus['metadataStatus'] }) {
  if (status === 'pending') {
    return <div className="h-4 w-4 flex-shrink-0 rounded-full border-2 border-[hsl(var(--launcher-border))]" />;
  }
  if (status === 'found') {
    return <CheckCircle2 className="h-4 w-4 flex-shrink-0 text-[hsl(var(--launcher-accent-success))]" />;
  }
  if (status === 'not_found') {
    return <AlertCircle className="h-4 w-4 flex-shrink-0 text-[hsl(var(--launcher-accent-warning))]" />;
  }
  if (status === 'error') {
    return <AlertCircle className="h-4 w-4 flex-shrink-0 text-[hsl(var(--launcher-accent-error))]" />;
  }
  return null;
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
    return <NonFileLookupCard entry={entry} />;
  }

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
          <MetadataExpandButton
            entry={entry}
            isExpanded={isExpanded}
            onToggle={toggleMetadataExpand}
          />
        ) : (
          <div className="h-4 w-4 flex-shrink-0" />
        )}

        <MetadataStatusIcon status={entry.metadataStatus} />

        <div className="min-w-0 flex-1">
          <p className="truncate text-sm text-[hsl(var(--launcher-text-secondary))]">
            {entry.filename}
          </p>
          {entry.hfMetadata?.repo_id ? (
            <RepoLink repoId={entry.hfMetadata.repo_id} stopPropagation />
          ) : (
            <p className="truncate text-xs text-[hsl(var(--launcher-text-muted))]">{entry.path}</p>
          )}
        </div>

        <TrustBadge entry={entry} />
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
