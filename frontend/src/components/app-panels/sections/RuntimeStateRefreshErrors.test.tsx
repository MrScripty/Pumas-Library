import { act, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { OllamaModelSection } from './OllamaModelSection';
import { TorchModelSlotsSection } from './TorchModelSlotsSection';

const { apiMock, isApiAvailableMock } = vi.hoisted(() => ({
  apiMock: {
    ollama_list_models: vi.fn(),
    ollama_list_running: vi.fn(),
    torch_list_slots: vi.fn(),
    torch_list_devices: vi.fn(),
  },
  isApiAvailableMock: vi.fn<() => boolean>(),
}));

vi.mock('../../../api/adapter', () => ({
  api: apiMock,
  getElectronAPI: () => undefined,
  isAPIAvailable: isApiAvailableMock,
}));

describe('runtime state refresh failures', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
    apiMock.ollama_list_models.mockResolvedValue({ success: true, models: [] });
    apiMock.ollama_list_running.mockResolvedValue({ success: true, models: [] });
    apiMock.torch_list_slots.mockResolvedValue({ success: true, slots: [] });
    apiMock.torch_list_devices.mockResolvedValue({ success: true, devices: [] });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('shows a thrown Ollama refresh failure instead of an empty model list', async () => {
    apiMock.ollama_list_models.mockRejectedValueOnce(new Error('connection refused'));
    render(<OllamaModelSection connectionUrl="http://127.0.0.1:11434" isRunning={true} modelGroups={[]} />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Could not refresh Ollama state: connection refused');
  });

  it('shows an unsuccessful Ollama refresh with the backend reason', async () => {
    apiMock.ollama_list_running.mockResolvedValueOnce({ success: false, error: 'ps endpoint timed out', models: [] });
    render(<OllamaModelSection connectionUrl="http://127.0.0.1:11434" isRunning={true} modelGroups={[]} />);

    expect(await screen.findByRole('alert'))
      .toHaveTextContent('Could not refresh Ollama running models: ps endpoint timed out');
  });

  it('shows a Torch refresh failure and clears it after the next successful poll', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    apiMock.torch_list_slots.mockRejectedValueOnce(new Error('server restarting'));
    render(<TorchModelSlotsSection connectionUrl="http://127.0.0.1:8000" isRunning={true} modelGroups={[]} />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Could not refresh Torch state: server restarting');

    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
    expect(apiMock.torch_list_slots).toHaveBeenCalledTimes(2);
  });

  it('shows an unsuccessful Torch device listing with the backend reason', async () => {
    apiMock.torch_list_devices.mockResolvedValueOnce({ success: false, error: 'CUDA probe failed', devices: [] });
    render(<TorchModelSlotsSection connectionUrl="http://127.0.0.1:8000" isRunning={true} modelGroups={[]} />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Could not refresh Torch devices: CUDA probe failed');
  });
});
