import { CheckCircle, Link, RefreshCw, XCircle } from 'lucide-react';
import type { MappingApplyResult, MappingPreviewStatus } from './MappingPreviewDetailsTypes';

function getApplyResultClasses(applyResult: MappingApplyResult): string {
  if (applyResult.success) {
    return 'bg-[hsl(var(--accent-success)/0.1)] border-[hsl(var(--accent-success)/0.3)]';
  }
  return 'bg-[hsl(var(--accent-error)/0.1)] border-[hsl(var(--accent-error)/0.3)]';
}

function ApplyResultIcon({ applyResult }: { applyResult: MappingApplyResult }) {
  if (applyResult.success) {
    return <CheckCircle className="w-4 h-4 text-[hsl(var(--accent-success))] flex-shrink-0 mt-0.5" />;
  }
  return <XCircle className="w-4 h-4 text-[hsl(var(--accent-error))] flex-shrink-0 mt-0.5" />;
}

function ApplyResultMessage({ applyResult }: { applyResult: MappingApplyResult }) {
  if (!applyResult.success) {
    return <>{applyResult.error || 'Unknown error occurred'}</>;
  }

  return (
    <>
      Created {applyResult.links_created} link
      {applyResult.links_created !== 1 ? 's' : ''}
      {applyResult.links_removed > 0 && <>, removed {applyResult.links_removed} broken</>}
    </>
  );
}

export function ApplyResultAlert({
  applyResult,
}: {
  applyResult: MappingApplyResult | null;
}) {
  if (!applyResult) {
    return null;
  }

  return (
    <div className={`p-3 rounded-lg border ${getApplyResultClasses(applyResult)}`}>
      <div className="flex items-start gap-2">
        <ApplyResultIcon applyResult={applyResult} />
        <div>
          <div
            className={`text-sm font-medium ${
              applyResult.success ? 'text-[hsl(var(--accent-success))]' : 'text-[hsl(var(--accent-error))]'
            }`}
          >
            {applyResult.success ? 'Mapping Applied' : 'Mapping Failed'}
          </div>
          <div className="text-xs text-[hsl(var(--launcher-text-secondary))] mt-1">
            <ApplyResultMessage applyResult={applyResult} />
          </div>
        </div>
      </div>
    </div>
  );
}

function ApplyMappingButtonContent({ isApplying }: { isApplying: boolean }) {
  if (isApplying) {
    return (
      <>
        <RefreshCw className="w-3 h-3 animate-spin" />
        Applying...
      </>
    );
  }

  return (
    <>
      <Link className="w-3 h-3" />
      Apply Mapping
    </>
  );
}

export function MappingPreviewControls({
  isApplying,
  isLoading,
  showApplyButton,
  status,
  toCreateCount,
  onApplyMapping,
  onFetchPreview,
}: {
  isApplying: boolean;
  isLoading: boolean;
  showApplyButton: boolean;
  status: MappingPreviewStatus;
  toCreateCount: number;
  onApplyMapping: () => void;
  onFetchPreview: () => void;
}) {
  const canShowApplyButton = showApplyButton && toCreateCount > 0;

  return (
    <div className="flex gap-2 pt-2">
      <button
        onClick={onFetchPreview}
        disabled={isLoading || isApplying}
        className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--launcher-bg-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] rounded transition-colors disabled:opacity-50"
      >
        <RefreshCw className={`w-3 h-3 ${isLoading ? 'animate-spin' : ''}`} />
        Refresh
      </button>
      {canShowApplyButton && (
        <button
          onClick={onApplyMapping}
          disabled={isLoading || isApplying || status === 'errors'}
          className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--accent-primary))] hover:bg-[hsl(var(--accent-primary-hover))] text-white rounded transition-colors disabled:opacity-50"
        >
          <ApplyMappingButtonContent isApplying={isApplying} />
        </button>
      )}
    </div>
  );
}

export function AllModelsLinkedNotice({
  applyResult,
  hasIssues,
  skipCount,
  toCreateCount,
}: {
  applyResult: MappingApplyResult | null;
  hasIssues: boolean;
  skipCount: number;
  toCreateCount: number;
}) {
  if (hasIssues || toCreateCount > 0 || skipCount === 0 || applyResult) {
    return null;
  }

  return (
    <div className="text-xs text-center text-[hsl(var(--accent-success))] py-2 flex items-center justify-center gap-2">
      <CheckCircle className="w-4 h-4" />
      All models already linked
    </div>
  );
}
