import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ModelMetadataModalFrame } from './ModelMetadataModalFrame';

describe('ModelMetadataModalFrame', () => {
  it('renders a named dialog and closes from the backdrop or Escape key', () => {
    const onClose = vi.fn();

    render(
      <ModelMetadataModalFrame
        isLoading={false}
        isRefetching={false}
        modelName="Test Model"
        onClose={onClose}
        onRefetch={vi.fn()}
      >
        <div>Metadata content</div>
      </ModelMetadataModalFrame>
    );

    expect(screen.getByRole('dialog', { name: 'Test Model' })).toBeInTheDocument();
    expect(screen.getByText('Metadata content')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Close metadata modal' }));
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('disables refetch while loading or refetching', () => {
    render(
      <ModelMetadataModalFrame
        isLoading={true}
        isRefetching={false}
        modelName="Test Model"
        onClose={vi.fn()}
        onRefetch={vi.fn()}
      >
        <div>Metadata content</div>
      </ModelMetadataModalFrame>
    );

    expect(
      screen.getByRole('button', { name: 'Refetch metadata from HuggingFace' })
    ).toBeDisabled();
  });
});
