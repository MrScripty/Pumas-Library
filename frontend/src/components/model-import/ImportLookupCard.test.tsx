import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { ImportLookupCard } from './ImportLookupCard';
import type { ImportEntryStatus } from './modelImportWorkflowTypes';

const singleFileEntry: ImportEntryStatus = {
  path: '/imports/model.gguf',
  originPath: '/imports/model.gguf',
  filename: 'model.gguf',
  kind: 'single_file',
  status: 'pending',
  metadataStatus: 'found',
  hfMetadata: {
    repo_id: 'user/model',
    official_name: 'Model',
    family: 'test',
    match_method: 'filename_exact',
    match_confidence: 0.95,
  },
  suggestedFamily: 'test',
  suggestedOfficialName: 'Model',
  modelType: 'llm',
};

function renderCard(overrides: Partial<ImportEntryStatus> = {}) {
  const toggleMetadataExpand = vi.fn();
  const toggleMetadataSource = vi.fn();
  const toggleShowAllEmbeddedMetadata = vi.fn();
  const entry = { ...singleFileEntry, ...overrides };

  const view = render(
    <ImportLookupCard
      entry={entry}
      expandedMetadata={new Set(overrides.path ? [overrides.path] : [])}
      showEmbeddedMetadata={new Set()}
      showAllEmbeddedMetadata={new Set()}
      toggleMetadataExpand={toggleMetadataExpand}
      toggleMetadataSource={toggleMetadataSource}
      toggleShowAllEmbeddedMetadata={toggleShowAllEmbeddedMetadata}
    />
  );

  return { ...view, toggleMetadataExpand };
}

describe('ImportLookupCard', () => {
  it('uses a native expand button for metadata rows', async () => {
    const user = userEvent.setup();
    const { container, toggleMetadataExpand } = renderCard();

    expect(container.querySelector('[role="button"]')).not.toBeInTheDocument();

    const expandButton = screen.getByRole('button', {
      name: 'Expand metadata for model.gguf',
    });
    expect(expandButton).toHaveAttribute('aria-expanded', 'false');

    await user.click(expandButton);

    expect(toggleMetadataExpand).toHaveBeenCalledWith('/imports/model.gguf');
  });
});
