import {
  Blocks,
  ChartPie,
  ChartSpline,
  Cpu,
  Download,
  Key,
  UserRound,
  UserRoundSearch,
} from 'lucide-react';
import type { RemoteModelInfo } from '../types/apps';
import {
  formatDownloadSize,
  formatDownloads,
  formatReleaseDate,
  resolveReleaseIcon,
} from '../utils/modelFormatters';
import { ModelKindIcon } from './ModelKindIcon';
import { MetadataItem } from './ui';

interface RemoteModelSummaryProps {
  isHydratingDetails: boolean;
  model: RemoteModelInfo;
  modelError?: string | undefined;
  quantLabels: string[];
  retryHint?: string | null | undefined;
  onHfAuthClick?: (() => void) | undefined;
  onSearchDeveloper?: ((developer: string) => void) | undefined;
}

function formatDownloadSizeRange(model: RemoteModelInfo, isHydrating: boolean): string {
  const optionSizes = model.downloadOptions?.map((option) => option.sizeBytes) ?? [];
  const validSizes = optionSizes.filter((size): size is number => typeof size === 'number' && size > 0);
  if (validSizes.length > 1) {
    const min = Math.min(...validSizes);
    const max = Math.max(...validSizes);
    const formatValue = (bytes: number) => {
      const gb = bytes / (1024 ** 3);
      return gb >= 10 ? gb.toFixed(1) : gb.toFixed(2);
    };
    return `${formatValue(min)}-${formatValue(max)} GB`;
  }
  if (validSizes.length === 1) {
    return formatDownloadSize(validSizes[0]);
  }
  if (typeof model.totalSizeBytes === 'number' && model.totalSizeBytes > 0) {
    return formatDownloadSize(model.totalSizeBytes);
  }
  return isHydrating ? 'Loading details...' : 'Load details';
}

function getEngineStyle(engine: string): string {
  switch (engine.toLowerCase()) {
    case 'ollama':
      return 'bg-[hsl(var(--launcher-accent-primary)/0.15)] text-[hsl(var(--launcher-accent-primary))]';
    case 'llama.cpp':
      return 'bg-[hsl(var(--launcher-accent-info)/0.15)] text-[hsl(var(--launcher-accent-info))]';
    case 'candle':
    case 'transformers':
      return 'bg-[hsl(var(--launcher-accent-warning)/0.15)] text-[hsl(var(--launcher-accent-warning))]';
    case 'diffusers':
      return 'bg-[hsl(var(--launcher-accent-gpu)/0.15)] text-[hsl(var(--launcher-accent-gpu))]';
    case 'onnx-runtime':
    case 'tensorrt':
      return 'bg-[hsl(var(--launcher-accent-ram)/0.15)] text-[hsl(var(--launcher-accent-ram))]';
    default:
      return 'bg-[hsl(var(--launcher-bg-secondary)/0.5)] text-[hsl(var(--text-secondary))]';
  }
}

export function RemoteModelSummary({
  isHydratingDetails,
  model,
  modelError,
  quantLabels,
  retryHint,
  onHfAuthClick,
  onSearchDeveloper,
}: RemoteModelSummaryProps) {
  const ReleaseIcon = resolveReleaseIcon(model.releaseDate);

  return (
    <div className="min-w-0">
      <div className="flex items-center gap-2">
        <span className="truncate text-sm font-semibold text-[hsl(var(--text-primary))]">
          {model.name}
        </span>
      </div>
      <div className="mt-1 flex items-start justify-between gap-4 text-xs text-[hsl(var(--text-muted))]">
        <div className="flex min-w-0 flex-col gap-1">
          {model.developer && onSearchDeveloper && (
            <button
              type="button"
              onClick={() => onSearchDeveloper(model.developer)}
              className="group inline-flex items-center gap-1 text-left"
              title="Search by developer"
            >
              <span className="inline-flex">
                <UserRound className="h-3.5 w-3.5 group-hover:hidden" />
                <UserRoundSearch className="hidden h-3.5 w-3.5 group-hover:inline-flex" />
              </span>
              {model.developer}
            </button>
          )}
          <span
            className="inline-flex items-center gap-1"
            title={model.kind}
            aria-label={model.kind}
          >
            <ModelKindIcon kind={model.kind} />
          </span>
        </div>
        <div className="flex flex-col items-end gap-1 text-[hsl(var(--text-muted))]">
          <span className="inline-flex items-center gap-1">
            <span title="Release date" aria-label="Release date" className="inline-flex">
              <ReleaseIcon className="h-3.5 w-3.5" />
            </span>
            {formatReleaseDate(model.releaseDate)}
          </span>
          <span className="inline-flex items-center gap-1">
            <span title="Downloads" aria-label="Downloads" className="inline-flex">
              <ChartSpline className="h-3.5 w-3.5" />
            </span>
            {formatDownloads(model.downloads)}
          </span>
        </div>
      </div>
      <div className="mt-1.5 flex flex-wrap gap-2 text-xs text-[hsl(var(--text-muted))]">
        <MetadataItem icon={<Blocks />}>
          {model.formats.length ? model.formats.join(', ') : 'Unknown'}
        </MetadataItem>
        <MetadataItem icon={<ChartPie />}>
          {quantLabels.length ? quantLabels.join(', ') : 'Unknown'}
        </MetadataItem>
        <MetadataItem icon={<Download />}>
          {formatDownloadSizeRange(model, isHydratingDetails)}
        </MetadataItem>
      </div>
      {model.compatibleEngines && model.compatibleEngines.length > 0 && (
        <div className="mt-1.5 flex flex-wrap gap-1">
          <Cpu className="mr-0.5 h-3.5 w-3.5 text-[hsl(var(--text-muted))]" />
          {model.compatibleEngines.map((engine) => (
            <span
              key={engine}
              className={`rounded px-1.5 py-0.5 text-[10px] font-medium ${getEngineStyle(engine)}`}
              title={`Compatible with ${engine}`}
            >
              {engine}
            </span>
          ))}
        </div>
      )}
      {modelError && (
        <div className="mt-1.5 text-xs text-[hsl(var(--accent-error))]">
          {modelError}
          {/\b401\b/.test(modelError) && onHfAuthClick && (
            <button
              type="button"
              onClick={onHfAuthClick}
              className="ml-2 inline-flex items-center gap-1 text-[hsl(var(--accent-primary))] hover:underline"
            >
              <Key className="h-3 w-3" />
              Sign in to HuggingFace
            </button>
          )}
        </div>
      )}
      {retryHint && (
        <div className="mt-1 text-xs text-[hsl(var(--launcher-accent-warning))]">
          {retryHint}
        </div>
      )}
    </div>
  );
}
