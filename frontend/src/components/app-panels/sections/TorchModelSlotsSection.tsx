/**
 * Torch Model Slots Section
 *
 * Shows safetensors models from the library that can be loaded into
 * the Torch inference server. Supports multi-model loading with
 * per-model device selection (CPU, CUDA, MPS).
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { Box, Loader2, Play, Square, AlertCircle, Cpu, Monitor, Trash2 } from 'lucide-react';
import { api, isAPIAvailable } from '../../../api/adapter';
import type { ModelCategory } from '../../../types/apps';
import type { TorchModelSlot, TorchDeviceInfo } from '../../../types/api';
import { Tooltip } from '../../ui';
import { getLogger } from '../../../utils/logger';

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
      if (slotsResult.success && slotsResult.slots) {
        setSlots(slotsResult.slots);
      }
      if (devicesResult.success && devicesResult.devices) {
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
      fetchTorchState();
      const interval = setInterval(fetchTorchState, 5000);
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

      {/* Active model slots */}
      {slots.length > 0 && (
        <div className="space-y-3">
          <div className="text-xs uppercase tracking-wider text-[hsl(var(--launcher-text-muted))] flex items-center gap-2">
            <Monitor className="w-3.5 h-3.5" />
            <span>Active Model Slots</span>
            {isRefreshing && (
              <Loader2 className="w-3.5 h-3.5 animate-spin text-[hsl(var(--text-secondary))]" />
            )}
          </div>

          <div className="space-y-1.5 max-h-48 overflow-y-auto">
            {slots.map((slot) => {
              const isUnloading = unloadingSlot === slot.slot_id;

              return (
                <div
                  key={slot.slot_id}
                  className="flex items-center justify-between gap-2 px-3 py-2 rounded-lg bg-[hsl(var(--launcher-bg-secondary)/0.3)] border border-[hsl(var(--launcher-border)/0.3)] hover:bg-[hsl(var(--launcher-bg-secondary)/0.5)] transition-colors"
                >
                  <div className="flex flex-col min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                        {slot.model_name}
                      </span>
                      <SlotStateBadge state={slot.state} />
                      <DeviceBadge device={slot.device} />
                    </div>
                    <span className="text-xs text-[hsl(var(--launcher-text-muted))]">
                      {slot.model_type || 'unknown'}
                      {slot.gpu_memory_bytes ? ` \u2022 ${formatSize(slot.gpu_memory_bytes)} VRAM` : ''}
                      {slot.ram_memory_bytes ? ` \u2022 ${formatSize(slot.ram_memory_bytes)} RAM` : ''}
                    </span>
                  </div>

                  <Tooltip content="Unload model" position="left">
                    <button
                      onClick={() => handleUnload(slot.slot_id)}
                      disabled={isUnloading || slot.state === 'loading' || slot.state === 'unloading'}
                      className="p-1.5 rounded transition-colors bg-[hsl(var(--accent-error)/0.15)] text-[hsl(var(--accent-error))] hover:bg-[hsl(var(--accent-error)/0.25)] disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {isUnloading || slot.state === 'unloading' ? (
                        <Loader2 className="w-4 h-4 animate-spin" />
                      ) : (
                        <Square className="w-4 h-4" />
                      )}
                    </button>
                  </Tooltip>
                </div>
              );
            })}
          </div>

          {/* Per-device memory summary */}
          {devices.length > 0 && (
            <div className="space-y-1">
              {devices.filter(d => d.is_available).map(device => {
                const usedOnDevice = slots
                  .filter(s => s.device === device.device_id && s.state === 'ready')
                  .reduce((sum, s) => sum + (s.gpu_memory_bytes || s.ram_memory_bytes || 0), 0);
                const percent = device.memory_total > 0
                  ? Math.round((usedOnDevice / device.memory_total) * 100)
                  : 0;

                return (
                  <div key={device.device_id} className="flex items-center gap-2 text-xs text-[hsl(var(--launcher-text-muted))]">
                    <span className="w-16 truncate">{device.device_id}</span>
                    <div className="flex-1 h-1.5 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded-full overflow-hidden">
                      <div
                        className="h-full bg-[hsl(var(--accent-primary)/0.6)] rounded-full transition-all"
                        style={{ width: `${percent}%` }}
                      />
                    </div>
                    <span className="w-20 text-right">
                      {formatSize(usedOnDevice)} / {formatSize(device.memory_total)}
                    </span>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

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
                      {model.size ? ` \u2022 ${formatSize(model.size)}` : ''}
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

function SlotStateBadge({ state }: { state: string }) {
  const styles: Record<string, string> = {
    ready: 'bg-[hsl(var(--accent-success)/0.15)] text-[hsl(var(--accent-success))]',
    loading: 'bg-[hsl(var(--accent-warning)/0.15)] text-[hsl(var(--accent-warning))]',
    unloading: 'bg-[hsl(var(--accent-warning)/0.15)] text-[hsl(var(--accent-warning))]',
    error: 'bg-[hsl(var(--accent-error)/0.15)] text-[hsl(var(--accent-error))]',
    unloaded: 'bg-[hsl(var(--launcher-bg-secondary)/0.5)] text-[hsl(var(--launcher-text-muted))]',
  };

  return (
    <span className={`shrink-0 px-1.5 py-0.5 text-[10px] font-medium rounded ${styles[state] || styles.unloaded}`}>
      {state.toUpperCase()}
    </span>
  );
}

function DeviceBadge({ device }: { device: string }) {
  const isGpu = device.startsWith('cuda') || device === 'mps';

  return (
    <span className={`shrink-0 px-1.5 py-0.5 text-[10px] font-medium rounded ${
      isGpu
        ? 'bg-[hsl(var(--accent-success)/0.1)] text-[hsl(var(--accent-success))]'
        : 'bg-[hsl(var(--accent-primary)/0.1)] text-[hsl(var(--accent-primary))]'
    }`}>
      {device.toUpperCase()}
    </span>
  );
}

/** Check if a model path suggests it contains a safetensors file. */
function hasSafetensorsFile(path: string): boolean {
  const lower = path.toLowerCase();
  return lower.endsWith('.safetensors') || lower.includes('/safetensors/') || lower.includes('safetensors');
}

function formatSize(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`;
  if (bytes >= 1e3) return `${(bytes / 1e3).toFixed(1)} KB`;
  return `${bytes} B`;
}
