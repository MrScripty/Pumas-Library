/**
 * Model Selector Section for GenericAppPanel.
 *
 * Allows loading/unloading models compatible with the app.
 * Filters local models by supported formats from plugin config.
 */

import { useState, useEffect, useCallback } from 'react';
import { Box, Loader2, Play, Square, AlertCircle } from 'lucide-react';
import { api, isAPIAvailable } from '../../../api/adapter';
import type { ModelCompatibility } from '../../../types/plugins';
import { Tooltip } from '../../ui';
import { getLogger } from '../../../utils/logger';

const logger = getLogger('ModelSelectorSection');

export interface ModelSelectorSectionConfig {
  filter?: string;
}

/** Model info for the selector - simplified version */
interface SelectorModelInfo {
  id?: string;
  name: string;
  path?: string;
  category: string;
  size?: number;
}

export interface ModelSelectorSectionProps {
  appId: string;
  compatibility?: ModelCompatibility;
  config?: ModelSelectorSectionConfig;
  isRunning: boolean;
  localModels?: SelectorModelInfo[];
}

interface LoadedModel {
  name: string;
  size?: number;
}

export function ModelSelectorSection({
  appId,
  compatibility,
  config = {},
  isRunning,
  localModels = [],
}: ModelSelectorSectionProps) {
  const [loadedModels, setLoadedModels] = useState<LoadedModel[]>([]);
  const [loadingModel, setLoadingModel] = useState<string | null>(null);
  const [unloadingModel, setUnloadingModel] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  // Filter models by supported formats
  const compatibleModels = localModels.filter((model) => {
    if (!compatibility?.supportedFormats) return true;

    // Get model format from filename extension or metadata
    const modelFormat = getModelFormat(model);
    return compatibility.supportedFormats.some(
      (fmt) => fmt.toLowerCase() === modelFormat.toLowerCase()
    );
  });

  // Apply additional filter from config
  const filteredModels = config.filter
    ? compatibleModels.filter((model) => {
        const modelFormat = getModelFormat(model);
        return modelFormat.toLowerCase() === config.filter?.toLowerCase();
      })
    : compatibleModels;

  const fetchLoadedModels = useCallback(async () => {
    if (!isAPIAvailable() || !isRunning) {
      setLoadedModels([]);
      return;
    }

    try {
      setIsRefreshing(true);
      const result = await api.call_plugin_endpoint(appId, 'listModels', {});

      if (result.success && result.data) {
        const data = result.data as { models?: LoadedModel[] };
        setLoadedModels(data.models || []);
      }
    } catch (err) {
      logger.debug('Failed to fetch loaded models', { appId, error: err });
    } finally {
      setIsRefreshing(false);
    }
  }, [appId, isRunning]);

  useEffect(() => {
    if (isRunning) {
      void fetchLoadedModels();
      const interval = setInterval(() => void fetchLoadedModels(), 10000);
      return () => clearInterval(interval);
    }
    setLoadedModels([]);
    return undefined;
  }, [fetchLoadedModels, isRunning]);

  const handleLoadModel = async (modelName: string) => {
    if (!isAPIAvailable()) return;

    setLoadingModel(modelName);
    setError(null);

    try {
      const result = await api.call_plugin_endpoint(appId, 'loadModel', {
        model_name: modelName,
      });

      if (result.success) {
        await fetchLoadedModels();
      } else {
        setError(result.error || 'Failed to load model');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      setError(message);
    } finally {
      setLoadingModel(null);
    }
  };

  const handleUnloadModel = async (modelName: string) => {
    if (!isAPIAvailable()) return;

    setUnloadingModel(modelName);
    setError(null);

    try {
      const result = await api.call_plugin_endpoint(appId, 'unloadModel', {
        model_name: modelName,
      });

      if (result.success) {
        await fetchLoadedModels();
      } else {
        setError(result.error || 'Failed to unload model');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      setError(message);
    } finally {
      setUnloadingModel(null);
    }
  };

  const isModelLoaded = (modelName: string): boolean => {
    return loadedModels.some(
      (m) => m.name.toLowerCase() === modelName.toLowerCase()
    );
  };

  if (!isRunning) {
    return null;
  }

  if (filteredModels.length === 0) {
    return (
      <div className="w-full py-4 text-center text-[hsl(var(--text-secondary))]">
        <Box className="w-6 h-6 mx-auto mb-2 opacity-50" />
        <p className="text-sm">No compatible models found</p>
        {compatibility?.supportedFormats && (
          <p className="text-xs mt-1 text-[hsl(var(--text-muted))]">
            Supported: {compatibility.supportedFormats.join(', ')}
          </p>
        )}
      </div>
    );
  }

  return (
    <div className="w-full space-y-3">
      <div className="flex items-center justify-between">
        <div className="text-xs uppercase tracking-wider text-[hsl(var(--launcher-text-muted))] flex items-center gap-2">
          <Box className="w-3.5 h-3.5" />
          <span>Compatible Models</span>
        </div>
        {isRefreshing && <Loader2 className="w-3.5 h-3.5 animate-spin text-[hsl(var(--text-secondary))]" />}
      </div>

      {error && (
        <div className="flex items-center gap-2 px-3 py-2 rounded bg-[hsl(var(--accent-error)/0.1)] text-[hsl(var(--accent-error))]">
          <AlertCircle className="w-4 h-4" />
          <span className="text-sm">{error}</span>
        </div>
      )}

      <div className="space-y-1.5 max-h-64 overflow-y-auto">
        {filteredModels.slice(0, 20).map((model) => {
          const modelName = model.name;
          const isLoaded = isModelLoaded(modelName);
          const isLoading = loadingModel === modelName;
          const isUnloading = unloadingModel === modelName;
          const isBusy = isLoading || isUnloading;

          return (
            <div
              key={model.id || modelName}
              className="flex items-center justify-between gap-2 px-3 py-2 rounded-lg bg-[hsl(var(--launcher-bg-secondary)/0.3)] border border-[hsl(var(--launcher-border)/0.3)] hover:bg-[hsl(var(--launcher-bg-secondary)/0.5)] transition-colors"
            >
              <div className="flex flex-col min-w-0">
                <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                  {modelName}
                </span>
                <span className="text-xs text-[hsl(var(--launcher-text-muted))]">
                  {model.category} {model.size ? `• ${formatSize(model.size)}` : ''}
                </span>
              </div>

              <Tooltip
                content={isLoaded ? 'Unload model' : 'Load model'}
                position="left"
              >
                <button
                  onClick={() =>
                    isLoaded ? handleUnloadModel(modelName) : handleLoadModel(modelName)
                  }
                  disabled={isBusy}
                  className={`p-1.5 rounded transition-colors ${
                    isLoaded
                      ? 'bg-[hsl(var(--accent-success)/0.15)] text-[hsl(var(--accent-success))] hover:bg-[hsl(var(--accent-success)/0.25)]'
                      : 'bg-[hsl(var(--surface-interactive))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))] hover:text-[hsl(var(--text-primary))]'
                  } disabled:opacity-50 disabled:cursor-not-allowed`}
                >
                  {isBusy ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : isLoaded ? (
                    <Square className="w-4 h-4" />
                  ) : (
                    <Play className="w-4 h-4" />
                  )}
                </button>
              </Tooltip>
            </div>
          );
        })}
      </div>

      {filteredModels.length > 20 && (
        <p className="text-xs text-center text-[hsl(var(--text-muted))]">
          Showing 20 of {filteredModels.length} models
        </p>
      )}
    </div>
  );
}

function getModelFormat(model: SelectorModelInfo): string {
  // Try to get format from model path or name
  const path = model.path || model.name || '';
  const ext = path.split('.').pop()?.toLowerCase() || '';

  // Map common extensions to formats
  const formatMap: Record<string, string> = {
    gguf: 'gguf',
    ggml: 'ggml',
    safetensors: 'safetensors',
    bin: 'pytorch',
    pt: 'pytorch',
    pth: 'pytorch',
    onnx: 'onnx',
  };

  return formatMap[ext] || ext;
}

function formatSize(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`;
  return `${(bytes / 1e3).toFixed(1)} KB`;
}
