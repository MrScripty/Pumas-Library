/**
 * Ollama Model Section
 *
 * Shows GGUF models from the library that can be loaded into a running
 * Ollama instance. Allows creating, loading/unloading, and deleting
 * Ollama models backed by library GGUF files.
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { Box, Loader2, Play, Square, AlertCircle, Cpu, Trash2 } from 'lucide-react';
import { api, isAPIAvailable } from '../../../api/adapter';
import type { ModelCategory } from '../../../types/apps';
import type { OllamaModelInfo, OllamaRunningModel } from '../../../types/api';
import { Tooltip } from '../../ui';
import { getLogger } from '../../../utils/logger';

const logger = getLogger('OllamaModelSection');

/** A library model that has a GGUF file. */
interface GgufLibraryModel {
  id: string;
  name: string;
  category: string;
  size?: number;
}

export interface OllamaModelSectionProps {
  connectionUrl: string;
  isRunning: boolean;
  modelGroups: ModelCategory[];
}

export function OllamaModelSection({
  connectionUrl,
  isRunning,
  modelGroups,
}: OllamaModelSectionProps) {
  const [ollamaModels, setOllamaModels] = useState<OllamaModelInfo[]>([]);
  const [runningModels, setRunningModels] = useState<OllamaRunningModel[]>([]);
  const [loadingModel, setLoadingModel] = useState<string | null>(null);
  const [togglingModel, setTogglingModel] = useState<string | null>(null);
  const [deletingModel, setDeletingModel] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  // Extract GGUF models from library model groups
  const ggufModels: GgufLibraryModel[] = [];
  for (const group of modelGroups) {
    for (const model of group.models) {
      const path = model.path || model.name || '';
      if (hasGgufFile(path)) {
        ggufModels.push({
          id: model.id,
          name: model.name,
          category: group.category,
          size: model.size,
        });
      }
    }
  }

  // Set of model names currently loaded in memory
  const runningSet = useMemo(
    () => new Set(runningModels.map((m) => m.name)),
    [runningModels]
  );

  // Map running model name -> VRAM size
  const vramMap = useMemo(() => {
    const map = new Map<string, number>();
    for (const m of runningModels) {
      map.set(m.name, m.size_vram);
    }
    return map;
  }, [runningModels]);

  const fetchOllamaState = useCallback(async () => {
    if (!isAPIAvailable() || !isRunning) {
      setOllamaModels([]);
      setRunningModels([]);
      return;
    }

    try {
      setIsRefreshing(true);
      const [tagsResult, psResult] = await Promise.all([
        api.ollama_list_models(connectionUrl),
        api.ollama_list_running(connectionUrl),
      ]);
      if (tagsResult.success && tagsResult.models) {
        setOllamaModels(tagsResult.models);
      }
      if (psResult.success && psResult.models) {
        setRunningModels(psResult.models);
      }
    } catch (err) {
      logger.debug('Failed to fetch Ollama state', { error: err });
    } finally {
      setIsRefreshing(false);
    }
  }, [connectionUrl, isRunning]);

  useEffect(() => {
    if (isRunning) {
      fetchOllamaState();
      const interval = setInterval(fetchOllamaState, 10000);
      return () => clearInterval(interval);
    }
    setOllamaModels([]);
    setRunningModels([]);
    return undefined;
  }, [fetchOllamaState, isRunning]);

  const handleCreate = async (model: GgufLibraryModel) => {
    if (!isAPIAvailable()) return;

    setLoadingModel(model.id);
    setError(null);

    try {
      const result = await api.ollama_create_model(model.id, undefined, connectionUrl);
      if (result.success) {
        await fetchOllamaState();
      } else {
        setError(result.error || 'Failed to load model into Ollama');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      setError(message);
    } finally {
      setLoadingModel(null);
    }
  };

  const handleToggleLoad = async (modelName: string, isLoaded: boolean) => {
    if (!isAPIAvailable()) return;

    setTogglingModel(modelName);
    setError(null);

    try {
      const result = isLoaded
        ? await api.ollama_unload_model(modelName, connectionUrl)
        : await api.ollama_load_model(modelName, connectionUrl);
      if (result.success) {
        await fetchOllamaState();
      } else {
        setError(result.error || `Failed to ${isLoaded ? 'unload' : 'load'} model`);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      setError(message);
    } finally {
      setTogglingModel(null);
    }
  };

  const handleDelete = async (modelName: string) => {
    if (!isAPIAvailable()) return;

    setDeletingModel(modelName);
    setError(null);

    try {
      const result = await api.ollama_delete_model(modelName, connectionUrl);
      if (result.success) {
        await fetchOllamaState();
      } else {
        setError(result.error || 'Failed to remove model from Ollama');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      setError(message);
    } finally {
      setDeletingModel(null);
    }
  };

  if (!isRunning) {
    return null;
  }

  return (
    <div className="w-full space-y-4">
      {/* Error display */}
      {error && (
        <div className="flex items-center gap-2 px-3 py-2 rounded bg-[hsl(var(--accent-error)/0.1)] text-[hsl(var(--accent-error))]">
          <AlertCircle className="w-4 h-4 shrink-0" />
          <span className="text-sm">{error}</span>
        </div>
      )}

      {/* Ollama-registered models */}
      {ollamaModels.length > 0 && (
        <div className="space-y-3">
          <div className="text-xs uppercase tracking-wider text-[hsl(var(--launcher-text-muted))] flex items-center gap-2">
            <Cpu className="w-3.5 h-3.5" />
            <span>Ollama Models</span>
            {isRefreshing && (
              <Loader2 className="w-3.5 h-3.5 animate-spin text-[hsl(var(--text-secondary))]" />
            )}
          </div>

          <div className="space-y-1.5 max-h-48 overflow-y-auto">
            {ollamaModels.map((model) => {
              const isLoaded = runningSet.has(model.name);
              const isToggling = togglingModel === model.name;
              const isDeleting = deletingModel === model.name;
              const modelVram = vramMap.get(model.name);

              return (
                <div
                  key={model.name}
                  className="flex items-center justify-between gap-2 px-3 py-2 rounded-lg bg-[hsl(var(--launcher-bg-secondary)/0.3)] border border-[hsl(var(--launcher-border)/0.3)] hover:bg-[hsl(var(--launcher-bg-secondary)/0.5)] transition-colors"
                >
                  <div className="flex flex-col min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                        {model.name}
                      </span>
                      {isLoaded && (
                        <span className="shrink-0 px-1.5 py-0.5 text-[10px] font-medium rounded bg-[hsl(var(--accent-success)/0.15)] text-[hsl(var(--accent-success))]">
                          LOADED
                        </span>
                      )}
                    </div>
                    <span className="text-xs text-[hsl(var(--launcher-text-muted))]">
                      {formatSize(model.size)}
                      {isLoaded && modelVram ? ` \u2022 ${formatSize(modelVram)} VRAM` : ''}
                    </span>
                  </div>

                  <div className="flex items-center gap-1">
                    {/* Load / Unload toggle */}
                    <Tooltip content={isLoaded ? 'Unload from memory' : 'Load into memory'} position="left">
                      <button
                        onClick={() => handleToggleLoad(model.name, isLoaded)}
                        disabled={isToggling || isDeleting}
                        className={`p-1.5 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
                          isLoaded
                            ? 'bg-[hsl(var(--accent-success)/0.15)] text-[hsl(var(--accent-success))] hover:bg-[hsl(var(--accent-success)/0.25)]'
                            : 'bg-[hsl(var(--surface-interactive))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))] hover:text-[hsl(var(--text-primary))]'
                        }`}
                      >
                        {isToggling ? (
                          <Loader2 className="w-4 h-4 animate-spin" />
                        ) : isLoaded ? (
                          <Square className="w-4 h-4" />
                        ) : (
                          <Play className="w-4 h-4" />
                        )}
                      </button>
                    </Tooltip>

                    {/* Delete from Ollama */}
                    <Tooltip content="Remove from Ollama" position="left">
                      <button
                        onClick={() => handleDelete(model.name)}
                        disabled={isDeleting || isToggling}
                        className="p-1.5 rounded transition-colors bg-[hsl(var(--accent-error)/0.15)] text-[hsl(var(--accent-error))] hover:bg-[hsl(var(--accent-error)/0.25)] disabled:opacity-50 disabled:cursor-not-allowed"
                      >
                        {isDeleting ? (
                          <Loader2 className="w-4 h-4 animate-spin" />
                        ) : (
                          <Trash2 className="w-4 h-4" />
                        )}
                      </button>
                    </Tooltip>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Library GGUF models available to load */}
      {ggufModels.length > 0 && (
        <div className="space-y-3">
          <div className="text-xs uppercase tracking-wider text-[hsl(var(--launcher-text-muted))] flex items-center gap-2">
            <Box className="w-3.5 h-3.5" />
            <span>Library GGUF Models</span>
          </div>

          <div className="space-y-1.5 max-h-64 overflow-y-auto">
            {ggufModels.map((model) => {
              const isLoading = loadingModel === model.id;

              return (
                <div
                  key={model.id}
                  className="flex items-center justify-between gap-2 px-3 py-2 rounded-lg bg-[hsl(var(--launcher-bg-secondary)/0.3)] border border-[hsl(var(--launcher-border)/0.3)] hover:bg-[hsl(var(--launcher-bg-secondary)/0.5)] transition-colors"
                >
                  <div className="flex flex-col min-w-0">
                    <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                      {model.name}
                    </span>
                    <span className="text-xs text-[hsl(var(--launcher-text-muted))]">
                      {model.category}
                      {model.size ? ` \u2022 ${formatSize(model.size)}` : ''}
                    </span>
                  </div>

                  <Tooltip content="Load into Ollama" position="left">
                    <button
                      onClick={() => handleCreate(model)}
                      disabled={isLoading}
                      className="p-1.5 rounded transition-colors bg-[hsl(var(--surface-interactive))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))] hover:text-[hsl(var(--text-primary))] disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {isLoading ? (
                        <Loader2 className="w-4 h-4 animate-spin" />
                      ) : (
                        <Play className="w-4 h-4" />
                      )}
                    </button>
                  </Tooltip>
                </div>
              );
            })}
          </div>

          {ggufModels.length > 20 && (
            <p className="text-xs text-center text-[hsl(var(--text-muted))]">
              Showing first 20 of {ggufModels.length} models
            </p>
          )}
        </div>
      )}

      {ggufModels.length === 0 && ollamaModels.length === 0 && (
        <div className="w-full py-4 text-center text-[hsl(var(--text-secondary))]">
          <Box className="w-6 h-6 mx-auto mb-2 opacity-50" />
          <p className="text-sm">No GGUF models in library</p>
          <p className="text-xs mt-1 text-[hsl(var(--text-muted))]">
            Download or import GGUF models to load them into Ollama
          </p>
        </div>
      )}
    </div>
  );
}

/** Check if a model path suggests it contains a GGUF file. */
function hasGgufFile(path: string): boolean {
  const lower = path.toLowerCase();
  // Check if the path or model ID contains gguf indicators
  return lower.endsWith('.gguf') || lower.includes('/gguf/') || lower.includes('gguf');
}

function formatSize(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`;
  if (bytes >= 1e3) return `${(bytes / 1e3).toFixed(1)} KB`;
  return `${bytes} B`;
}
