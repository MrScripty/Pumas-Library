import {
  AlertTriangle,
  CheckCircle2,
  ChevronRight,
  FileBox,
  Folder,
} from 'lucide-react';
import type { ImportEntryStatus, ShardedSetInfo } from './useModelImportWorkflow';

interface ImportShardedSetsSectionProps {
  entries: ImportEntryStatus[];
  shardedSets: ShardedSetInfo[];
  toggleShardedSet: (key: string) => void;
}

export function ImportShardedSetsSection({
  entries,
  shardedSets,
  toggleShardedSet,
}: ImportShardedSetsSectionProps) {
  if (shardedSets.length === 0) {
    return null;
  }

  return (
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
            <ChevronRight
              className={`w-4 h-4 text-[hsl(var(--launcher-text-muted))] transition-transform ${set.expanded ? 'rotate-90' : ''}`}
            />
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
  );
}
