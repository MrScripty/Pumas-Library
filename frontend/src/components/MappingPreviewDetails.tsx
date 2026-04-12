import {
  AlertCircle,
  AlertTriangle,
  CheckCircle,
  HardDrive,
  Link,
  Plus,
  RefreshCw,
  SkipForward,
  XCircle,
} from 'lucide-react';
import {
  renderConflictAction,
  renderCreateAction,
  renderSkipAction,
} from './MappingPreviewActionRows';
import type { MappingPreviewResponse } from './MappingPreviewTypes';
import { MappingPreviewListSection } from './MappingPreviewListSection';

interface MappingPreviewDetailsProps {
  applyResult: {
    success: boolean;
    links_created: number;
    links_removed: number;
    error?: string;
  } | null;
  brokenCount: number;
  conflictCount: number;
  crossFsWarning: {
    cross_filesystem: boolean;
    warning?: string;
    recommendation?: string;
  } | null;
  expandedSection: string | null;
  hasIssues: boolean;
  isApplying: boolean;
  isLoading: boolean;
  preview: MappingPreviewResponse;
  showApplyButton: boolean;
  skipCount: number;
  status: 'ready' | 'warnings' | 'errors';
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
      {crossFsWarning?.cross_filesystem && (
        <div className="p-3 bg-[hsl(var(--accent-warning)/0.1)] rounded-lg border border-[hsl(var(--accent-warning)/0.3)]">
          <div className="flex items-start gap-2">
            <HardDrive className="w-4 h-4 text-[hsl(var(--accent-warning))] flex-shrink-0 mt-0.5" />
            <div>
              <div className="text-sm font-medium text-[hsl(var(--accent-warning))]">
                Cross-Filesystem Warning
              </div>
              <div className="text-xs text-[hsl(var(--launcher-text-secondary))] mt-1">
                {crossFsWarning.warning}
              </div>
              {crossFsWarning.recommendation && (
                <div className="text-xs text-[hsl(var(--launcher-text-tertiary))] mt-1">
                  Tip: {crossFsWarning.recommendation}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      <div className="grid grid-cols-4 gap-2 text-center">
        <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
          <div className="text-lg font-semibold text-[hsl(var(--accent-success))]">
            {toCreateCount}
          </div>
          <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">To Create</div>
        </div>
        <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
          <div className="text-lg font-semibold text-[hsl(var(--launcher-text-tertiary))]">
            {skipCount}
          </div>
          <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Existing</div>
        </div>
        <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
          <div
            className={`text-lg font-semibold ${conflictCount > 0 ? 'text-[hsl(var(--accent-warning))]' : 'text-[hsl(var(--launcher-text-primary))]'}`}
          >
            {conflictCount}
          </div>
          <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Conflicts</div>
        </div>
        <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
          <div
            className={`text-lg font-semibold ${brokenCount > 0 ? 'text-[hsl(var(--accent-error))]' : 'text-[hsl(var(--launcher-text-primary))]'}`}
          >
            {brokenCount}
          </div>
          <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Broken</div>
        </div>
      </div>

      {preview.warnings.length > 0 && (
        <div className="space-y-1">
          {preview.warnings.map((warning, index) => (
            <div
              key={index}
              className="text-xs p-2 bg-[hsl(var(--accent-warning)/0.1)] rounded border border-[hsl(var(--accent-warning)/0.2)] flex items-start gap-2"
            >
              <AlertTriangle className="w-3 h-3 text-[hsl(var(--accent-warning))] flex-shrink-0 mt-0.5" />
              <span className="text-[hsl(var(--launcher-text-secondary))]">{warning}</span>
            </div>
          ))}
        </div>
      )}

      {toCreateCount > 0 && (
        <MappingPreviewListSection
          count={toCreateCount}
          expandedSection={expandedSection}
          icon={<Plus className="w-3 h-3 text-[hsl(var(--accent-success))]" />}
          sectionId="create"
          title="Links to Create"
          onToggle={onToggleSection}
        >
          <div className="max-h-48 overflow-y-auto px-3 pb-2 space-y-1">
            {preview.to_create.map(renderCreateAction)}
          </div>
        </MappingPreviewListSection>
      )}

      {skipCount > 0 && (
        <MappingPreviewListSection
          count={skipCount}
          expandedSection={expandedSection}
          icon={<SkipForward className="w-3 h-3 text-[hsl(var(--launcher-text-tertiary))]" />}
          sectionId="skip"
          title="Already Linked"
          onToggle={onToggleSection}
        >
          <div className="max-h-32 overflow-y-auto px-3 pb-2 space-y-1">
            {preview.to_skip_exists.map(renderSkipAction)}
          </div>
        </MappingPreviewListSection>
      )}

      {conflictCount > 0 && (
        <MappingPreviewListSection
          accentBorderClass="border-[hsl(var(--accent-warning)/0.3)]"
          buttonHoverClass="hover:bg-[hsl(var(--accent-warning)/0.1)]"
          count={conflictCount}
          expandedSection={expandedSection}
          icon={<AlertCircle className="w-3 h-3 text-[hsl(var(--accent-warning))]" />}
          sectionId="conflicts"
          title="Conflicts"
          titleClassName="text-[hsl(var(--accent-warning))]"
          onToggle={onToggleSection}
        >
          <div className="max-h-48 overflow-y-auto px-3 pb-2 space-y-1">
            {preview.conflicts.map(renderConflictAction)}
          </div>
          <div className="px-3 pb-2">
            <p className="text-xs text-[hsl(var(--launcher-text-tertiary))] mb-2">
              Use the Resolve Conflicts button to choose how to handle each conflict.
            </p>
          </div>
        </MappingPreviewListSection>
      )}

      {applyResult && (
        <div
          className={`p-3 rounded-lg border ${
            applyResult.success
              ? 'bg-[hsl(var(--accent-success)/0.1)] border-[hsl(var(--accent-success)/0.3)]'
              : 'bg-[hsl(var(--accent-error)/0.1)] border-[hsl(var(--accent-error)/0.3)]'
          }`}
        >
          <div className="flex items-start gap-2">
            {applyResult.success ? (
              <CheckCircle className="w-4 h-4 text-[hsl(var(--accent-success))] flex-shrink-0 mt-0.5" />
            ) : (
              <XCircle className="w-4 h-4 text-[hsl(var(--accent-error))] flex-shrink-0 mt-0.5" />
            )}
            <div>
              <div
                className={`text-sm font-medium ${
                  applyResult.success
                    ? 'text-[hsl(var(--accent-success))]'
                    : 'text-[hsl(var(--accent-error))]'
                }`}
              >
                {applyResult.success ? 'Mapping Applied' : 'Mapping Failed'}
              </div>
              <div className="text-xs text-[hsl(var(--launcher-text-secondary))] mt-1">
                {applyResult.success ? (
                  <>
                    Created {applyResult.links_created} link
                    {applyResult.links_created !== 1 ? 's' : ''}
                    {applyResult.links_removed > 0 && (
                      <>, removed {applyResult.links_removed} broken</>
                    )}
                  </>
                ) : (
                  applyResult.error || 'Unknown error occurred'
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      <div className="flex gap-2 pt-2">
        <button
          onClick={onFetchPreview}
          disabled={isLoading || isApplying}
          className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--launcher-bg-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] rounded transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-3 h-3 ${isLoading ? 'animate-spin' : ''}`} />
          Refresh
        </button>
        {showApplyButton && toCreateCount > 0 && (
          <button
            onClick={onApplyMapping}
            disabled={isLoading || isApplying || status === 'errors'}
            className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--accent-primary))] hover:bg-[hsl(var(--accent-primary-hover))] text-white rounded transition-colors disabled:opacity-50"
          >
            {isApplying ? (
              <>
                <RefreshCw className="w-3 h-3 animate-spin" />
                Applying...
              </>
            ) : (
              <>
                <Link className="w-3 h-3" />
                Apply Mapping
              </>
            )}
          </button>
        )}
      </div>

      {!hasIssues && toCreateCount === 0 && skipCount > 0 && !applyResult && (
        <div className="text-xs text-center text-[hsl(var(--accent-success))] py-2 flex items-center justify-center gap-2">
          <CheckCircle className="w-4 h-4" />
          All models already linked
        </div>
      )}
    </div>
  );
}
