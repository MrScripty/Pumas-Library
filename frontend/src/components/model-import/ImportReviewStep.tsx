import {
  AlertCircle,
  AlertTriangle,
  FileBox,
  Folder,
  Package2,
  Unlink,
  X,
} from 'lucide-react';
import { getSecurityBadge } from './metadataUtils';
import { ImportBundleComponents } from './ImportBundleComponents';
import { ImportShardedSetsSection } from './ImportShardedSetsSection';
import type {
  DirectoryReviewFinding,
  ImportEntryStatus,
  ShardedSetInfo,
} from './useModelImportWorkflow';

function EntryIcon({ entry }: { entry: ImportEntryStatus }) {
  if (entry.kind === 'external_diffusers_bundle') {
    return (
      <Package2 className="w-5 h-5 text-[hsl(var(--launcher-accent-primary))] flex-shrink-0" />
    );
  }
  if (entry.kind === 'directory_model') {
    return (
      <Folder className="w-5 h-5 text-[hsl(var(--launcher-accent-primary))] flex-shrink-0" />
    );
  }
  return (
    <FileBox className="w-5 h-5 text-[hsl(var(--launcher-text-muted))] flex-shrink-0" />
  );
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
    <span
      className={`px-2 py-0.5 rounded text-xs font-medium flex items-center gap-1 ${badge.className}`}
    >
      <BadgeIcon className="w-3 h-3" />
      {badge.text}
    </span>
  );
}

function ImportPackageEvidence({ entry }: { entry: ImportEntryStatus }) {
  const facts = [
    entry.detectedFileType ? ['Format', entry.detectedFileType] : null,
    entry.bundleFormat ? ['Bundle', entry.bundleFormat] : null,
    entry.pipelineClass ? ['Pipeline', entry.pipelineClass] : null,
    entry.componentManifest?.length
      ? ['Components', String(entry.componentManifest.length)]
      : null,
    entry.hfMetadata?.repo_id ? ['HF', entry.hfMetadata.repo_id] : null,
  ].filter((fact): fact is [string, string] => fact !== null);

  if (facts.length === 0) {
    return null;
  }

  return (
    <div className="mt-2 flex flex-wrap gap-1.5">
      {facts.map(([label, value]) => (
        <span
          key={`${label}:${value}`}
          className="inline-flex max-w-full items-center gap-1 rounded border border-[hsl(var(--launcher-border))] px-1.5 py-0.5 text-[10px] text-[hsl(var(--launcher-text-muted))]"
          title={`${label}: ${value}`}
        >
          <span className="font-medium text-[hsl(var(--launcher-text-secondary))]">{label}</span>
          <span className="truncate">{value}</span>
        </span>
      ))}
    </div>
  );
}

interface ImportReviewStepProps {
  blockedFindings: DirectoryReviewFinding[];
  classificationError: string | null;
  containerFindings: DirectoryReviewFinding[];
  entries: ImportEntryStatus[];
  pickleFilesCount: number;
  removeEntry: (path: string) => void;
  shardedSets: ShardedSetInfo[];
  standaloneEntries: ImportEntryStatus[];
  toggleSecurityAck: (path: string) => void;
  toggleShardedSet: (key: string) => void;
}

export function ImportReviewStep({
  blockedFindings,
  classificationError,
  containerFindings,
  entries,
  pickleFilesCount,
  removeEntry,
  shardedSets,
  standaloneEntries,
  toggleSecurityAck,
  toggleShardedSet,
}: ImportReviewStepProps) {
  return (
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
                    <p
                      key={reason}
                      className="text-xs text-[hsl(var(--launcher-text-muted))] mt-1"
                    >
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
                    Expanded into {finding.candidates.length} import candidate
                    {finding.candidates.length === 1 ? '' : 's'}.
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
                {pickleFilesCount} file{pickleFilesCount > 1 ? 's use' : ' uses'} PyTorch pickle
                format
              </p>
              <p className="text-xs text-[hsl(var(--launcher-text-muted))] mt-1">
                Pickle files can execute arbitrary code. Check the acknowledgment box for each
                file to proceed.
              </p>
            </div>
          </div>
        </div>
      )}

      <ImportShardedSetsSection
        entries={entries}
        shardedSets={shardedSets}
        toggleShardedSet={toggleShardedSet}
      />

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
                <ImportPackageEvidence entry={entry} />
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
                  <span className="text-xs text-[hsl(var(--launcher-text-muted))]">
                    I understand
                  </span>
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
  );
}
