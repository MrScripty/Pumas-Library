import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ConfirmationDialog } from './ConfirmationDialog';

describe('ConfirmationDialog', () => {
  it('calls confirm and cancel handlers from dialog controls', () => {
    const onCancel = vi.fn();
    const onConfirm = vi.fn();

    render(
      <ConfirmationDialog
        isOpen={true}
        title="Confirm action"
        message="This action changes persisted state."
        confirmLabel="Continue"
        onCancel={onCancel}
        onConfirm={onConfirm}
      />
    );

    expect(screen.getByRole('alertdialog', { name: 'Confirm action' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    expect(onConfirm).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('cancels on Escape before parent key handlers observe the event', () => {
    const onCancel = vi.fn();
    const parentKeyHandler = vi.fn();

    window.addEventListener('keydown', parentKeyHandler);
    render(
      <ConfirmationDialog
        isOpen={true}
        title="Confirm action"
        message="This action changes persisted state."
        confirmLabel="Continue"
        onCancel={onCancel}
        onConfirm={vi.fn()}
      />
    );

    fireEvent.keyDown(window, { key: 'Escape' });

    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(parentKeyHandler).not.toHaveBeenCalled();
    window.removeEventListener('keydown', parentKeyHandler);
  });
});
