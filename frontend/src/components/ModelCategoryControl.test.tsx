import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelCategoryControl } from './ModelCategoryControl';

const { setModelCategoryMock } = vi.hoisted(() => ({
  setModelCategoryMock: vi.fn<(modelId: string, category: string | null) => Promise<{
    success: true; model_id: string; category: string;
  }>>(),
}));

vi.mock('../api/models', () => ({
  modelsAPI: { setModelCategory: setModelCategoryMock },
}));

describe('ModelCategoryControl', () => {
  beforeEach(() => setModelCategoryMock.mockReset());

  it('saves only on Apply, so stepping through options records nothing', async () => {
    const onChanged = vi.fn();
    setModelCategoryMock.mockResolvedValue({ success: true, model_id: 'llm/x/kev', category: 'decision' });
    render(<ModelCategoryControl modelId="llm/x/kev" category="llm" resolutionSource="model-type-resolver-arch-rules" onChanged={onChanged} />);

    const select = screen.getByRole('combobox', { name: 'Used as' });
    expect(select).toHaveValue('');
    expect(select).toHaveAccessibleDescription('Detected as Language models from the model files and source.');
    const apply = screen.getByRole('button', { name: 'Apply' });
    expect(apply).toBeDisabled();

    fireEvent.change(select, { target: { value: 'vision' } });
    fireEvent.change(select, { target: { value: 'decision' } });
    expect(setModelCategoryMock).not.toHaveBeenCalled();
    expect(select).not.toBeDisabled();

    fireEvent.click(apply);
    await waitFor(() => expect(onChanged).toHaveBeenCalledTimes(1));
    expect(setModelCategoryMock).toHaveBeenCalledTimes(1);
    expect(setModelCategoryMock).toHaveBeenCalledWith('llm/x/kev', 'decision');
    expect(screen.getByRole('status')).toHaveTextContent('Saved as Decision and evaluation.');
  });

  const detected = {
    modelId: 'llm/x/kev', category: 'llm', resolutionSource: 'model-type-resolver-arch-rules',
  };

  it('shows the category the backend saved once the metadata re-read arrives', async () => {
    const onChanged = vi.fn();
    setModelCategoryMock.mockResolvedValue({ success: true, model_id: 'llm/x/kev', category: 'decision' });
    const { rerender } = render(<ModelCategoryControl {...detected} onChanged={onChanged} />);
    const select = screen.getByRole('combobox', { name: 'Used as' });
    fireEvent.change(select, { target: { value: 'decision' } });
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    await waitFor(() => expect(onChanged).toHaveBeenCalledTimes(1));

    rerender(<ModelCategoryControl {...detected} category="embedding" resolutionSource="user" onChanged={onChanged} />);
    expect(select).toHaveValue('embedding');
    expect(screen.getByRole('button', { name: 'Apply' })).toBeDisabled();
  });

  it('keeps a newer pick made while the metadata re-read is in flight', async () => {
    const onChanged = vi.fn();
    setModelCategoryMock.mockResolvedValue({ success: true, model_id: 'llm/x/kev', category: 'decision' });
    const { rerender } = render(<ModelCategoryControl {...detected} onChanged={onChanged} />);
    const select = screen.getByRole('combobox', { name: 'Used as' });
    fireEvent.change(select, { target: { value: 'decision' } });
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    await waitFor(() => expect(onChanged).toHaveBeenCalledTimes(1));

    fireEvent.change(select, { target: { value: 'vision' } });
    rerender(<ModelCategoryControl {...detected} category="decision" resolutionSource="user" onChanged={onChanged} />);
    expect(select).toHaveValue('vision');
    expect(screen.getByRole('button', { name: 'Apply' })).toBeEnabled();
  });

  it('clears a choice to re-detect, and shows refusal reasons', async () => {
    const onChanged = vi.fn();
    setModelCategoryMock.mockRejectedValueOnce(new Error('category must be one of llm, vlm'));
    render(<ModelCategoryControl modelId="llm/x/kev" category="vision" resolutionSource="user" onChanged={onChanged} />);

    const select = screen.getByRole('combobox', { name: 'Used as' });
    expect(select).toHaveValue('vision');
    expect(screen.getByText('Chosen by you: Vision. Re-scans keep it.')).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Detect automatically' })).toBeInTheDocument();
    fireEvent.change(select, { target: { value: '' } });
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('category must be one of llm, vlm');
    expect(setModelCategoryMock).toHaveBeenCalledWith('llm/x/kev', null);
    expect(onChanged).not.toHaveBeenCalled();
  });
});
