import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useS3PrefixDiscovery } from './useS3PrefixDiscovery';
import type { S3ImportDraft } from './useS3ModelImport';
import type { S3DiscoveryOutcome } from '../generated/desktop-contract';
const calls = vi.hoisted(() => ({ start: vi.fn(), auth: vi.fn(), get: vi.fn(), cancel: vi.fn() }));
vi.mock('../api/import', () => ({importAPI: {startS3PrefixDiscovery: calls.start, startAuthenticatedS3PrefixDiscovery: calls.auth,
  getS3PrefixDiscovery: calls.get, cancelS3PrefixDiscovery: calls.cancel}}));
const id = 'c3f7d104-1234-4321-abcd-aaaaaaaaaaaa';
const draft: S3ImportDraft = {endpoint:'https://source.invalid',bucket:'fixture-bucket',region:'fixture-region',addressing:'path',
  key:'',version_id:'',sha256:'',filename:'',family:'',official_name:''};
const complete: S3DiscoveryOutcome = {status:'complete',operation_id:id,objects:[],pages:1};
const running: S3DiscoveryOutcome = {status:'running',operation_id:id};
function deferred<T>() {
  let resolve!: (value:T)=>void;
  const promise=new Promise<T>(done=>{resolve=done;});
  return {promise,resolve};
}
describe('source-scoped prefix discovery observation', () => {
  beforeEach(() => {vi.useFakeTimers();vi.resetAllMocks();vi.spyOn(crypto,'randomUUID').mockReturnValue(id);
    calls.cancel.mockResolvedValue({status:'running',operation_id:id});calls.get.mockResolvedValue(complete);});
  afterEach(() => {vi.useRealTimers();vi.restoreAllMocks();});
  it('admits discovery without import fields and observes complete pins without any import', async () => {
    calls.start.mockResolvedValue({status:'running',operation_id:id});const {result}=renderHook(useS3PrefixDiscovery);
    await act(async () => {await result.current.start(draft,'models/');});
    expect(calls.start).toHaveBeenCalledWith({operation_id:id,endpoint:draft.endpoint,bucket:draft.bucket,region:draft.region,addressing:'path',prefix:'models/',timeout_ms:15000});
    await act(async () => {await vi.advanceTimersByTimeAsync(250);});
    expect(result.current.snapshot).toEqual(complete);expect(result.current.busy).toBe(false);
  });
  it('keeps credentials out of state and never resubmits after lost acknowledgement', async () => {
    calls.auth.mockRejectedValue(new Error('synthetic-private-provider-error'));const {result}=renderHook(useS3PrefixDiscovery);
    await act(async () => {await result.current.start(draft,'models/',{access_key_id:'synthetic-key',secret_access_key:'synthetic-secret',session_token:'synthetic-token'});});
    expect(result.current.busy).toBe(true);expect(JSON.stringify(result.current)).not.toMatch(/synthetic-(secret|token|private)/);
    await act(async () => {result.current.observeAgain();});
    expect(calls.auth).toHaveBeenCalledTimes(1);expect(calls.get).toHaveBeenCalledWith(id);expect(result.current.snapshot).toEqual(complete);
  });
  it('rejects malformed requests before transport', async () => {
    const {result}=renderHook(useS3PrefixDiscovery);
    await act(async () => {await result.current.start({...draft,endpoint:'http://source.invalid'},'models/');});
    expect(calls.start).not.toHaveBeenCalled();expect(result.current.busy).toBe(false);
  });
  it('ignores mismatched results and recovers by observing the same query', async () => {
    calls.start.mockResolvedValue({...complete,operation_id:'different'});const {result}=renderHook(useS3PrefixDiscovery);
    await act(async () => {await result.current.start(draft,'models/');});
    expect(result.current.snapshot).toBeNull();expect(result.current.error).toMatch(/unavailable/);
    await act(async () => {result.current.observeAgain();});expect(result.current.snapshot).toEqual(complete);
  });
  it('cancels on source reset and discards a late admission response', async () => {
    let resolve!: (v:S3DiscoveryOutcome)=>void;calls.start.mockReturnValue(new Promise<S3DiscoveryOutcome>(r=>{resolve=r;}));
    const {result}=renderHook(useS3PrefixDiscovery);let pending!:Promise<void>;
    act(() => {pending=result.current.start(draft,'models/');});act(() => {result.current.reset();});
    await act(async () => {resolve({status:'running',operation_id:id});await pending;});
    expect(calls.cancel).toHaveBeenCalledWith(id);expect(result.current.snapshot).toBeNull();expect(result.current.busy).toBe(false);
  });
  it('does not treat cancellation request as terminal and cancels on unmount', async () => {
    calls.start.mockResolvedValue({status:'running',operation_id:id});calls.get.mockResolvedValue({status:'running',operation_id:id});
    const {result,unmount}=renderHook(useS3PrefixDiscovery);await act(async () => {await result.current.start(draft,'models/');});
    await act(async () => {await result.current.cancel();});expect(result.current.busy).toBe(true);unmount();expect(calls.cancel).toHaveBeenCalledWith(id);
  });
  it('keeps one poll timer after cancellation replaces a pending observation', async () => {
    calls.start.mockResolvedValue(running);calls.get.mockResolvedValue(running);
    const {result}=renderHook(useS3PrefixDiscovery);
    await act(async()=>{await result.current.start(draft,'models/');});
    await act(async()=>{await result.current.cancel();});
    expect(calls.get).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(1);
    await act(async()=>{await vi.advanceTimersByTimeAsync(1000);});
    expect(calls.get).toHaveBeenCalledTimes(5);
    expect(vi.getTimerCount()).toBe(1);
  });
  it('coalesces rapid observe and cancel actions while an observation is in flight', async () => {
    const read=deferred<S3DiscoveryOutcome>(),cancel=deferred<S3DiscoveryOutcome>();
    calls.start.mockResolvedValue(running);calls.get.mockReturnValueOnce(read.promise).mockResolvedValue(running);
    calls.cancel.mockReturnValueOnce(cancel.promise);
    const {result}=renderHook(useS3PrefixDiscovery);
    await act(async()=>{await result.current.start(draft,'models/');await vi.advanceTimersByTimeAsync(250);});
    let cancellation!:Promise<void>;
    await act(async()=>{
      cancellation=result.current.cancel();
      for(let index=0;index<20;index++){result.current.observeAgain();void result.current.cancel();}
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(calls.get).toHaveBeenCalledTimes(1);expect(calls.cancel).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
    await act(async()=>{cancel.resolve(running);await cancellation;});
    expect(calls.get).toHaveBeenCalledTimes(1);
    await act(async()=>{read.resolve(running);});
    expect(vi.getTimerCount()).toBe(1);
    await act(async()=>{await vi.advanceTimersByTimeAsync(250);});
    expect(calls.get).toHaveBeenCalledTimes(2);expect(vi.getTimerCount()).toBe(1);
  });
  it('coalesces synchronous manual observations and resumes after a rejected read', async () => {
    calls.start.mockResolvedValue(running);calls.get.mockRejectedValueOnce(new Error('fixture transport failure')).mockResolvedValue(complete);
    const {result}=renderHook(useS3PrefixDiscovery);await act(async()=>{await result.current.start(draft,'models/');});
    await act(async()=>{for(let index=0;index<20;index++)result.current.observeAgain();});
    expect(calls.get).toHaveBeenCalledTimes(1);expect(vi.getTimerCount()).toBe(0);expect(result.current.error).toMatch(/unavailable/);
    await act(async()=>{result.current.observeAgain();});
    expect(calls.get).toHaveBeenCalledTimes(2);expect(result.current.snapshot).toEqual(complete);expect(result.current.busy).toBe(false);
  });
  it('fences old observations and cancellation acknowledgements after reset and same-UUID restart', async () => {
    const oldRead=deferred<S3DiscoveryOutcome>(),oldCancel=deferred<S3DiscoveryOutcome>();
    calls.start.mockResolvedValue(running);calls.get.mockReturnValueOnce(oldRead.promise).mockResolvedValue(complete);calls.cancel.mockReturnValueOnce(oldCancel.promise);
    const {result}=renderHook(useS3PrefixDiscovery);await act(async()=>{await result.current.start(draft,'old/');await vi.advanceTimersByTimeAsync(250);});
    let cancellation!:Promise<void>;act(()=>{cancellation=result.current.cancel();result.current.reset();});
    await act(async()=>{await result.current.start(draft,'new/');oldRead.resolve(running);oldCancel.resolve(running);await cancellation;});
    expect(vi.getTimerCount()).toBe(1);expect(calls.get).toHaveBeenCalledTimes(1);
    await act(async()=>{await vi.advanceTimersByTimeAsync(250);});
    expect(result.current.snapshot).toEqual(complete);expect(calls.get).toHaveBeenCalledTimes(2);expect(vi.getTimerCount()).toBe(0);
    await act(async()=>{await vi.advanceTimersByTimeAsync(1000);});expect(calls.get).toHaveBeenCalledTimes(2);
  });
});
