import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { TorchDeviceInfo, TorchModelSlot } from '../../../types/api';
import { TorchActiveSlots } from './TorchActiveSlots';
import { formatTorchModelSize } from './torchModelSlotFormatting';

const readySlot: TorchModelSlot = {
  slot_id: 'slot-1',
  model_name: 'Alpha',
  model_path: 'model-alpha',
  device: 'cuda:0',
  state: 'ready',
  gpu_memory_bytes: 2_000_000_000,
  ram_memory_bytes: 512_000_000,
  model_type: 'text',
};

const devices: TorchDeviceInfo[] = [
  {
    device_id: 'cuda:0',
    name: 'GPU 0',
    memory_total: 8_000_000_000,
    memory_available: 6_000_000_000,
    is_available: true,
  },
];

describe('TorchActiveSlots', () => {
  it('renders active slot badges and device memory usage', () => {
    render(
      <TorchActiveSlots
        devices={devices}
        isRefreshing={false}
        slots={[readySlot]}
        unloadingSlot={null}
        onUnload={vi.fn()}
      />
    );

    expect(screen.getByText('Active Model Slots')).toBeInTheDocument();
    expect(screen.getByText('Alpha')).toBeInTheDocument();
    expect(screen.getByText('READY')).toBeInTheDocument();
    expect(screen.getByText('CUDA:0')).toBeInTheDocument();
    expect(screen.getByText(/2\.0 GB VRAM/)).toBeInTheDocument();
    expect(screen.getByText('2.0 GB / 8.0 GB')).toBeInTheDocument();
  });

  it('calls the unload handler through a named button', async () => {
    const user = userEvent.setup();
    const onUnload = vi.fn();

    render(
      <TorchActiveSlots
        devices={devices}
        isRefreshing={false}
        slots={[readySlot]}
        unloadingSlot={null}
        onUnload={onUnload}
      />
    );

    await user.click(screen.getByRole('button', { name: 'Unload Alpha' }));

    expect(onUnload).toHaveBeenCalledWith('slot-1');
  });

  it('disables unload while the slot is loading', () => {
    render(
      <TorchActiveSlots
        devices={devices}
        isRefreshing={false}
        slots={[{ ...readySlot, state: 'loading' }]}
        unloadingSlot={null}
        onUnload={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: 'Unload Alpha' })).toBeDisabled();
  });
});

describe('formatTorchModelSize', () => {
  it('formats bytes with the expected display unit', () => {
    expect(formatTorchModelSize(500)).toBe('500 B');
    expect(formatTorchModelSize(1_500)).toBe('1.5 KB');
    expect(formatTorchModelSize(2_000_000)).toBe('2.0 MB');
    expect(formatTorchModelSize(3_000_000_000)).toBe('3.0 GB');
  });
});
