import { formatSize } from '../utils/modelFormatters';

interface LocalModelMetadataSummaryProps {
  dependencyCount?: number | undefined;
  format?: string | undefined;
  hasDependencies?: boolean | undefined;
  partialError?: string | undefined;
  quant?: string | undefined;
  size?: number | undefined;
}

function formatModelFormat(format?: string): string {
  return format ? format.toUpperCase() : 'N/A';
}

function formatQuantLabel(quant?: string): string {
  return quant ?? 'N/A';
}

export function LocalModelMetadataSummary({
  dependencyCount,
  format,
  hasDependencies,
  partialError,
  quant,
  size,
}: LocalModelMetadataSummaryProps) {
  return (
    <>
      <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-[hsl(var(--text-secondary))]">
        <span className="min-w-0 truncate">
          {formatModelFormat(format)}
        </span>
        <span className="min-w-0 truncate">
          {formatQuantLabel(quant)}
        </span>
        <span className="min-w-0 truncate">
          {formatSize(size)}
        </span>
        <div className="flex items-center">
          {hasDependencies && (
            <span
              className="inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium bg-[hsl(var(--accent-success)/0.14)] text-[hsl(var(--accent-success))]"
              title={dependencyCount
                ? `${dependencyCount} dependency binding${dependencyCount === 1 ? '' : 's'}`
                : 'Dependency bindings projected from the library index'}
            >
              Deps
            </span>
          )}
        </div>
      </div>
      {partialError && (
        <div className="mt-1 text-xs text-[hsl(var(--accent-error))]">
          {partialError}
        </div>
      )}
    </>
  );
}
