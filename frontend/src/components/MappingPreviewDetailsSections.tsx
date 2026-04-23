import {
  AlertCircle,
  AlertTriangle,
  HardDrive,
  Plus,
  SkipForward,
} from 'lucide-react';
import {
  renderConflictAction,
  renderCreateAction,
  renderSkipAction,
} from './MappingPreviewActionRows';
import { MappingPreviewListSection } from './MappingPreviewListSection';
import type { MappingPreviewResponse } from './MappingPreviewTypes';
import type { MappingCrossFilesystemWarning } from './MappingPreviewDetailsTypes';

export function CrossFilesystemWarning({
  warning,
}: {
  warning: MappingCrossFilesystemWarning | null;
}) {
  if (!warning?.cross_filesystem) {
    return null;
  }

  return (
    <div className="p-3 bg-[hsl(var(--accent-warning)/0.1)] rounded-lg border border-[hsl(var(--accent-warning)/0.3)]">
      <div className="flex items-start gap-2">
        <HardDrive className="w-4 h-4 text-[hsl(var(--accent-warning))] flex-shrink-0 mt-0.5" />
        <div>
          <div className="text-sm font-medium text-[hsl(var(--accent-warning))]">
            Cross-Filesystem Warning
          </div>
          <div className="text-xs text-[hsl(var(--launcher-text-secondary))] mt-1">
            {warning.warning}
          </div>
          {warning.recommendation && (
            <div className="text-xs text-[hsl(var(--launcher-text-tertiary))] mt-1">
              Tip: {warning.recommendation}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function getIssueCountClassName(count: number, issueClassName: string): string {
  if (count > 0) {
    return issueClassName;
  }
  return 'text-[hsl(var(--launcher-text-primary))]';
}

function SummaryCard({
  label,
  value,
  valueClassName,
}: {
  label: string;
  value: number;
  valueClassName: string;
}) {
  return (
    <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
      <div className={`text-lg font-semibold ${valueClassName}`}>{value}</div>
      <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">{label}</div>
    </div>
  );
}

export function MappingSummaryGrid({
  brokenCount,
  conflictCount,
  skipCount,
  toCreateCount,
}: {
  brokenCount: number;
  conflictCount: number;
  skipCount: number;
  toCreateCount: number;
}) {
  return (
    <div className="grid grid-cols-4 gap-2 text-center">
      <SummaryCard
        label="To Create"
        value={toCreateCount}
        valueClassName="text-[hsl(var(--accent-success))]"
      />
      <SummaryCard
        label="Existing"
        value={skipCount}
        valueClassName="text-[hsl(var(--launcher-text-tertiary))]"
      />
      <SummaryCard
        label="Conflicts"
        value={conflictCount}
        valueClassName={getIssueCountClassName(conflictCount, 'text-[hsl(var(--accent-warning))]')}
      />
      <SummaryCard
        label="Broken"
        value={brokenCount}
        valueClassName={getIssueCountClassName(brokenCount, 'text-[hsl(var(--accent-error))]')}
      />
    </div>
  );
}

export function MappingWarnings({ warnings }: { warnings: string[] }) {
  if (warnings.length === 0) {
    return null;
  }

  return (
    <div className="space-y-1">
      {warnings.map((warning, index) => (
        <div
          key={index}
          className="text-xs p-2 bg-[hsl(var(--accent-warning)/0.1)] rounded border border-[hsl(var(--accent-warning)/0.2)] flex items-start gap-2"
        >
          <AlertTriangle className="w-3 h-3 text-[hsl(var(--accent-warning))] flex-shrink-0 mt-0.5" />
          <span className="text-[hsl(var(--launcher-text-secondary))]">{warning}</span>
        </div>
      ))}
    </div>
  );
}

export function LinksToCreateSection({
  count,
  expandedSection,
  preview,
  onToggleSection,
}: {
  count: number;
  expandedSection: string | null;
  preview: MappingPreviewResponse;
  onToggleSection: (section: string) => void;
}) {
  if (count === 0) {
    return null;
  }

  return (
    <MappingPreviewListSection
      count={count}
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
  );
}

export function AlreadyLinkedSection({
  count,
  expandedSection,
  preview,
  onToggleSection,
}: {
  count: number;
  expandedSection: string | null;
  preview: MappingPreviewResponse;
  onToggleSection: (section: string) => void;
}) {
  if (count === 0) {
    return null;
  }

  return (
    <MappingPreviewListSection
      count={count}
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
  );
}

export function ConflictsSection({
  count,
  expandedSection,
  preview,
  onToggleSection,
}: {
  count: number;
  expandedSection: string | null;
  preview: MappingPreviewResponse;
  onToggleSection: (section: string) => void;
}) {
  if (count === 0) {
    return null;
  }

  return (
    <MappingPreviewListSection
      accentBorderClass="border-[hsl(var(--accent-warning)/0.3)]"
      buttonHoverClass="hover:bg-[hsl(var(--accent-warning)/0.1)]"
      count={count}
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
  );
}
