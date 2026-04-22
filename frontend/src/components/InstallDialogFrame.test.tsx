import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { InstallDialogFrame } from './InstallDialogFrame';

describe('InstallDialogFrame', () => {
  it('renders modal mode as a named dialog and closes from backdrop or Escape key', () => {
    const onClose = vi.fn();

    render(
      <InstallDialogFrame
        isOpen={true}
        isPageMode={false}
        onClose={onClose}
        title="Install ComfyUI Version"
      >
        <div>Install content</div>
      </InstallDialogFrame>
    );

    expect(screen.getByRole('dialog', { name: 'Install ComfyUI Version' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Dismiss install dialog' }));
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('renders page mode without modal dialog chrome', () => {
    render(
      <InstallDialogFrame
        isOpen={true}
        isPageMode={true}
        onClose={vi.fn()}
        title="Install ComfyUI Version"
      >
        <div>Install content</div>
      </InstallDialogFrame>
    );

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Dismiss install dialog' })).not.toBeInTheDocument();
    expect(screen.getByText('Install content')).toBeInTheDocument();
  });
});
