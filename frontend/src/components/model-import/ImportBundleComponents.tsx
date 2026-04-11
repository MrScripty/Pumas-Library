import React from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import type { ImportEntryStatus } from './useModelImportWorkflow';

interface ImportBundleComponentsProps {
  entry: ImportEntryStatus;
}

function formatBundleComponentState(
  state: NonNullable<ImportEntryStatus['componentManifest']>[number]['state']
): string {
  switch (state) {
    case 'present':
      return 'Present';
    case 'missing':
      return 'Missing';
    case 'unreadable':
      return 'Unreadable';
    case 'path_escape':
      return 'Invalid Path';
    default:
      return state;
  }
}

export function ImportBundleComponents({ entry }: ImportBundleComponentsProps) {
  const [expanded, setExpanded] = React.useState(false);

  if (entry.kind !== 'external_diffusers_bundle' || !entry.componentManifest?.length) {
    return null;
  }

  return (
    <div className="mt-2">
      <button
        onClick={() => setExpanded((prev) => !prev)}
        className="flex items-center gap-2 text-xs text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))]"
      >
        {expanded ? (
          <ChevronDown className="h-3 w-3" />
        ) : (
          <ChevronRight className="h-3 w-3" />
        )}
        Components ({entry.componentManifest.length})
      </button>
      {expanded && (
        <div className="mt-2 space-y-1 rounded-md bg-[hsl(var(--launcher-bg-secondary))] p-2">
          {entry.componentManifest.map((component) => (
            <div
              key={`${entry.path}:${component.name}`}
              className="flex items-start justify-between gap-3 text-xs"
            >
              <div className="min-w-0">
                <div className="text-[hsl(var(--launcher-text-secondary))]">{component.name}</div>
                {component.relative_path !== component.name && (
                  <div className="break-all font-mono text-[hsl(var(--launcher-text-muted))]">
                    {component.relative_path}
                  </div>
                )}
              </div>
              <span
                className={`shrink-0 rounded px-2 py-0.5 ${
                  component.state === 'present'
                    ? 'bg-[hsl(var(--launcher-accent-success)/0.15)] text-[hsl(var(--launcher-accent-success))]'
                    : 'bg-[hsl(var(--launcher-accent-warning)/0.15)] text-[hsl(var(--launcher-accent-warning))]'
                }`}
              >
                {formatBundleComponentState(component.state)}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
