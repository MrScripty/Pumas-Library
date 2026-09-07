import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelConversionDialog, formatConversionDirection } from './ModelConversionDialog';
import type { ConversionProgress, ListConversionsResponse } from '../types/api-conversion';
import type { ModelInfo } from '../types/apps';

const bridge = vi.hoisted(() => ({
  check_conversion_environment: vi.fn(), list_model_conversions: vi.fn(),
  setup_conversion_environment: vi.fn(), start_model_conversion: vi.fn(), cancel_model_conversion: vi.fn(),
}));
vi.mock('../api/adapter', () => ({ api: bridge }));

const model: ModelInfo = { id: 'llm/source', name: 'Source model', category: 'llm', primaryFormat: 'gguf' };
function progress(status: ConversionProgress['status']): ConversionProgress {
  return { conversionId: 'conversion-1', sourceModelId: model.id, direction: 'gguf_to_safetensors',
    status, progress: 0.25, targetQuant: 'F16', currentTensor: null, tensorsCompleted: null,
    tensorsTotal: null, bytesWritten: null, estimatedOutputSize: null, outputModelId: null,
    error: null, pipelineStep: null, pipelineStepsTotal: null, pipelineStepLabel: null };
}

describe('ModelConversionDialog', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    bridge.check_conversion_environment.mockResolvedValue({ success: true, ready: false });
    bridge.list_model_conversions.mockResolvedValue({ success: true, conversions: [] });
    bridge.setup_conversion_environment.mockResolvedValue({ success: true });
    bridge.start_model_conversion.mockResolvedValue({ success: true, conversion_id: 'conversion-1' });
    bridge.cancel_model_conversion.mockResolvedValue({ success: true, cancelled: true });
  });

  it('requires explicit setup consent then sends the selected source and F16 direction', async () => {
    const user = userEvent.setup();
    render(<ModelConversionDialog model={model} direction="gguf_to_safetensors" onClose={vi.fn()} />);
    const setup = await screen.findByRole('button', { name: 'Install conversion tools' });
    expect(bridge.setup_conversion_environment).not.toHaveBeenCalled();
    expect(bridge.start_model_conversion).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByText(/does not restore precision/)).toBeVisible());
    expect(screen.getByRole('button', { name: 'Start conversion' })).toBeDisabled();
    bridge.check_conversion_environment.mockResolvedValue({ success: true, ready: true });
    await user.click(setup);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Start conversion' })).toBeEnabled());
    bridge.list_model_conversions.mockResolvedValue({ success: true, conversions: [progress('converting')] });
    await user.click(screen.getByRole('button', { name: 'Start conversion' }));
    expect(bridge.setup_conversion_environment).toHaveBeenCalledTimes(1);
    expect(bridge.start_model_conversion).toHaveBeenCalledWith(model.id, 'gguf_to_safetensors', 'F16');
    expect(await screen.findByRole('progressbar', { name: 'Conversion progress' })).toHaveAttribute('value', '0.25');
    expect(screen.getByRole('button', { name: 'Start conversion' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Cancel conversion' }));
    expect(bridge.cancel_model_conversion).toHaveBeenCalledWith('conversion-1');
    expect(screen.queryByText('Cancelled')).not.toBeInTheDocument();
  });

  it('exposes unavailable reads and refresh without showing guessed readiness', async () => {
    bridge.list_model_conversions.mockRejectedValueOnce(new Error('private transport detail'));
    render(<ModelConversionDialog model={model} direction="gguf_to_safetensors" onClose={vi.fn()} />);
    expect(await screen.findByRole('alert')).not.toHaveTextContent('private transport detail');
    expect(screen.getByRole('button', { name: 'Start conversion' })).toBeDisabled();
    await userEvent.click(screen.getByRole('button', { name: 'Refresh status' }));
    expect(await screen.findByRole('button', { name: 'Install conversion tools' })).toBeEnabled();
  });

  it('reopens existing backend work without starting another conversion', async () => {
    bridge.check_conversion_environment.mockResolvedValue({ success: true, ready: true });
    const listing: ListConversionsResponse = { success: true, conversions: [progress('converting')] };
    bridge.list_model_conversions.mockResolvedValue(listing);
    const close = vi.fn();
    render(<ModelConversionDialog model={model} direction="gguf_to_safetensors" onClose={close} />);
    expect(await screen.findByText('Converting')).toBeVisible();
    expect(screen.getByRole('button', { name: 'Start conversion' })).toBeDisabled();
    await userEvent.keyboard('{Escape}');
    expect(close).toHaveBeenCalledOnce();
    expect(bridge.start_model_conversion).not.toHaveBeenCalled();
    expect(bridge.cancel_model_conversion).not.toHaveBeenCalled();
  });

  it('maps only the supported format directions', () => {
    expect(formatConversionDirection('gguf')).toBe('gguf_to_safetensors');
    expect(formatConversionDirection('safetensors')).toBe('safetensors_to_gguf');
    expect(formatConversionDirection('onnx')).toBeNull();
    expect(formatConversionDirection(undefined)).toBeNull();
  });
});
