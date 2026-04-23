import {
  AllModelsLinkedNotice,
  ApplyResultAlert,
  MappingPreviewControls,
} from './MappingPreviewDetailsFeedback';
import {
  AlreadyLinkedSection,
  ConflictsSection,
  CrossFilesystemWarning,
  LinksToCreateSection,
  MappingSummaryGrid,
  MappingWarnings,
} from './MappingPreviewDetailsSections';
import type {
  MappingApplyResult,
  MappingCrossFilesystemWarning,
  MappingPreviewStatus,
} from './MappingPreviewDetailsTypes';
import type { MappingPreviewResponse } from './MappingPreviewTypes';

interface MappingPreviewDetailsProps {
  applyResult: MappingApplyResult | null;
  brokenCount: number;
  conflictCount: number;
  crossFsWarning: MappingCrossFilesystemWarning | null;
  expandedSection: string | null;
  hasIssues: boolean;
  isApplying: boolean;
  isLoading: boolean;
  preview: MappingPreviewResponse;
  showApplyButton: boolean;
  skipCount: number;
  status: MappingPreviewStatus;
  toCreateCount: number;
  onApplyMapping: () => void;
  onFetchPreview: () => void;
  onToggleSection: (section: string) => void;
}

export function MappingPreviewDetails({
  applyResult,
  brokenCount,
  conflictCount,
  crossFsWarning,
  expandedSection,
  hasIssues,
  isApplying,
  isLoading,
  preview,
  showApplyButton,
  skipCount,
  status,
  toCreateCount,
  onApplyMapping,
  onFetchPreview,
  onToggleSection,
}: MappingPreviewDetailsProps) {
  return (
    <div className="px-4 pb-4 space-y-3">
      <CrossFilesystemWarning warning={crossFsWarning} />
      <MappingSummaryGrid
        brokenCount={brokenCount}
        conflictCount={conflictCount}
        skipCount={skipCount}
        toCreateCount={toCreateCount}
      />
      <MappingWarnings warnings={preview.warnings} />
      <LinksToCreateSection
        count={toCreateCount}
        expandedSection={expandedSection}
        preview={preview}
        onToggleSection={onToggleSection}
      />
      <AlreadyLinkedSection
        count={skipCount}
        expandedSection={expandedSection}
        preview={preview}
        onToggleSection={onToggleSection}
      />
      <ConflictsSection
        count={conflictCount}
        expandedSection={expandedSection}
        preview={preview}
        onToggleSection={onToggleSection}
      />
      <ApplyResultAlert applyResult={applyResult} />
      <MappingPreviewControls
        isApplying={isApplying}
        isLoading={isLoading}
        showApplyButton={showApplyButton}
        status={status}
        toCreateCount={toCreateCount}
        onApplyMapping={onApplyMapping}
        onFetchPreview={onFetchPreview}
      />
      <AllModelsLinkedNotice
        applyResult={applyResult}
        hasIssues={hasIssues}
        skipCount={skipCount}
        toCreateCount={toCreateCount}
      />
    </div>
  );
}
