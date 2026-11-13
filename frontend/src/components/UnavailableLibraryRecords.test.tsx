import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { UnavailableLibraryRecords } from './UnavailableLibraryRecords';

const retained = {
  model_id: 'audio/acme/retained',
  reason: 'Ready on disk, final index commit never acknowledged',
  code: 'import_publication_finalization_pending',
  recovery: 'finish_retained_import' as const,
};
const malformed = { model_id: 'llm/acme/broken', reason: 'dependency_bindings is missing' };

describe('UnavailableLibraryRecords', () => {
  it('offers the backend repair only for records that report one', () => {
    render(<UnavailableLibraryRecords records={[retained, malformed]} onFinishRetainedImport={vi.fn()} />);
    expect(screen.getByRole('alert')).toHaveTextContent('2 library records could not be shown:');
    expect(screen.getAllByRole('button', { name: 'Verify and finish import' })).toHaveLength(1);
  });

  it('runs the repair and shows its reason when the backend refuses', async () => {
    const finish = vi.fn()
      .mockRejectedValueOnce(new Error('its confirmed receipt or payload no longer matches'))
      .mockResolvedValueOnce(undefined);
    render(<UnavailableLibraryRecords records={[retained]} onFinishRetainedImport={finish} />);
    await userEvent.click(screen.getByRole('button', { name: 'Verify and finish import' }));
    expect(finish).toHaveBeenCalledWith('audio/acme/retained');
    expect(await screen.findByText('Could not finish: its confirmed receipt or payload no longer matches')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Verify and finish import' }));
    await waitFor(() => expect(screen.queryByText(/Could not finish/)).not.toBeInTheDocument());
  });

  it('renders nothing when every record loaded', () => {
    const { container } = render(<UnavailableLibraryRecords records={[]} />);
    expect(container).toBeEmptyDOMElement();
  });
});
