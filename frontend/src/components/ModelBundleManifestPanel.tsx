import React from 'react';
import { ChevronDown, ChevronUp } from 'lucide-react';
import type { BundleComponentManifestEntry } from '../types/api';

interface ModelBundleManifestPanelProps {
  componentManifest: BundleComponentManifestEntry[];
  showComponents: boolean;
  onToggle: () => void;
}

function formatBundleComponentState(state: BundleComponentManifestEntry['state']): string {
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

export function ModelBundleManifestPanel({
  componentManifest,
  showComponents,
  onToggle,
}: ModelBundleManifestPanelProps) {
  if (componentManifest.length === 0) {
    return null;
  }

  return (
    <div className="rounded-lg border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-low))]">
      <button
        onClick={onToggle}
        className="flex w-full items-center justify-between px-3 py-2 text-left transition-colors hover:bg-[hsl(var(--surface-mid)/0.35)]"
      >
        <div>
          <div className="text-sm font-medium text-[hsl(var(--text-primary))]">
            Components ({componentManifest.length})
          </div>
          <div className="text-xs text-[hsl(var(--text-muted))]">
            Bundle component list derived from `model_index.json`
          </div>
        </div>
        {showComponents ? (
          <ChevronUp className="h-4 w-4 text-[hsl(var(--text-secondary))]" />
        ) : (
          <ChevronDown className="h-4 w-4 text-[hsl(var(--text-secondary))]" />
        )}
      </button>

      {showComponents && (
        <div className="space-y-2 border-t border-[hsl(var(--border-default))] px-3 py-3">
          {componentManifest.map((component) => (
            <div
              key={component.name}
              className="rounded-md border border-[hsl(var(--border-muted))] bg-[hsl(var(--surface-high)/0.35)] px-3 py-2"
            >
              <div className="flex items-center justify-between gap-3">
                <div className="text-sm font-medium text-[hsl(var(--text-primary))]">
                  {component.name}
                </div>
                <span
                  className={`rounded px-2 py-0.5 text-[10px] uppercase tracking-wide ${
                    component.state === 'present'
                      ? 'bg-[hsl(var(--accent-success)/0.15)] text-[hsl(var(--accent-success))]'
                      : 'bg-[hsl(var(--accent-error)/0.15)] text-[hsl(var(--accent-error))]'
                  }`}
                >
                  {formatBundleComponentState(component.state)}
                </span>
              </div>
              <div className="mt-1 break-all font-mono text-xs text-[hsl(var(--text-secondary))]">
                {component.relative_path}
              </div>
              {(component.class_name || component.source_library) && (
                <div className="mt-1 text-xs text-[hsl(var(--text-muted))]">
                  {[component.class_name, component.source_library].filter(Boolean).join(' · ')}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
