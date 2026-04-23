/**
 * Torch Model Slots Section
 *
 * Shows safetensors models from the library that can be loaded into
 * the Torch inference server. Supports multi-model loading with
 * per-model device selection (CPU, CUDA, MPS).
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { Box, Loader2, Play, AlertCircle } from 'lucide-react';
import { api, isAPIAvailable } from '../../../api/adapter';
import type { ModelCategory } from '../../../types/apps';
import type { TorchModelSlot, TorchDeviceInfo } from '../../../types/api';
import { Tooltip } from '../../ui';
import { getLogger } from '../../../utils/logger';
import { TorchActiveSlots } from './TorchActiveSlots';
import { formatTorchModelSize } from './torchModelSlotFormatting';

const logger = getLogger('TorchModelSlotsSection');

/** A library model that has a safetensors file. */
interface SafetensorsLibraryModel {
  id: string;
  name: string;
  category: string;
  size?: number;
}

export interface TorchModelSlotsSectionProps {
  connectionUrl: string;
  isRunning: boolean;
  modelGroups: ModelCategory[];
}

export function TorchModelSlotsSection({
  connectionUrl,
  isRunning,
  modelGroups,
}: TorchModelSlotsSectionProps) {
  const [slots, setSlots] = useState<TorchModelSlot[]>([]);
  const [devices, setDevices] = useState<TorchDeviceInfo[]>([]);
  const [loadingModel, setLoadingModel] = useState<string | null>(null);
  const [unloadingSlot, setUnloadingSlot] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [selectedDevices, setSelectedDevices] = useState<Record<string, string>>({});

  // Extract safetensors models from library model groups
  const safetensorsModels: SafetensorsLibraryModel[] = useMemo(() => {
    const models: SafetensorsLibraryModel[] = [];
    for (const group of modelGroups) {
      for (const model of group.models) {
        if (model.isPartialDownload) {
          continue;
        }
        const path = model.path || model.name || '';
        if (hasSafetensorsFile(path)) {
          models.push({
            id: model.id,
            name: model.name,
            category: group.category,
            size: model.size,
          });
        }
      }
    }
    return models;
  }, [modelGroups]);

  // Set of model names currently loaded
  const loadedModelIds = useMemo(
    () => new Set(slots.filter(s => s.state === 'ready' || s.state === 'loading').map(s => s.model_path)),
    [slots]
  );

  const fetchTorchState = useCallback(async () => {
    if (!isAPIAvailable() || !isRunning) {
      setSlots([]);
      setDevices([]);
      return;
    }

    try {
      setIsRefreshing(true);
      const [slotsResult, devicesResult] = await Promise.all([
        api.torch_list_slots(connectionUrl),
        api.torch_list_devices(connectionUrl),
      ]);
      if (slotsResult.success) {
        setSlots(slotsResult.slots);
      }
      if (devicesResult.success) {
        setDevices(devicesResult.devices);
      }
    } catch (err) {
      logger.debug('Failed to fetch Torch state', { error: err });
    } finally {
      setIsRefreshing(false);
    }
  }, [connectionUrl, isRunning]);

  useEffect(() => {
    if (isRunning) {
      void fetchTorchState();
      const interval = setInterval(() => void fetchTorchState(), 5000);
      return () => clearInterval(interval);
    }
    setSlots([]);
    setDevices([]);
    return undefined;
  }, [fetchTorchState, isRunning]);

  const handleLoad = async (model: SafetensorsLibraryModel) => {
    if (!isAPIAvailable()) return;

    setLoadingModel(model.id);
    setError(null);

    const device = selectedDevices[model.id] || 'auto';

    try {
      const result = await api.torch_load_model(model.id, device, connectionUrl);
      if (result.success) {
        await fetchTorchState();
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

  const handleUnload = async (slotId: string) => {
    if (!isAPIAvailable()) return;

    setUnloadingSlot(slotId);
    setError(null);

    try {
      const result = await api.torch_unload_model(slotId, connectionUrl);
      if (result.success) {
        await fetchTorchState();
      } else {
        setError(result.error || 'Failed to unload model');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      setError(message);
    } finally {
      setUnloadingSlot(null);
    }
  };

  const handleDeviceChange = (modelId: string, device: string) => {
    setSelectedDevices(prev => ({ ...prev, [modelId]: device }));
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

      <TorchActiveSlots
        devices={devices}
        isRefreshing={isRefreshing}
        slots={slots}
        unloadingSlot={unloadingSlot}
        onUnload={(slotId) => void handleUnload(slotId)}
      />

      {/* Library safetensors models available to load */}
      {safetensorsModels.length > 0 && (
        <div className="space-y-3">
          <div className="text-xs uppercase tracking-wider text-[hsl(var(--launcher-text-muted))] flex items-center gap-2">
            <Box className="w-3.5 h-3.5" />
            <span>Library SafeTensors Models</span>
          </div>

          <div className="space-y-1.5 max-h-64 overflow-y-auto">
            {safetensorsModels.map((model) => {
              const isLoading = loadingModel === model.id;
              const isAlreadyLoaded = loadedModelIds.has(model.id);

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
                      {model.size ? ` \u2022 ${formatTorchModelSize(model.size)}` : ''}
                    </span>
                  </div>

                  <div className="flex items-center gap-1.5">
                    {/* Device picker */}
                    <select
                      value={selectedDevices[model.id] || 'auto'}
                      onChange={(e) => handleDeviceChange(model.id, e.target.value)}
                      disabled={isLoading || isAlreadyLoaded}
                      className="text-xs px-1.5 py-1 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))] disabled:opacity-50"
                    >
                      <option value="auto">Auto</option>
                      <option value="cpu">CPU</option>
                      {devices.filter(d => d.device_id.startsWith('cuda')).map(d => (
                        <option key={d.device_id} value={d.device_id}>
                          {d.name || d.device_id}
                        </option>
                      ))}
                      {devices.some(d => d.device_id === 'mps') && (
                        <option value="mps">MPS</option>
                      )}
                    </select>

                    {/* Load button */}
                    <Tooltip content={isAlreadyLoaded ? 'Already loaded' : 'Load model'} position="left">
                      <button
                        onClick={() => handleLoad(model)}
                        disabled={isLoading || isAlreadyLoaded}
                        className={`p-1.5 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
                          isAlreadyLoaded
                            ? 'bg-[hsl(var(--accent-success)/0.15)] text-[hsl(var(--accent-success))]'
                            : 'bg-[hsl(var(--surface-interactive))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))] hover:text-[hsl(var(--text-primary))]'
                        }`}
                      >
                        {isLoading ? (
                          <Loader2 className="w-4 h-4 animate-spin" />
                        ) : (
                          <Play className="w-4 h-4" />
                        )}
                      </button>
                    </Tooltip>
                  </div>
                </div>
              );
            })}
          </div>

          {safetensorsModels.length > 20 && (
            <p className="text-xs text-center text-[hsl(var(--text-muted))]">
              Showing first 20 of {safetensorsModels.length} models
            </p>
          )}
        </div>
      )}

      {safetensorsModels.length === 0 && slots.length === 0 && (
        <div className="w-full py-4 text-center text-[hsl(var(--text-secondary))]">
          <Box className="w-6 h-6 mx-auto mb-2 opacity-50" />
          <p className="text-sm">No SafeTensors models in library</p>
          <p className="text-xs mt-1 text-[hsl(var(--text-muted))]">
            Download or import SafeTensors models to load them into Torch
          </p>
        </div>
      )}
    </div>
  );
}

/** Check if a model path suggests it contains a safetensors file. */
function hasSafetensorsFile(path: string): boolean {
  const lower = path.toLowerCase();
  return lower.endsWith('.safetensors') || lower.includes('/safetensors/') || lower.includes('safetensors');
}
