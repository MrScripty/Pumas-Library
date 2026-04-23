import {
  AlertTriangle,
  CheckCircle,
  ChevronDown,
  ChevronUp,
  FolderSymlink,
  RefreshCw,
  XCircle,
  type LucideIcon,
} from 'lucide-react';
import type { MappingPreviewStatus } from './MappingPreviewDetailsTypes';
import type { MappingPreviewResponse } from './MappingPreviewTypes';
import type { MappingPreviewCounts } from './MappingPreviewState';

function getStatusIcon(status: MappingPreviewStatus): LucideIcon {
  switch (status) {
    case 'errors':
      return XCircle;
    case 'warnings':
      return AlertTriangle;
    case 'ready':
      return CheckCircle;
  }
}

function getStatusColor(status: MappingPreviewStatus): string {
  switch (status) {
    case 'errors':
      return 'text-[hsl(var(--accent-error))]';
    case 'warnings':
      return 'text-[hsl(var(--accent-warning))]';
    case 'ready':
      return 'text-[hsl(var(--accent-success))]';
  }
}

function getStatusLabel(status: MappingPreviewStatus, counts: MappingPreviewCounts): string {
  switch (status) {
    case 'errors':
      return 'Configuration error';
    case 'warnings':
      return `${counts.conflictCount} conflict${counts.conflictCount !== 1 ? 's' : ''}`;
    case 'ready':
      return `${counts.toCreateCount} links ready`;
  }
}

function MappingPreviewStatusBadge({
  counts,
  isLoading,
  status,
}: {
  counts: MappingPreviewCounts;
  isLoading: boolean;
  status: MappingPreviewStatus;
}) {
  if (isLoading) {
    return <RefreshCw className="w-4 h-4 animate-spin text-[hsl(var(--launcher-text-secondary))]" />;
  }

  const StatusIcon = getStatusIcon(status);
  const color = getStatusColor(status);
  return (
    <div className="flex items-center gap-2">
      <StatusIcon className={`w-4 h-4 ${color}`} />
      <span className={`text-xs ${color}`}>{getStatusLabel(status, counts)}</span>
    </div>
  );
}

function MappingPreviewActionCount({ preview }: { preview: MappingPreviewResponse | null }) {
  if (!preview) {
    return null;
  }

  return (
    <span className="text-xs text-[hsl(var(--launcher-text-tertiary))]">
      {preview.total_actions} action{preview.total_actions !== 1 ? 's' : ''}
    </span>
  );
}

export function MappingPreviewHeader({
  counts,
  isExpanded,
  isLoading,
  preview,
  status,
  onToggleExpanded,
}: {
  counts: MappingPreviewCounts;
  isExpanded: boolean;
  isLoading: boolean;
  preview: MappingPreviewResponse | null;
  status: MappingPreviewStatus;
  onToggleExpanded: () => void;
}) {
  return (
    <button
      onClick={onToggleExpanded}
      className="w-full px-4 py-3 flex items-center justify-between hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors rounded-lg"
    >
      <div className="flex items-center gap-3">
        <FolderSymlink className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
        <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
          Mapping Preview
        </span>
        <MappingPreviewStatusBadge counts={counts} isLoading={isLoading} status={status} />
      </div>
      <div className="flex items-center gap-2">
        <MappingPreviewActionCount preview={preview} />
        {isExpanded ? (
          <ChevronUp className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
        ) : (
          <ChevronDown className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
        )}
      </div>
    </button>
  );
}
