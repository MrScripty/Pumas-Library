import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ModelMetadataModal } from './ModelMetadataModal';

const {
  getInferenceSettingsMock,
  getLibraryModelMetadataMock,
  resolveModelPackageFactsMock,
} = vi.hoisted(() => ({
  getInferenceSettingsMock: vi.fn(),
  getLibraryModelMetadataMock: vi.fn(),
  resolveModelPackageFactsMock: vi.fn(),
}));

vi.mock('../api/models', () => ({
  modelsAPI: {
    getInferenceSettings: getInferenceSettingsMock,
    getLibraryModelMetadata: getLibraryModelMetadataMock,
    refetchMetadataFromHF: vi.fn(),
    resolveModelPackageFacts: resolveModelPackageFactsMock,
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

  it('lazy loads read-only execution facts when the execution tab is selected', async () => {
    getLibraryModelMetadataMock.mockResolvedValue({
      success: true,
      stored_metadata: { model_id: 'model-1' },
      embedded_metadata: null,
      primary_file: null,
      component_manifest: [],
    });
    getInferenceSettingsMock.mockResolvedValue({
      success: true,
      inference_settings: [],
    });
    resolveModelPackageFactsMock.mockResolvedValue({
      package_facts_contract_version: 2,
      model_ref: { model_id: 'model-1' },
      artifact: {
        artifact_kind: 'hf_compatible_directory',
        entry_path: 'model.safetensors',
        storage_kind: 'library_owned',
        validation_state: 'valid',
      },
      components: [],
      transformers: {
        config_status: 'present',
        config_model_type: 'llama',
        generation_config_status: 'present',
      },
      task: { task_type_primary: 'text_generation' },
      generation_defaults: { status: 'present' },
      custom_code: { requires_custom_code: false },
      backend_hints: { accepted: ['transformers'] },
    });

    render(<ModelMetadataModal modelId="model-1" modelName="Test Model" onClose={vi.fn()} />);

    expect(resolveModelPackageFactsMock).not.toHaveBeenCalled();

    fireEvent.click(await screen.findByRole('button', { name: 'Execution Facts' }));

    expect(resolveModelPackageFactsMock).toHaveBeenCalledWith('model-1');
    expect(await screen.findByText('Package Facts Contract Version')).toBeInTheDocument();
    expect(screen.getByText('1')).toBeInTheDocument();
    expect(screen.getByText('Artifact')).toBeInTheDocument();
  });
});
