import { CheckCircle, AlertCircle, AlertTriangle, RefreshCw, Trash2 } from 'lucide-react';
import type { LinkHealthResponse } from '../types/api';

interface LinkHealthDetailsProps {
  activeVersion?: string | null;
  hasBrokenLinks: boolean;
  hasIssues: boolean;
  hasOrphanedLinks: boolean;
  health: LinkHealthResponse;
  isCleaning: boolean;
  isLoading: boolean;
  isRemovingOrphans: boolean;
  lastAction: string | null;
  onCleanBrokenLinks: () => void;
  onRefresh: () => void;
  onRemoveOrphans: () => void;
}

export function LinkHealthDetails({
  activeVersion,
  hasBrokenLinks,
  hasIssues,
  hasOrphanedLinks,
  health,
  isCleaning,
  isLoading,
  isRemovingOrphans,
  lastAction,
  onCleanBrokenLinks,
  onRefresh,
  onRemoveOrphans,
}: LinkHealthDetailsProps) {
  return (
    <div className="px-4 pb-4 space-y-3">
      <div className="grid grid-cols-3 gap-2 text-center">
        <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
          <div className="text-lg font-semibold text-[hsl(var(--launcher-text-primary))]">
            {health.healthy_links}
          </div>
          <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Healthy</div>
        </div>
        <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
          <div className={`text-lg font-semibold ${hasBrokenLinks ? 'text-[hsl(var(--accent-error))]' : 'text-[hsl(var(--launcher-text-primary))]'}`}>
            {health.broken_links.length}
          </div>
          <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Broken</div>
        </div>
        <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
          <div className={`text-lg font-semibold ${hasOrphanedLinks ? 'text-[hsl(var(--accent-warning))]' : 'text-[hsl(var(--launcher-text-primary))]'}`}>
            {health.orphaned_links.length}
          </div>
          <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Orphaned</div>
        </div>
      </div>

      {hasBrokenLinks && (
        <div className="space-y-2">
          <div className="text-xs font-medium text-[hsl(var(--accent-error))] flex items-center gap-1">
            <AlertCircle className="w-3 h-3" />
            Broken Links
          </div>
          <div className="max-h-32 overflow-y-auto space-y-1">
            {health.broken_links.map((path: string, index: number) => (
              <div
                key={`${path}-${index}`}
                className="text-xs p-2 bg-[hsl(var(--accent-error)/0.1)] rounded border border-[hsl(var(--accent-error)/0.2)]"
              >
                <div className="font-mono truncate text-[hsl(var(--launcher-text-primary))]">
                  {path}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {hasOrphanedLinks && (
        <div className="space-y-2">
          <div className="text-xs font-medium text-[hsl(var(--accent-warning))] flex items-center gap-1">
            <AlertTriangle className="w-3 h-3" />
            Orphaned Links
          </div>
          <div className="max-h-32 overflow-y-auto space-y-1">
            {health.orphaned_links.map((path: string, index: number) => (
              <div
                key={index}
                className="text-xs p-2 bg-[hsl(var(--accent-warning)/0.1)] rounded border border-[hsl(var(--accent-warning)/0.2)] font-mono truncate"
              >
                {path}
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="flex gap-2 pt-2">
        <button
          onClick={onRefresh}
          disabled={isLoading}
          className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--launcher-bg-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] rounded transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-3 h-3 ${isLoading ? 'animate-spin' : ''}`} />
          Refresh
        </button>
        {hasBrokenLinks && (
          <button
            onClick={onCleanBrokenLinks}
            disabled={isCleaning}
            className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--accent-error)/0.2)] hover:bg-[hsl(var(--accent-error)/0.3)] text-[hsl(var(--accent-error))] rounded transition-colors disabled:opacity-50"
          >
            <Trash2 className={`w-3 h-3 ${isCleaning ? 'animate-spin' : ''}`} />
            Clean Broken
          </button>
        )}
        {hasOrphanedLinks && activeVersion && (
          <button
            onClick={onRemoveOrphans}
            disabled={isRemovingOrphans}
            className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--accent-warning)/0.2)] hover:bg-[hsl(var(--accent-warning)/0.3)] text-[hsl(var(--accent-warning))] rounded transition-colors disabled:opacity-50"
          >
            <Trash2 className={`w-3 h-3 ${isRemovingOrphans ? 'animate-spin' : ''}`} />
            Remove Orphans
          </button>
        )}
      </div>

      {lastAction && (
        <div className="text-xs text-center text-[hsl(var(--launcher-text-secondary))] py-1">
          {lastAction}
        </div>
      )}

      {!hasIssues && (
        <div className="text-xs text-center text-[hsl(var(--accent-success))] py-2 flex items-center justify-center gap-2">
          <CheckCircle className="w-4 h-4" />
          All symlinks are healthy
        </div>
      )}
    </div>
  );
}
