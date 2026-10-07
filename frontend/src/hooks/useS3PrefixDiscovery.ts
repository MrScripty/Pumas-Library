import { useCallback, useEffect, useRef, useState } from 'react';
import { APIError } from '../errors';
import { importAPI } from '../api/import';
import { decodeS3DiscoveryParams, decodeS3AuthenticatedDiscoveryParams, type S3CredentialParams, type S3DiscoveryOutcome } from '../generated/desktop-contract';
import type { S3ImportDraft } from './useS3ModelImport';

/** Credentials exist only in the start call. Each result belongs to an exact
 * source query; cancellation/closing revokes observation without importing. */
export function useS3PrefixDiscovery() {
  const [snapshot, setSnapshot] = useState<S3DiscoveryOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const current = useRef<{id: string; token: object} | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mounted = useRef(true);
  const reset = useCallback(() => {
    const old = current.current;
    current.current = null;
    if (timer.current !== null) clearTimeout(timer.current);
    if (old) void importAPI.cancelS3PrefixDiscovery(old.id).catch(() => {});
    if (mounted.current) { setSnapshot(null); setError(null); setBusy(false); }
  }, []);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; reset(); }; }, [reset]);

  const observe = async (owner: {id: string; token: object}, first?: S3DiscoveryOutcome) => {
    const live = () => mounted.current && current.current === owner;
    if (!live()) return;
    try {
      const value = first ?? await importAPI.getS3PrefixDiscovery(owner.id);
      if (!live()) return;
      if ('operation_id' in value && value.operation_id !== owner.id) throw new APIError('Mismatched discovery');
      setSnapshot(value); setError(null);
      if (value.status === 'running') timer.current = setTimeout(() => { void observe(owner); }, 250);
      else { current.current = null; setBusy(false); }
    } catch {
      if (live()) { setSnapshot(null); setError('Discovery observation is unavailable. Observe again or cancel the same query.'); }
    }
  };
  const start = async (draft: S3ImportDraft, prefix: string, credentials?: S3CredentialParams) => {
    if (current.current) return;
    if (typeof crypto.randomUUID !== 'function') { setError('Discovery is unavailable in this renderer.'); return; }
    const decoded = decodeS3DiscoveryParams({operation_id: crypto.randomUUID(), endpoint: draft.endpoint,
      bucket: draft.bucket, region: draft.region, addressing: draft.addressing, prefix, timeout_ms: 15000});
    const auth = credentials === undefined || decoded.status !== 'valid' ? null
      : decodeS3AuthenticatedDiscoveryParams({source: decoded.value, credentials});
    if (decoded.status !== 'valid' || auth?.status === 'invalid' || (auth && auth.status !== 'valid')) {
      setError('Check the HTTPS source, prefix and one-use credentials.'); return;
    }
    const owner = {id: decoded.value.operation_id, token: {}};
    current.current = owner; setBusy(true); setSnapshot(null); setError(null);
    try {
      const value = auth?.status === 'valid'
        ? await importAPI.startAuthenticatedS3PrefixDiscovery(auth.value)
        : await importAPI.startS3PrefixDiscovery(decoded.value);
      if (current.current !== owner) { void importAPI.cancelS3PrefixDiscovery(owner.id).catch(() => {}); return; }
      void observe(owner, value);
    } catch {
      if (mounted.current && current.current === owner) {
        setError('Discovery acknowledgement is unavailable. Observe again or cancel the same query.');
      }
    }
  };
  const cancel = async () => {
    const owner = current.current;
    if (!owner) return;
    try { await importAPI.cancelS3PrefixDiscovery(owner.id); void observe(owner); }
    catch { if (mounted.current && current.current === owner) setError('Cancellation is unconfirmed. Observe the same query again.'); }
  };
  return {snapshot, error, busy, start, cancel, reset,
    observeAgain: () => { if (current.current) void observe(current.current); }};
}
