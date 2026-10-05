import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { S3ModelImportDialog } from './S3ModelImportDialog';
import type { S3ImportOutcome } from '../generated/desktop-contract';
const hook = vi.hoisted(() => ({ snapshot: null as S3ImportOutcome | null, error: null,
  commandBusy: false, start: vi.fn(), startAuthenticated: vi.fn(), cancel: vi.fn(), observeAgain: vi.fn() }));
vi.mock('../hooks/useS3ModelImport', () => ({ useS3ModelImport: () => hook }));
describe('S3 import dialog content and commands (DOM fixture)', () => {
  beforeEach(() => { vi.clearAllMocks(); hook.snapshot = { status: 'idle' }; });
  it('uses one-use password inputs and clears them before awaiting admission (DOM fixture)', () => {
    hook.startAuthenticated.mockReturnValueOnce(new Promise(() => {}));
    render(<S3ModelImportDialog onClose={vi.fn()} />);
    fireEvent.click(screen.getByLabelText('Use one-use credentials'));
    const access = screen.getByLabelText('Access key ID');
    const secret = screen.getByLabelText('Secret access key');
    const token = screen.getByLabelText('Session token (optional)');
    for (const [node, value] of [[access, 'synthetic-ui-key'], [secret, 'synthetic-ui-secret'], [token, 'synthetic-ui-token']] as const) {
      expect(node).toHaveAttribute('type', 'password');
      expect(node).toHaveAttribute('autocomplete', 'off');
      fireEvent.change(node, { target: { value } });
    }
    const form = screen.getByRole('button', { name: 'Import pinned object' }).closest('form');
    expect(form).not.toBeNull();
    if (form) fireEvent.submit(form);
    expect(hook.startAuthenticated).toHaveBeenCalledWith(expect.anything(), {
      access_key_id: 'synthetic-ui-key', secret_access_key: 'synthetic-ui-secret', session_token: 'synthetic-ui-token',
    });
    for (const node of [access, secret, token]) expect(node).toHaveValue('');
    expect(hook.start).not.toHaveBeenCalled();
  });
  it('clears owned inputs on mode replacement, close and unmount without storing a credential draft (DOM fixture)', () => {
    const close = vi.fn(); const view = render(<S3ModelImportDialog onClose={close} />);
    const mode = screen.getByLabelText('Use one-use credentials'); fireEvent.click(mode);
    const first = screen.getByLabelText('Secret access key'); fireEvent.change(first, { target: { value: 'synthetic-first-secret' } });
    fireEvent.click(mode); expect(first).toHaveValue('');
    fireEvent.click(mode);
    const next = screen.getByLabelText('Secret access key'); expect(next).toHaveValue('');
    fireEvent.change(next, { target: { value: 'synthetic-next-secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Close' })); expect(close).toHaveBeenCalledTimes(1); expect(next).toHaveValue('');
    fireEvent.change(next, { target: { value: 'synthetic-unmount-secret' } }); view.unmount(); expect(next).toHaveValue('');
  });
  it('offers only explicit anonymous pins and labelled source fields', () => {
    render(<S3ModelImportDialog onClose={vi.fn()} />);
    expect(screen.getByRole('dialog', { name: 'Import from S3' })).toBeInTheDocument();
    for (const label of ['HTTPS endpoint origin', 'Immutable VersionId', 'Expected SHA-256', 'Exact object key']) {
      expect(screen.getByLabelText(label)).toBeRequired();
    }
    expect(screen.queryByLabelText(/secret access key|access key ID|session token/i)).not.toBeInTheDocument();
    expect(screen.getByLabelText('Use one-use credentials')).not.toBeChecked();
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
