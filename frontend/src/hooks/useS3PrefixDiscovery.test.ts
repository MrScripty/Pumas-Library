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
});
