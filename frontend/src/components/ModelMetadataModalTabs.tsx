import { ActivitySquare, Database, FileText, PencilLine, Play, Settings } from 'lucide-react';
import type { MetadataSource } from './ModelMetadataFieldConfig';

type ModelMetadataModalTabsProps = {
  activeSource: MetadataSource;
  embeddedFileType: string | null;
  embeddedMetadata: Record<string, unknown> | null;
  storedMetadata: Record<string, unknown> | null;
  onActiveSourceChange: (source: MetadataSource) => void;
};

function tabClassName(active: boolean): string {
  return `flex items-center gap-2 px-3 py-1.5 rounded text-sm ${
    active
      ? 'bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--text-primary))]'
      : 'bg-[hsl(var(--surface-high))] hover:bg-[hsl(var(--surface-mid))] text-[hsl(var(--text-secondary))]'
  }`;
}

export function ModelMetadataModalTabs({
  activeSource,
  embeddedFileType,
  embeddedMetadata,
  storedMetadata,
  onActiveSourceChange,
}: ModelMetadataModalTabsProps) {
  return (
    <div className="flex flex-wrap gap-2">
      <button
        onClick={() => onActiveSourceChange('embedded')}
        className={tabClassName(activeSource === 'embedded')}
        disabled={!embeddedMetadata}
      >
        <FileText className="w-4 h-4" />
        Embedded ({embeddedFileType || 'N/A'})
      </button>
      <button
        onClick={() => onActiveSourceChange('stored')}
        className={tabClassName(activeSource === 'stored')}
        disabled={!storedMetadata}
      >
        <Database className="w-4 h-4" />
        Stored
      </button>
      <button
        onClick={() => onActiveSourceChange('inference')}
        className={tabClassName(activeSource === 'inference')}
      >
        <Settings className="w-4 h-4" />
        Inference
      </button>
      <button
        onClick={() => onActiveSourceChange('execution')}
        className={tabClassName(activeSource === 'execution')}
      >
        <ActivitySquare className="w-4 h-4" />
        Execution Facts
      </button>
      <button
        onClick={() => onActiveSourceChange('runtime')}
        className={tabClassName(activeSource === 'runtime')}
      >
        <Play className="w-4 h-4" />
        Serving
      </button>
      <button
        onClick={() => onActiveSourceChange('notes')}
        className={tabClassName(activeSource === 'notes')}
      >
        <PencilLine className="w-4 h-4" />
        Notes
      </button>
    </div>
  );
}
