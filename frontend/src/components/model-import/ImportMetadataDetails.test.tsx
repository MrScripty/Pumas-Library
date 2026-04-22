import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { ImportMetadataDetails } from './ImportMetadataDetails';
import type { ImportEntryStatus } from './modelImportWorkflowTypes';

const baseEntry: ImportEntryStatus = {
  path: '/imports/model.gguf',
  originPath: '/imports/model.gguf',
  filename: 'model.gguf',
  kind: 'single_file',
  status: 'pending',
  metadataStatus: 'found',
  hfMetadata: {
    repo_id: 'owner/model',
    official_name: 'Model',
    family: 'test-family',
    match_method: 'filename_exact',
    match_confidence: 0.9,
  },
  suggestedFamily: 'test-family',
  suggestedOfficialName: 'Model',
  modelType: 'llm',
  detectedFileType: 'gguf',
};

describe('ImportMetadataDetails', () => {
  it('renders filtered Hugging Face metadata fields', () => {
    render(
      <ImportMetadataDetails
        entry={baseEntry}
        isShowingAllEmbedded={false}
        isShowingEmbedded={false}
        onToggleMetadataSource={vi.fn()}
        onToggleShowAllEmbeddedMetadata={vi.fn()}
      />
    );

    expect(screen.getByText('Official Name')).toBeInTheDocument();
    expect(screen.getByText('Model')).toBeInTheDocument();
    expect(screen.getByText('Match Confidence')).toBeInTheDocument();
    expect(screen.getByText('90%')).toBeInTheDocument();
    expect(screen.queryByText('Repo Id')).not.toBeInTheDocument();
  });

  it('switches metadata source through the supplied callback', async () => {
    const user = userEvent.setup();
    const onToggleMetadataSource = vi.fn();

    render(
      <ImportMetadataDetails
        entry={baseEntry}
        isShowingAllEmbedded={false}
        isShowingEmbedded={false}
        onToggleMetadataSource={onToggleMetadataSource}
        onToggleShowAllEmbeddedMetadata={vi.fn()}
      />
    );

    await user.click(screen.getByRole('button', { name: /huggingface/i }));

    expect(onToggleMetadataSource).toHaveBeenCalledWith('/imports/model.gguf');
  });

  it('renders linked embedded metadata and hidden-field disclosure', async () => {
    const user = userEvent.setup();
    const onToggleShowAllEmbeddedMetadata = vi.fn();

    render(
      <ImportMetadataDetails
        entry={{
          ...baseEntry,
          embeddedMetadataStatus: 'loaded',
          embeddedMetadata: {
            'general.name': 'TinyModel',
            'general.quantized_by': 'builder',
            'tokenizer.chat_template': 'hidden template',
          },
        }}
        isShowingAllEmbedded={false}
        isShowingEmbedded={true}
        onToggleMetadataSource={vi.fn()}
        onToggleShowAllEmbeddedMetadata={onToggleShowAllEmbeddedMetadata}
      />
    );

    expect(screen.getByRole('link', { name: 'TinyModel' })).toHaveAttribute(
      'href',
      'https://huggingface.co/builder/TinyModel'
    );

    await user.click(screen.getByRole('button', { name: 'Show 1 more field' }));

    expect(onToggleShowAllEmbeddedMetadata).toHaveBeenCalledWith('/imports/model.gguf');
  });
});
