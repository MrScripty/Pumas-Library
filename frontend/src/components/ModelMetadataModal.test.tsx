import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ModelMetadataModal } from './ModelMetadataModal';

const {
  getInferenceSettingsMock,
  getLibraryModelMetadataMock,
} = vi.hoisted(() => ({
  getInferenceSettingsMock: vi.fn(),
  getLibraryModelMetadataMock: vi.fn(),
}));

vi.mock('../api/models', () => ({
  modelsAPI: {
    getInferenceSettings: getInferenceSettingsMock,
    getLibraryModelMetadata: getLibraryModelMetadataMock,
    refetchMetadataFromHF: vi.fn(),
    updateInferenceSettings: vi.fn(),
    updateModelNotes: vi.fn(),
  },
}));

describe('ModelMetadataModal', () => {
  it('renders as a named dialog and closes from the backdrop or Escape key', () => {
    const onClose = vi.fn();
    getLibraryModelMetadataMock.mockResolvedValue({
      success: true,
      stored_metadata: null,
      embedded_metadata: null,
      primary_file: null,
      component_manifest: [],
    });
    getInferenceSettingsMock.mockResolvedValue({
      success: true,
      inference_settings: [],
    });

    render(<ModelMetadataModal modelId="model-1" modelName="Test Model" onClose={onClose} />);

    expect(screen.getByRole('dialog', { name: 'Test Model' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Close metadata modal' }));
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
