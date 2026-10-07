import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { S3TransferRetryPanel } from './S3TransferRetryPanel';
import type { S3TransferRetryState, S3ImportOutcome } from '../generated/desktop-contract';
const {read} = vi.hoisted(() => ({read:vi.fn<() => Promise<S3TransferRetryState>>()}));
vi.mock('../api/import', () => ({importAPI:{getS3TransferRetry:read}}));
const id='c3f7d104-1234-4321-abcd-aaaaaaaaaaaa';
const finished:S3ImportOutcome={status:'finished',operation_id:id,result:{status:'cancelled',retained_work:true}};
describe('explicit same-process S3 retry',()=>{
  beforeEach(()=>{vi.resetAllMocks();});
  it('does not retry automatically and keeps cold custody unsupported',async()=>{
    const retry=vi.fn();read.mockResolvedValue({status:'unavailable',reason:'no_live_custody'});
    render(<S3TransferRetryPanel outcome={null} busy={false} onRetry={retry}/>);
    await waitFor(()=>expect(read).toHaveBeenCalled());
    expect(screen.getByText(/cold retry is unsupported/)).toBeInTheDocument();
    expect(screen.queryByRole('button',{name:'Retry transfer'})).not.toBeInTheDocument();expect(retry).not.toHaveBeenCalled();
  });
  it('uses only the retained identity and clears fresh one-use credentials',async()=>{
    const retry=vi.fn().mockResolvedValue(undefined);read.mockResolvedValue({status:'ready',operation_id:id,authentication_required:true});
    const view=render(<S3TransferRetryPanel outcome={finished} busy={false} onRetry={retry}/>);
    const button=await screen.findByRole('button',{name:'Retry transfer'});
    const access=screen.getByLabelText('Retry access key ID'),secret=screen.getByLabelText('Retry secret access key');
    fireEvent.change(access,{target:{value:'synthetic-fresh-key'}});fireEvent.change(secret,{target:{value:'synthetic-fresh-secret'}});fireEvent.click(button);
    expect(retry).toHaveBeenCalledWith({access_key_id:'synthetic-fresh-key',secret_access_key:'synthetic-fresh-secret',session_token:null});
    expect(access).toHaveValue('');expect(secret).toHaveValue('');
    fireEvent.change(secret,{target:{value:'synthetic-close-secret'}});view.rerender(<S3TransferRetryPanel outcome={finished} busy={false} onRetry={retry} clearEpoch={1}/>);expect(secret).toHaveValue('');
    fireEvent.change(secret,{target:{value:'synthetic-unmount-secret'}});view.unmount();expect(secret).toHaveValue('');
  });
  it('refuses mismatched availability and removes controls during a running attempt',async()=>{
    read.mockResolvedValue({status:'ready',operation_id:'aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa',authentication_required:false});
    const retry=vi.fn();const view=render(<S3TransferRetryPanel outcome={finished} busy={false} onRetry={retry}/>);
    await waitFor(()=>expect(read).toHaveBeenCalled());expect(screen.queryByRole('button',{name:'Retry transfer'})).not.toBeInTheDocument();
    view.rerender(<S3TransferRetryPanel outcome={{status:'running',operation_id:id,progress:{phase:'acquiring',downloaded_for_current_file:'0'}}} busy={false} onRetry={retry}/>);
    expect(screen.queryByRole('button',{name:'Retry transfer'})).not.toBeInTheDocument();expect(retry).not.toHaveBeenCalled();
  });
  it('does not restore a stale availability response after the observation changes',async()=>{
    let resolve: ((value:S3TransferRetryState)=>void)|undefined;
    read.mockReturnValueOnce(new Promise(done=>{resolve=done;}));
    const retry=vi.fn();const view=render(<S3TransferRetryPanel outcome={finished} busy={false} onRetry={retry}/>);
    view.rerender(<S3TransferRetryPanel outcome={{status:'running',operation_id:id,progress:{phase:'acquiring',downloaded_for_current_file:'0'}}} busy={false} onRetry={retry}/>);
    await act(async()=>{resolve?.({status:'ready',operation_id:id,authentication_required:false});});
    expect(screen.queryByRole('button',{name:'Retry transfer'})).not.toBeInTheDocument();
    expect(retry).not.toHaveBeenCalled();
  });
  it('clears credential nodes when a command begins and never offers completed work for retry',async()=>{
    read.mockResolvedValue({status:'ready',operation_id:id,authentication_required:true});
    const retry=vi.fn();const view=render(<S3TransferRetryPanel outcome={finished} busy={false} onRetry={retry}/>);
    await screen.findByRole('button',{name:'Retry transfer'});
    const secret=screen.getByLabelText('Retry secret access key');
    fireEvent.change(secret,{target:{value:'fixture-discarded-secret'}});
    view.rerender(<S3TransferRetryPanel outcome={finished} busy onRetry={retry}/>);
    expect(screen.queryByRole('button',{name:'Retry transfer'})).not.toBeInTheDocument();
    expect(secret).toHaveValue('');
    view.rerender(<S3TransferRetryPanel outcome={{status:'finished',operation_id:id,result:{status:'completed',model_id:'fixture/model'}}} busy={false} onRetry={retry}/>);
    await waitFor(()=>expect(read).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole('button',{name:'Retry transfer'})).not.toBeInTheDocument();
  });
});
