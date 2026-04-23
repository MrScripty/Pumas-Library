import React from 'react';
import { ChevronDown, ChevronUp, ExternalLink } from 'lucide-react';

interface ModelMetadataGridProps {
  metadata: Record<string, unknown>;
  isGguf: boolean;
  sourceKey: string;
  showAllFields: boolean;
  expandedFieldKeys: Set<string>;
  copiedFieldKey: string | null;
  onToggleShowAllFields: () => void;
  onToggleFieldExpanded: (fieldKey: string) => void;
  onCopyFieldValue: (fieldKey: string, value: unknown) => void;
  formatFieldName: (key: string) => string;
  formatMetadataValue: (key: string, value: unknown) => string;
  isPriorityGgufField: (key: string) => boolean;
  isHiddenGgufField: (key: string, value: unknown) => boolean;
  linkedGgufFields: Record<string, string>;
}

function isRecordValue(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isStructuredValue(value: unknown): boolean {
  return Array.isArray(value) || isRecordValue(value);
}

function getStructuredValueSummary(key: string, value: unknown): string {
  const lowerKey = key.toLowerCase();

  if (lowerKey === 'dependency_bindings' && Array.isArray(value)) {
    return `Dependency bindings (${value.length})`;
  }
  if (lowerKey === 'files' && Array.isArray(value)) {
    return `Files (${value.length})`;
  }
  if (lowerKey === 'hashes' && isRecordValue(value)) {
    const hashKinds = Object.keys(value).filter(
      (candidate) => value[candidate] != null && String(value[candidate]).trim() !== ''
    );
    if (hashKinds.length === 0) return 'Hashes (empty)';
    const preview = hashKinds.slice(0, 3).join(', ');
    return `Hashes (${preview}${hashKinds.length > 3 ? ', ...' : ''})`;
  }

  if (Array.isArray(value)) {
    if (value.length === 0) return 'Array (0)';
    const typedValues = value.filter((item) => item != null);
    if (typedValues.length > 0 && typedValues.every((item) => typeof item === 'string')) {
      return `Array of strings (${value.length})`;
    }
    if (typedValues.length > 0 && typedValues.every((item) => isRecordValue(item))) {
      return `Array of objects (${value.length})`;
    }
    return `Array (${value.length})`;
  }

  if (isRecordValue(value)) {
    const keys = Object.keys(value);
    if (keys.length === 0) return 'Object (0 keys)';
    const preview = keys.slice(0, 3).join(', ');
    return `Object (${keys.length} keys: ${preview}${keys.length > 3 ? ', ...' : ''})`;
  }

  return String(value ?? '');
}

function serializeMetadataValue(value: unknown): string {
  if (typeof value === 'string') return value;
  try {
    const serialized: unknown = JSON.stringify(value, null, 2);
    return typeof serialized === 'string' ? serialized : String(value);
  } catch {
    return String(value);
  }
}

export function ModelMetadataGrid({
  metadata,
  isGguf,
  sourceKey,
  showAllFields,
  expandedFieldKeys,
  copiedFieldKey,
  onToggleShowAllFields,
  onToggleFieldExpanded,
  onCopyFieldValue,
  formatFieldName,
  formatMetadataValue,
  isPriorityGgufField,
  isHiddenGgufField,
  linkedGgufFields,
}: ModelMetadataGridProps) {
  const entries = Object.entries(metadata).filter(([key]) => key !== 'notes');

  if (isGguf) {
    const priorityEntries = entries.filter(([key]) => isPriorityGgufField(key));
    const otherEntries = entries.filter(
      ([key, value]) => !isPriorityGgufField(key) && !isHiddenGgufField(key, value)
    );
    const hiddenEntries = entries.filter(([key, value]) => isHiddenGgufField(key, value));
    const displayEntries = showAllFields
      ? [...priorityEntries, ...otherEntries, ...hiddenEntries]
      : [...priorityEntries, ...otherEntries.slice(0, 5)];
    const hiddenCount =
      hiddenEntries.length + (showAllFields ? 0 : Math.max(0, otherEntries.length - 5));

    return (
      <div className="space-y-2">
        <div className="grid max-h-80 grid-cols-2 gap-x-4 gap-y-1 overflow-y-auto text-sm">
          {displayEntries.map(([key, value]) => {
            const linkedUrl = linkedGgufFields[key.toLowerCase()];
            const urlValue = linkedUrl ? (metadata[linkedUrl] as string) : null;

            return (
              <React.Fragment key={key}>
                <span className="truncate text-[hsl(var(--text-muted))]" title={key}>
                  {formatFieldName(key)}
                </span>
                <span
                  className="truncate text-[hsl(var(--text-secondary))]"
                  title={formatMetadataValue(key, value)}
                >
                  {urlValue ? (
                    <a
                      href={urlValue}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="inline-flex items-center gap-1 text-[hsl(var(--accent-link))] hover:underline"
                    >
                      {formatMetadataValue(key, value)}
                      <ExternalLink className="h-3 w-3" />
                    </a>
                  ) : (
                    formatMetadataValue(key, value)
                  )}
                </span>
              </React.Fragment>
            );
          })}
        </div>
        {hiddenCount > 0 && (
          <button
            onClick={onToggleShowAllFields}
            className="flex items-center gap-1 text-xs text-[hsl(var(--text-muted))] hover:text-[hsl(var(--text-primary))]"
          >
            {showAllFields ? (
              <>
                <ChevronUp className="h-3 w-3" /> Show less
              </>
            ) : (
              <>
                <ChevronDown className="h-3 w-3" /> Show {hiddenCount} more fields
              </>
            )}
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="grid max-h-80 grid-cols-2 gap-x-4 gap-y-1 overflow-y-auto text-sm">
      {entries.map(([key, value]) => {
        const fieldKey = `${sourceKey}:${key}`;
        const isStructured = isStructuredValue(value);
        const isExpanded = expandedFieldKeys.has(fieldKey);
        const summaryLabel = isStructured
          ? getStructuredValueSummary(key, value)
          : formatMetadataValue(key, value);

        return (
          <React.Fragment key={key}>
            <span className="truncate text-[hsl(var(--text-muted))]" title={key}>
              {formatFieldName(key)}
            </span>
            <div className="text-[hsl(var(--text-secondary))]">
              {!isStructured ? (
                <span className="block truncate" title={summaryLabel}>
                  {summaryLabel}
                </span>
              ) : (
                <div className="flex min-w-0 items-center gap-2">
                  <span className="truncate" title={summaryLabel}>
                    {summaryLabel}
                  </span>
                  <button
                    type="button"
                    onClick={() => onToggleFieldExpanded(fieldKey)}
                    className="shrink-0 rounded border border-[hsl(var(--border-default))] px-1.5 py-0.5 text-xs hover:bg-[hsl(var(--surface-high))]"
                    title={isExpanded ? 'Collapse value' : 'Expand value'}
                  >
                    {isExpanded ? 'Collapse' : 'Expand'}
                  </button>
                  <button
                    type="button"
                    onClick={() => onCopyFieldValue(fieldKey, value)}
                    className="shrink-0 rounded border border-[hsl(var(--border-default))] px-1.5 py-0.5 text-xs hover:bg-[hsl(var(--surface-high))]"
                    title="Copy full JSON value"
                  >
                    {copiedFieldKey === fieldKey ? 'Copied' : 'Copy'}
                  </button>
                </div>
              )}
            </div>
            {isStructured && isExpanded && (
              <pre className="col-span-2 mb-2 mt-1 break-all rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-high)/0.5)] p-2 font-mono text-xs whitespace-pre-wrap text-[hsl(var(--text-secondary))]">
                {serializeMetadataValue(value)}
              </pre>
            )}
          </React.Fragment>
        );
      })}
    </div>
  );
}
