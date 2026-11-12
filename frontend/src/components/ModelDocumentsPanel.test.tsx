import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelDocumentsPanel } from './ModelDocumentsPanel';

const { getModelDocumentsMock, backfillMock } = vi.hoisted(() => ({
  getModelDocumentsMock: vi.fn(),
  backfillMock: vi.fn(),
}));

vi.mock('../api/models', () => ({
  modelsAPI: {
    getModelDocuments: getModelDocumentsMock,
    backfillModelCompanionFiles: backfillMock,
  },
}));

const stored = {
  model_id: 'llm/acme/model',
  model_card: {
    path: 'README.md',
    size_bytes: 40,
    content: '---\nlicense: apache-2.0\n---\n# Acme model card\n',
  },
  licenses: [{ path: 'LICENSE', size_bytes: 18, content: 'Apache License 2.0' }],
  code_files: [{ path: 'modeling_acme.py', size_bytes: 22 }],
  requires_custom_code: true,
  custom_code_sources: ['modeling_acme.py'],
  backfill_available: true,
};

describe('ModelDocumentsPanel', () => {
  beforeEach(() => vi.resetAllMocks());

  it('renders the backend card, license and code without the card front matter', async () => {
    getModelDocumentsMock.mockResolvedValue(stored);
    render(<ModelDocumentsPanel modelId="llm/acme/model" />);
    expect(await screen.findByText('Acme model card')).toBeTruthy();
    expect(screen.queryByText(/license: apache-2.0/)).toBeNull();
    expect(screen.getByText('Apache License 2.0')).toBeTruthy();
    expect(screen.getByText(/modeling_acme\.py/)).toBeTruthy();
    expect(screen.getByText(/never executes/)).toBeTruthy();
  });

  it('plans a backfill first and fetches only when asked', async () => {
    getModelDocumentsMock.mockResolvedValue({ ...stored, model_card: undefined, licenses: [] });
    backfillMock.mockResolvedValue({
      dry_run: true,
      generated_at: '',
      file_count: 1,
      total_bytes: 40,
      skipped_models: 0,
      failed_models: 0,
      models: [
        {
          model_id: 'llm/acme/model',
          outcome: 'planned',
          revision: '0123456789abcdef0123456789abcdef01234567',
          files: [{ path: 'README.md', kind: 'model_card', size_bytes: 40 }],
        },
      ],
    });
    render(<ModelDocumentsPanel modelId="llm/acme/model" />);
    fireEvent.click(await screen.findByText('Check for missing card, license and code'));
    expect(await screen.findByText(/Would fetch 1 file/)).toBeTruthy();
    expect(backfillMock).toHaveBeenCalledWith('llm/acme/model', true);
    fireEvent.click(screen.getByText('Fetch them'));
    await screen.findByText(/Would fetch|Fetched/);
    expect(backfillMock).toHaveBeenLastCalledWith('llm/acme/model', false);
  });

  it('shows the backend reason when a model cannot be backfilled', async () => {
    getModelDocumentsMock.mockResolvedValue(stored);
    backfillMock.mockResolvedValue({
      dry_run: true,
      generated_at: '',
      file_count: 0,
      total_bytes: 0,
      skipped_models: 1,
      failed_models: 0,
      models: [
        {
          model_id: 'llm/acme/model',
          outcome: 'skipped',
          reason: 'revision_unavailable',
          message: 'Revision abc of acme/model no longer exists on Hugging Face',
          files: [],
        },
      ],
    });
    render(<ModelDocumentsPanel modelId="llm/acme/model" />);
    fireEvent.click(await screen.findByText('Check for missing card, license and code'));
    expect(await screen.findByText(/no longer exists on Hugging Face/)).toBeTruthy();
    expect(screen.queryByText('Fetch them')).toBeNull();
  });
});
