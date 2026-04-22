import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { LocalModelNameButton } from './LocalModelNameButton';

describe('LocalModelNameButton', () => {
  it('opens metadata only for modified clicks', () => {
    const onOpenMetadata = vi.fn();

    render(
      <LocalModelNameButton
        modelId="llm/test"
        modelName="Test Model"
        hasIntegrityIssue={false}
        isDownloading={false}
        isLinked={true}
        isPartialDownload={false}
        onOpenMetadata={onOpenMetadata}
      />
    );

    const button = screen.getByRole('button', { name: /test model/i });
    fireEvent.click(button);
    expect(onOpenMetadata).not.toHaveBeenCalled();

    fireEvent.click(button, { ctrlKey: true });
    expect(onOpenMetadata).toHaveBeenCalledWith('llm/test', 'Test Model');
  });

  it('renders local model row badges with explanatory titles', () => {
    render(
      <LocalModelNameButton
        modelId="llm/test"
        modelName="Test Model"
        hasIntegrityIssue={true}
        integrityIssueMessage="Missing expected shard."
        isDownloading={false}
        isLinked={true}
        isPartialDownload={true}
        onOpenMetadata={vi.fn()}
        wasDequantized={true}
      />
    );

    expect(screen.getByText('DQ')).toHaveAttribute(
      'title',
      'Dequantized from quantized GGUF - may have reduced precision'
    );
    expect(screen.getByText('ISSUE')).toHaveAttribute(
      'title',
      'Missing expected shard.'
    );
    expect(screen.getByText('PARTIAL')).toHaveAttribute(
      'title',
      'Partial download detected - some expected files are missing'
    );
  });
});
