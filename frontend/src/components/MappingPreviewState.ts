import type { MappingPreviewStatus } from './MappingPreviewDetailsTypes';
import type { MappingPreviewResponse } from './MappingPreviewTypes';

export interface MappingPreviewCounts {
  brokenCount: number;
  conflictCount: number;
  skipCount: number;
  toCreateCount: number;
  warningCount: number;
}

export function getMappingPreviewCounts(preview: MappingPreviewResponse | null): MappingPreviewCounts {
  return {
    brokenCount: preview?.broken_to_remove?.length || 0,
    conflictCount: preview?.conflicts?.length || 0,
    skipCount: preview?.to_skip_exists?.length || 0,
    toCreateCount: preview?.to_create?.length || 0,
    warningCount: preview?.warnings?.length || 0,
  };
}

export function hasMappingPreviewIssues(counts: MappingPreviewCounts): boolean {
  return counts.conflictCount > 0 || counts.warningCount > 0;
}

export function getMappingPreviewStatus(
  preview: MappingPreviewResponse | null,
  counts: MappingPreviewCounts
): MappingPreviewStatus {
  if (preview?.errors?.length) {
    return 'errors';
  }
  if (hasMappingPreviewIssues(counts)) {
    return 'warnings';
  }
  return 'ready';
}
