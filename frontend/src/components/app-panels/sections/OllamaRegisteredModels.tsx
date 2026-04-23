import { Cpu, Loader2, Play, Square, Trash2 } from 'lucide-react';
import type { OllamaModelInfo } from '../../../types/api';
import { Tooltip } from '../../ui';
import { formatOllamaModelSize } from './ollamaModelFormatting';

interface OllamaRegisteredModelsProps {
  deletingModel: string | null;
  isRefreshing: boolean;
  models: OllamaModelInfo[];
  runningSet: Set<string>;
  togglingModel: string | null;
  vramMap: Map<string, number>;
  onDelete: (modelName: string) => void;
  onToggleLoad: (modelName: string, isLoaded: boolean) => void;
}

export function OllamaRegisteredModels({
  deletingModel,
  isRefreshing,
  models,
  runningSet,
  togglingModel,
  vramMap,
  onDelete,
  onToggleLoad,
}: OllamaRegisteredModelsProps) {
  if (models.length === 0) {
    return null;
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 text-xs uppercase tracking-wider text-[hsl(var(--launcher-text-muted))]">
        <Cpu className="h-3.5 w-3.5" />
        <span>Ollama Models</span>
        {isRefreshing && (
          <Loader2 className="h-3.5 w-3.5 animate-spin text-[hsl(var(--text-secondary))]" />
        )}
      </div>

      <div className="max-h-48 space-y-1.5 overflow-y-auto">
        {models.map((model) => {
          const isLoaded = runningSet.has(model.name);
          const isToggling = togglingModel === model.name;
          const isDeleting = deletingModel === model.name;
          const modelVram = vramMap.get(model.name);

          return (
            <OllamaRegisteredModelRow
              key={model.name}
              isDeleting={isDeleting}
              isLoaded={isLoaded}
              isToggling={isToggling}
              model={model}
              modelVram={modelVram}
              onDelete={onDelete}
              onToggleLoad={onToggleLoad}
            />
          );
        })}
      </div>
    </div>
  );
}

interface OllamaRegisteredModelRowProps {
  isDeleting: boolean;
  isLoaded: boolean;
  isToggling: boolean;
  model: OllamaModelInfo;
  modelVram?: number | undefined;
  onDelete: (modelName: string) => void;
  onToggleLoad: (modelName: string, isLoaded: boolean) => void;
}

function OllamaRegisteredModelRow({
  isDeleting,
  isLoaded,
  isToggling,
  model,
  modelVram,
  onDelete,
  onToggleLoad,
}: OllamaRegisteredModelRowProps) {
  return (
    <div className="flex items-center justify-between gap-2 rounded-lg border border-[hsl(var(--launcher-border)/0.3)] bg-[hsl(var(--launcher-bg-secondary)/0.3)] px-3 py-2 transition-colors hover:bg-[hsl(var(--launcher-bg-secondary)/0.5)]">
      <div className="flex min-w-0 flex-col">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
            {model.name}
          </span>
          {isLoaded && (
            <span className="shrink-0 rounded bg-[hsl(var(--accent-success)/0.15)] px-1.5 py-0.5 text-[10px] font-medium text-[hsl(var(--accent-success))]">
              LOADED
            </span>
          )}
        </div>
        <span className="text-xs text-[hsl(var(--launcher-text-muted))]">
          {formatOllamaModelSize(model.size)}
          {isLoaded && modelVram ? ` \u2022 ${formatOllamaModelSize(modelVram)} VRAM` : ''}
        </span>
      </div>

      <div className="flex items-center gap-1">
        <Tooltip content={isLoaded ? 'Unload from memory' : 'Load into memory'} position="left">
          <button
            aria-label={isLoaded ? `Unload ${model.name}` : `Load ${model.name}`}
            onClick={() => onToggleLoad(model.name, isLoaded)}
            disabled={isToggling || isDeleting}
            className={`rounded p-1.5 transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${
              isLoaded
                ? 'bg-[hsl(var(--accent-success)/0.15)] text-[hsl(var(--accent-success))] hover:bg-[hsl(var(--accent-success)/0.25)]'
                : 'bg-[hsl(var(--surface-interactive))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))] hover:text-[hsl(var(--text-primary))]'
            }`}
          >
            {isToggling ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : isLoaded ? (
              <Square className="h-4 w-4" />
            ) : (
              <Play className="h-4 w-4" />
            )}
          </button>
        </Tooltip>

        <Tooltip content="Remove from Ollama" position="left">
          <button
            aria-label={`Remove ${model.name} from Ollama`}
            onClick={() => onDelete(model.name)}
            disabled={isDeleting || isToggling}
            className="rounded bg-[hsl(var(--accent-error)/0.15)] p-1.5 text-[hsl(var(--accent-error))] transition-colors hover:bg-[hsl(var(--accent-error)/0.25)] disabled:cursor-not-allowed disabled:opacity-50"
          >
            {isDeleting ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Trash2 className="h-4 w-4" />
            )}
          </button>
        </Tooltip>
      </div>
    </div>
  );
}
