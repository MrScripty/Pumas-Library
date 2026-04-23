import { Loader2, Monitor, Square } from 'lucide-react';
import type { TorchDeviceInfo, TorchModelSlot } from '../../../types/api';
import { Tooltip } from '../../ui';
import { formatTorchModelSize } from './torchModelSlotFormatting';

interface TorchActiveSlotsProps {
  devices: TorchDeviceInfo[];
  isRefreshing: boolean;
  slots: TorchModelSlot[];
  unloadingSlot: string | null;
  onUnload: (slotId: string) => void;
}

export function TorchActiveSlots({
  devices,
  isRefreshing,
  slots,
  unloadingSlot,
  onUnload,
}: TorchActiveSlotsProps) {
  if (slots.length === 0) {
    return null;
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 text-xs uppercase tracking-wider text-[hsl(var(--launcher-text-muted))]">
        <Monitor className="h-3.5 w-3.5" />
        <span>Active Model Slots</span>
        {isRefreshing && (
          <Loader2 className="h-3.5 w-3.5 animate-spin text-[hsl(var(--text-secondary))]" />
        )}
      </div>

      <div className="max-h-48 space-y-1.5 overflow-y-auto">
        {slots.map((slot) => (
          <TorchActiveSlotRow
            key={slot.slot_id}
            isUnloading={unloadingSlot === slot.slot_id}
            slot={slot}
            onUnload={onUnload}
          />
        ))}
      </div>

      {devices.length > 0 && (
        <div className="space-y-1">
          {devices.filter((device) => device.is_available).map((device) => (
            <TorchDeviceMemoryRow key={device.device_id} device={device} slots={slots} />
          ))}
        </div>
      )}
    </div>
  );
}

interface TorchActiveSlotRowProps {
  isUnloading: boolean;
  slot: TorchModelSlot;
  onUnload: (slotId: string) => void;
}

function TorchActiveSlotRow({ isUnloading, slot, onUnload }: TorchActiveSlotRowProps) {
  const isBusy = isUnloading || slot.state === 'unloading';

  return (
    <div className="flex items-center justify-between gap-2 rounded-lg border border-[hsl(var(--launcher-border)/0.3)] bg-[hsl(var(--launcher-bg-secondary)/0.3)] px-3 py-2 transition-colors hover:bg-[hsl(var(--launcher-bg-secondary)/0.5)]">
      <div className="flex min-w-0 flex-col">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
            {slot.model_name}
          </span>
          <SlotStateBadge state={slot.state} />
          <DeviceBadge device={slot.device} />
        </div>
        <span className="text-xs text-[hsl(var(--launcher-text-muted))]">
          {slot.model_type || 'unknown'}
          {slot.gpu_memory_bytes ? ` \u2022 ${formatTorchModelSize(slot.gpu_memory_bytes)} VRAM` : ''}
          {slot.ram_memory_bytes ? ` \u2022 ${formatTorchModelSize(slot.ram_memory_bytes)} RAM` : ''}
        </span>
      </div>

      <Tooltip content="Unload model" position="left">
        <button
          aria-label={`Unload ${slot.model_name}`}
          onClick={() => onUnload(slot.slot_id)}
          disabled={isBusy || slot.state === 'loading'}
          className="rounded bg-[hsl(var(--accent-error)/0.15)] p-1.5 text-[hsl(var(--accent-error))] transition-colors hover:bg-[hsl(var(--accent-error)/0.25)] disabled:cursor-not-allowed disabled:opacity-50"
        >
          {isBusy ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Square className="h-4 w-4" />
          )}
        </button>
      </Tooltip>
    </div>
  );
}

function TorchDeviceMemoryRow({
  device,
  slots,
}: {
  device: TorchDeviceInfo;
  slots: TorchModelSlot[];
}) {
  const usedOnDevice = slots
    .filter((slot) => slot.device === device.device_id && slot.state === 'ready')
    .reduce((sum, slot) => sum + (slot.gpu_memory_bytes || slot.ram_memory_bytes || 0), 0);
  const percent = device.memory_total > 0
    ? Math.round((usedOnDevice / device.memory_total) * 100)
    : 0;

  return (
    <div className="flex items-center gap-2 text-xs text-[hsl(var(--launcher-text-muted))]">
      <span className="w-16 truncate">{device.device_id}</span>
      <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-[hsl(var(--launcher-bg-secondary)/0.5)]">
        <div
          className="h-full rounded-full bg-[hsl(var(--accent-primary)/0.6)] transition-all"
          style={{ width: `${percent}%` }}
        />
      </div>
      <span className="w-20 text-right">
        {formatTorchModelSize(usedOnDevice)} / {formatTorchModelSize(device.memory_total)}
      </span>
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
    <span className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium ${styles[state] || styles['unloaded']}`}>
      {state.toUpperCase()}
    </span>
  );
}

function DeviceBadge({ device }: { device: string }) {
  const isGpu = device.startsWith('cuda') || device === 'mps';

  return (
    <span className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium ${
      isGpu
        ? 'bg-[hsl(var(--accent-success)/0.1)] text-[hsl(var(--accent-success))]'
        : 'bg-[hsl(var(--accent-primary)/0.1)] text-[hsl(var(--accent-primary))]'
    }`}>
      {device.toUpperCase()}
    </span>
  );
}
