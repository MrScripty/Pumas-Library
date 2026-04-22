import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { InstallDialog } from './InstallDialog';

describe('InstallDialog', () => {
  it('renders modal mode as a named dialog and closes from backdrop or Escape key', () => {
    const onClose = vi.fn();

    render(
      <InstallDialog
        isOpen={true}
        onClose={onClose}
        availableVersions={[]}
        installedVersions={[]}
        isLoading={false}
        onInstallVersion={vi.fn().mockResolvedValue(true)}
        onRefreshAll={vi.fn().mockResolvedValue(undefined)}
        onRemoveVersion={vi.fn().mockResolvedValue(true)}
      />
    );

    expect(screen.getByRole('dialog', { name: 'Install ComfyUI Version' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Dismiss install dialog' }));
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
