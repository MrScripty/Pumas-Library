import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ImportReviewStep } from './ImportReviewStep';
import type { ImportEntryStatus } from './useModelImportWorkflow';

function createEntry(overrides: Partial<ImportEntryStatus> = {}): ImportEntryStatus {
  return {
    path: '/models/tiny/model.safetensors',
    originPath: '/models/tiny/model.safetensors',
    filename: 'model.safetensors',
    kind: 'single_file',
    status: 'pending',
    securityTier: 'safe',
    securityAcknowledged: false,
    metadataStatus: 'found',
    validFileType: true,
    detectedFileType: 'safetensors',
    suggestedFamily: 'tiny',
    suggestedOfficialName: 'Tiny Model',
    modelType: 'llm',
    hfMetadata: {
      repo_id: 'acme/tiny-model',
      official_name: 'Tiny Model',
      family: 'tiny',
      model_type: 'llm',
      match_confidence: 1,
      match_method: 'hash',
    },
    ...overrides,
  };
}

describe('ImportReviewStep', () => {
  it('surfaces package evidence before final import', () => {
    render(
      <ImportReviewStep
        blockedFindings={[]}
        classificationError={null}
        containerFindings={[]}
        entries={[createEntry()]}
        pickleFilesCount={0}
        removeEntry={vi.fn()}
        shardedSets={[]}
        standaloneEntries={[createEntry()]}
        toggleSecurityAck={vi.fn()}
        toggleShardedSet={vi.fn()}
      />
    );

    expect(screen.getByText('Format')).toBeInTheDocument();
    expect(screen.getByText('safetensors')).toBeInTheDocument();
    expect(screen.getByText('HF')).toBeInTheDocument();
    expect(screen.getByText('acme/tiny-model')).toBeInTheDocument();
  });
});
