import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { S3PersistedImportsPanel } from './S3PersistedImportsPanel';
const inspect = vi.hoisted(() => vi.fn());
vi.mock('../api/import', () => ({importAPI:{inspectPersistedS3Imports:inspect}}));
const id='c3f7d104-1234-4321-abcd-aaaaaaaaaaaa';
const record={operation_id:id, acquisition_id:id, phase:'using',receipt_present:true,
  model_binding:{model_id:'fixture/gguf/model',publication_id:id,publication_state:'pending'}};
describe('explicit persisted S3 inspection', () => {
  beforeEach(() => { vi.clearAllMocks(); });
  it('does not inspect on mount and exposes uncertain recorded bindings without recovery controls', async () => {
    inspect.mockResolvedValue({status:'complete',imports:[record]});
    render(<S3PersistedImportsPanel/>); expect(inspect).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button',{name:'Inspect persisted imports'}));
    await screen.findByText('Persisted phase: using');
    expect(screen.getByText('Recorded model: fixture/gguf/model')).toBeInTheDocument();
    expect(screen.getByText(`Publication receipt: ${id} (pending)`)).toBeInTheDocument();
    expect(screen.getByText(/do not prove model availability/)).toBeInTheDocument();
    expect(screen.queryByRole('button',{name:/resume|reconcile|retry import/i})).not.toBeInTheDocument();
    expect(inspect).toHaveBeenCalledExactlyOnceWith();
  });
  it('clears old rows on refresh and refuses malformed or over-capacity responses', async () => {
    inspect.mockResolvedValueOnce({status:'complete',imports:[record]})
      .mockResolvedValueOnce({status:'incomplete'})
      .mockResolvedValueOnce({status:'complete',imports:[{...record,credentials:{secret:'synthetic-secret'}}]});
    render(<S3PersistedImportsPanel/>);
    fireEvent.click(screen.getByRole('button',{name:'Inspect persisted imports'}));
    await screen.findByText('Persisted phase: using');
    fireEvent.click(screen.getByRole('button',{name:'Inspect persisted imports'}));
    await screen.findByText(/Inspection exceeded its capacity/);
    expect(screen.queryByText('Persisted phase: using')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button',{name:'Inspect persisted imports'}));
    await screen.findByText(/evidence is uncertain/); expect(screen.queryByText(/synthetic-secret/)).not.toBeInTheDocument();
  });
  it('coalesces pending inspections and preserves an empty unavailable observation', async () => {
    let settle: (value:unknown)=>void = () => {};
    inspect.mockReturnValue(new Promise(resolve => {settle=resolve;}));
    render(<S3PersistedImportsPanel/>);
    const button=screen.getByRole('button',{name:'Inspect persisted imports'});
    fireEvent.click(button); fireEvent.click(button); expect(inspect).toHaveBeenCalledOnce();
    expect(button).toBeDisabled(); settle({status:'complete',imports:[]});
    await screen.findByText('No owned persisted acquisition records were observed.');
    await waitFor(()=>expect(button).toBeEnabled());
  });
});
