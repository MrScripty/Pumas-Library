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

  it('records a chosen category and reports it as the person\'s choice', async () => {
    const onChanged = vi.fn();
    setModelCategoryMock.mockResolvedValue({ success: true, model_id: 'llm/x/kev', category: 'decision' });
    render(<ModelCategoryControl modelId="llm/x/kev" category="llm" resolutionSource="model-type-resolver-arch-rules" onChanged={onChanged} />);

    expect(screen.getByRole('combobox')).toHaveValue('');
    expect(screen.getByText('Detected: Language models')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'decision' } });

    await waitFor(() => expect(onChanged).toHaveBeenCalledWith('decision', 'user'));
    expect(setModelCategoryMock).toHaveBeenCalledWith('llm/x/kev', 'decision');
  });

  it('clears a choice to re-detect, and shows refusal reasons', async () => {
    const onChanged = vi.fn();
    setModelCategoryMock.mockRejectedValueOnce(new Error('category must be one of llm, vlm'));
    render(<ModelCategoryControl modelId="llm/x/kev" category="vision" resolutionSource="user" onChanged={onChanged} />);

    expect(screen.getByRole('combobox')).toHaveValue('vision');
    expect(screen.getByText('Chosen by you; re-scans keep it.')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('combobox'), { target: { value: '' } });
    expect(await screen.findByRole('alert')).toHaveTextContent('category must be one of llm, vlm');
    expect(setModelCategoryMock).toHaveBeenCalledWith('llm/x/kev', null);
    expect(onChanged).not.toHaveBeenCalled();
  });
});
