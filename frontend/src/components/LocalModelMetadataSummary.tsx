import { formatSize } from '../utils/modelFormatters';
import { modalityLabel } from '../utils/modelCategories';

interface LocalModelMetadataSummaryProps {
  adapter?: boolean | undefined;
  dependencyCount?: number | undefined;
  inputModalities?: readonly string[] | undefined;
  outputModalities?: readonly string[] | undefined;
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

function formatModalities(modalities: readonly string[]): string {
  return modalities.map(modalityLabel).join(', ');
}

const badgeClass = 'inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium';

export function LocalModelMetadataSummary({
  adapter,
  dependencyCount,
  format,
  hasDependencies,
  inputModalities = [],
  outputModalities = [],
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
        {(inputModalities.length > 0 || outputModalities.length > 0) && (
          <span
            className={`${badgeClass} bg-[hsl(var(--surface-interactive))] text-[hsl(var(--text-secondary))]`}
            title="Inputs the model accepts and outputs it produces"
          >
            {formatModalities(inputModalities) || '?'} → {formatModalities(outputModalities) || '?'}
          </span>
        )}
        {adapter && (
          <span
            className={`${badgeClass} bg-[hsl(var(--accent-warning)/0.14)] text-[hsl(var(--accent-warning))]`}
            title="An adapter (LoRA) for another model; it is not used on its own"
          >
            Adapter
          </span>
        )}
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
