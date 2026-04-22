import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { LocalModelsEmptyState } from './LocalModelsEmptyState';

describe('LocalModelsEmptyState', () => {
  it('renders the default empty-library message without a picker action', () => {
    render(
      <LocalModelsEmptyState
        totalModels={0}
        hasFilters={false}
      />
    );

    expect(
      screen.getByText('No models found. Add models to your library to get started.')
    ).toBeInTheDocument();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('renders the existing-library picker action for an empty packaged library', () => {
    const onChooseExistingLibrary = vi.fn();

    render(
      <LocalModelsEmptyState
        totalModels={0}
        hasFilters={false}
        onChooseExistingLibrary={onChooseExistingLibrary}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /use existing library/i }));

    expect(screen.getByText('No library models found')).toBeInTheDocument();
    expect(onChooseExistingLibrary).toHaveBeenCalledTimes(1);
  });

  it('disables the picker action while a library picker is opening', () => {
    render(
      <LocalModelsEmptyState
        totalModels={0}
        hasFilters={false}
        onChooseExistingLibrary={vi.fn()}
        isChoosingExistingLibrary={true}
      />
    );

    expect(
      screen.getByRole('button', { name: /opening library picker/i })
    ).toBeDisabled();
  });

  it('renders and invokes the clear-filters action for empty filter results', () => {
    const onClearFilters = vi.fn();

    render(
      <LocalModelsEmptyState
        totalModels={3}
        hasFilters={true}
        onClearFilters={onClearFilters}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /clear filters/i }));

    expect(screen.getByText('No models match your filters.')).toBeInTheDocument();
    expect(onClearFilters).toHaveBeenCalledTimes(1);
  });
});
