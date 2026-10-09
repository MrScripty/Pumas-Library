import { useEffect, useRef, useState } from 'react';
import { importAPI } from '../api/import';
import { decodeS3TransferRetryState, type S3TransferRetryState, type S3ImportOutcome, type S3CredentialParams } from '../generated/desktop-contract';

/** Availability is advisory. The existing owner checks exact custody on retry. */
export function S3TransferRetryPanel({outcome, busy, onRetry, clearEpoch = 0}: {
  outcome: S3ImportOutcome | null; busy: boolean; clearEpoch?: number; onRetry: (credentials?: S3CredentialParams) => Promise<void>;
}) {
  const [observation, setObservation] = useState<{outcome: S3ImportOutcome | null; state: S3TransferRetryState} | null>(null);
  const access = useRef<HTMLInputElement>(null);
  const secret = useRef<HTMLInputElement>(null);
  const token = useRef<HTMLInputElement>(null);
  const clear = () => { for (const input of [access.current, secret.current, token.current]) if (input) input.value = ''; };
  useEffect(() => { for (const input of [access.current, secret.current, token.current]) if (input) input.value = ''; }, [clearEpoch]);
  const id = outcome && 'operation_id' in outcome ? outcome.operation_id : undefined;
  const terminal = outcome?.status !== 'running';
  useEffect(() => {
    const invocation = {active: true};
    setObservation(null);
    if (terminal && !busy) {
      void (async () => {
        try {
          const decoded = decodeS3TransferRetryState(await importAPI.getS3TransferRetry(id));
          if (invocation.active) setObservation({outcome, state: decoded.status === 'valid' && (decoded.value.status !== 'ready' || decoded.value.operation_id === id)
            ? decoded.value : {status:'unavailable',reason:'not_retryable'}});
        } catch { if (invocation.active) setObservation({outcome, state:{status:'unavailable',reason:'not_retryable'}}); }
      })();
    }
    return () => { invocation.active = false; };
  }, [id, terminal, busy, outcome]);
  const state = observation?.outcome === outcome ? observation.state : null;
  const authenticated = state?.status === 'ready' && state.authentication_required;
  useEffect(() => {
    const inputs = [access.current,secret.current,token.current];
    return () => { for (const input of inputs) if (input) input.value = ''; };
  }, [authenticated]);
  if (busy || outcome?.status !== 'finished' || outcome.result.status === 'completed' || state?.status !== 'ready') return <p className="my-3 text-sm">Retry requires eligible transfer inputs still held by this process. After a restart, inspection cannot reopen their physical reservation; cold retry is unsupported and retained work is preserved.</p>;
  return <section aria-label="Retry retained S3 transfer" className="my-4 space-y-2">
    <p className="text-sm">Retry the same operation with its original source, versions, paths and SHA-256 pins. Verified complete files are rechecked; partial transfers may restart from byte zero.</p>
    {authenticated && <fieldset disabled={busy} className="space-y-2">
      <legend>Fresh credentials for retry</legend>
      <p>Previous credentials were discarded. These inputs clear on retry or close.</p>
      <label>Retry access key ID<input ref={access} type="password" autoComplete="off" maxLength={4096} /></label>
      <label>Retry secret access key<input ref={secret} type="password" autoComplete="off" maxLength={4096} /></label>
      <label>Retry session token<input ref={token} type="password" autoComplete="off" maxLength={4096} /></label>
    </fieldset>}
    <button type="button" disabled={busy} className="rounded border px-3 py-2" onClick={() => {
      const credentials = authenticated ? {access_key_id:access.current?.value ?? '',secret_access_key:secret.current?.value ?? '',session_token:token.current?.value || null} : undefined;
      clear(); void onRetry(credentials);
    }}>Retry transfer</button>
  </section>;
}
