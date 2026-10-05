import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { S3ModelImportDialog } from './S3ModelImportDialog';
import type { S3ImportOutcome } from '../generated/desktop-contract';
const hook = vi.hoisted(() => ({ snapshot: null as S3ImportOutcome | null, error: null,
  commandBusy: false, start: vi.fn(), cancel: vi.fn(), observeAgain: vi.fn() }));
vi.mock('../hooks/useS3ModelImport', () => ({ useS3ModelImport: () => hook }));
describe('S3 import dialog content and commands (DOM fixture)', () => {
  beforeEach(() => { vi.clearAllMocks(); hook.snapshot = { status: 'idle' }; });
  it('offers only explicit anonymous pins and labelled source fields', () => {
    render(<S3ModelImportDialog onClose={vi.fn()} />);
    expect(screen.getByRole('dialog', { name: 'Import from S3' })).toBeInTheDocument();
    for (const label of ['HTTPS endpoint origin', 'Immutable VersionId', 'Expected SHA-256', 'Exact object key']) {
      expect(screen.getByLabelText(label)).toBeRequired();
    }
    expect(screen.queryByLabelText(/credential|secret|access key|token/i)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Import pinned object' })).toBeEnabled();
  });
  it('shows byte observations without completion claims and closes without cancelling owned work', () => {
    hook.snapshot = { status: 'running', operation_id: 'fixture-id', progress: {
      phase: 'finalizing', downloaded_for_current_file: '18446744073709551615',
    } };
    const close = vi.fn();
    render(<S3ModelImportDialog onClose={close} />);
    expect(screen.getByText(/18446744073709551615 bytes observed/)).toHaveTextContent('does not prove verification');
    expect(screen.getByRole('button', { name: 'Cancel import' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(close).toHaveBeenCalledTimes(1);
    expect(hook.cancel).not.toHaveBeenCalled();
  });
  it('blocks replay of retained cancellation and reports an unavailable backend', () => {
    hook.snapshot = { status: 'finished', operation_id: 'fixture-id', result: { status: 'cancelled', retained_work: true } };
    const view = render(<S3ModelImportDialog onClose={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Import pinned object' })).toBeDisabled();
    expect(screen.getByText(/Reconcile retained work/)).toBeInTheDocument();
    hook.snapshot = { status: 'unavailable' };
    view.rerender(<S3ModelImportDialog onClose={vi.fn()} />);
    expect(screen.getByText(/backend built with the S3 feature/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Import pinned object' })).toBeDisabled();
  });
});
