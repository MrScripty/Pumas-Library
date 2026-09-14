import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { VersionSelector } from './VersionSelector';

describe('VersionSelector popup', () => {
  it('shows a failed switch and keeps the active version selected', async () => {
    render(<VersionSelector
      activeVersion="v1.0.0" installedVersions={['v1.0.0', 'v1.1.0']}
      isLoading={false} onOpenVersionManager={vi.fn()}
      openActiveInstall={vi.fn().mockResolvedValue(true)}
      switchVersion={vi.fn().mockRejectedValue(new Error('Stop the running runtime first'))}
    />);
    fireEvent.click(screen.getByRole('button', { name: 'v1.0.0' }));
    fireEvent.click(screen.getByRole('button', { name: 'Switch to v1.1.0' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Stop the running runtime first');
    expect(screen.getByRole('button', { name: 'v1.0.0' })).toBeInTheDocument();
  });

  it('exposes a named action dialog and restores the version trigger after Escape', async () => {
    render(
      <VersionSelector
        activeVersion="v1.0.0"
        installedVersions={['v1.0.0', 'v1.1.0']}
        isLoading={false}
        onOpenVersionManager={vi.fn()}
        openActiveInstall={vi.fn().mockResolvedValue(true)}
        switchVersion={vi.fn().mockResolvedValue(true)}
      />
    );

    const trigger = screen.getByRole('button', { name: 'v1.0.0' });
    trigger.focus();
    fireEvent.click(trigger);
    const popup = screen.getByRole('dialog', { name: 'Version actions' });
    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    expect(trigger).toHaveAttribute('aria-controls', popup.id);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Switch to v1.0.0' })).toHaveFocus();
    });

    fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Version actions' })).not.toBeInTheDocument();
    });
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    expect(trigger).toHaveFocus();
  });
});
