import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
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
  beforeEach(() => vi.resetAllMocks());
  it('renders as a named dialog and closes from the backdrop or Escape key', async () => {
    const onClose = vi.fn();
    getLibraryModelMetadataMock.mockResolvedValue({
      success: true,
      model_id: 'model-1',
      stored_metadata: null,
      embedded_metadata: null,
      primary_file: null,
      component_manifest: [],
    });
    getInferenceSettingsMock.mockResolvedValue({
      success: true,
      model_id: 'model-1',
      inference_settings: [],
    });

    render(<ModelMetadataModal modelId="model-1" modelName="Test Model" onClose={onClose} />);

    const dialog = screen.getByRole('dialog', { name: 'Test Model' });
    await screen.findByText('No embedded metadata available');
    const backdrop = dialog.parentElement?.querySelector<HTMLElement>('[data-modal-backdrop]');
    if (!backdrop) {
      throw new TypeError('Expected metadata modal backdrop');
    }

    fireEvent.mouseDown(backdrop);
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('lazy loads read-only execution facts when the execution tab is selected', async () => {
    getLibraryModelMetadataMock.mockResolvedValue({
      success: true,
      model_id: 'model-1',
      stored_metadata: { model_id: 'model-1' },
      embedded_metadata: null,
      primary_file: null,
      component_manifest: [],
    });
    getInferenceSettingsMock.mockResolvedValue({
      success: true,
      model_id: 'model-1',
      inference_settings: [],
    });
    resolveModelPackageFactsMock.mockResolvedValue({
      package_facts_contract_version: 3,
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
    expect(screen.getByText('3')).toBeInTheDocument();
    expect(screen.getByText('Artifact')).toBeInTheDocument();
  });

  it.each(['rejected', 'wrong-model'] as const)('shows %s settings as unavailable, not editable empty success', async (failure) => {
    getLibraryModelMetadataMock.mockResolvedValue(metadata('model-1'));
    if (failure === 'rejected') getInferenceSettingsMock.mockRejectedValue(new Error('read failed'));
    else getInferenceSettingsMock.mockResolvedValue({ success: true, model_id: 'other', inference_settings: [] });
    render(<ModelMetadataModal modelId="model-1" modelName="Model" onClose={vi.fn()} />);
    await screen.findByText('No embedded metadata available');
    fireEvent.click(screen.getByRole('button', { name: 'Inference' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Unable to load inference settings');
    expect(screen.queryByRole('button', { name: /Save/ })).not.toBeInTheDocument();
  });

  it.each(['success', 'failure'] as const)('does not apply superseded %s reads to a replacement model', async (outcome) => {
    const oldMetadata = deferred();
    const oldSettings = deferred();
    getLibraryModelMetadataMock.mockReturnValueOnce(oldMetadata.promise).mockResolvedValue(metadata('model-2'));
    getInferenceSettingsMock.mockReturnValueOnce(oldSettings.promise).mockResolvedValue({ success: true, model_id: 'model-2', inference_settings: [] });
    const { rerender } = render(<ModelMetadataModal modelId="model-1" modelName="First" onClose={vi.fn()} />);
    rerender(<ModelMetadataModal modelId="model-2" modelName="Second" onClose={vi.fn()} />);
    await screen.findByText('No embedded metadata available');
    await act(async () => {
      if (outcome === 'failure') {
        oldMetadata.reject(new Error('old metadata failed'));
        oldSettings.reject(new Error('old settings failed'));
      } else {
        oldMetadata.resolve({ ...metadata('model-1'), embedded_metadata: { file_type: 'gguf', metadata: { stale: 'Old content' } } });
        oldSettings.resolve({ success: true, model_id: 'model-1', inference_settings: [] });
      }
    });
    expect(screen.getByRole('dialog', { name: 'Second' })).toBeInTheDocument();
    expect(screen.getByText('No embedded metadata available')).toBeInTheDocument();
    expect(screen.queryByText('Old content')).not.toBeInTheDocument();
    expect(screen.queryByText('old metadata failed')).not.toBeInTheDocument();
  });

  it('resets loaded metadata and edit state when the model changes', async () => {
    getLibraryModelMetadataMock.mockResolvedValueOnce({ ...metadata('model-1'), embedded_metadata: { file_type: 'gguf', metadata: { old: 'Old content' } } }).mockResolvedValue(metadata('model-2'));
    getInferenceSettingsMock.mockResolvedValueOnce({ success: true, model_id: 'model-1', inference_settings: [] }).mockResolvedValue({ success: true, model_id: 'model-2', inference_settings: [] });
    const { rerender } = render(<ModelMetadataModal modelId="model-1" modelName="First" onClose={vi.fn()} />);
    await screen.findByText('Old content');
    fireEvent.click(screen.getByRole('button', { name: 'Inference' }));
    rerender(<ModelMetadataModal modelId="model-2" modelName="Second" onClose={vi.fn()} />);
    await screen.findByText('No embedded metadata available');
    expect(screen.queryByText('Old content')).not.toBeInTheDocument();
  });

  it('rejects metadata for a different model', async () => {
    getLibraryModelMetadataMock.mockResolvedValue(metadata('other'));
    getInferenceSettingsMock.mockResolvedValue({ success: true, model_id: 'model-1', inference_settings: [] });
    render(<ModelMetadataModal modelId="model-1" modelName="Model" onClose={vi.fn()} />);
    expect(await screen.findByText('Failed to load metadata')).toBeInTheDocument();
  });
});

function metadata(modelId: string) {
  return { success: true, model_id: modelId, stored_metadata: null, embedded_metadata: null, primary_file: null, component_manifest: [] };
}

function deferred() {
  let resolve!: (value: unknown) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<unknown>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}
