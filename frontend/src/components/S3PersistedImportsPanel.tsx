import { useEffect, useRef, useState } from 'react';
import { importAPI } from '../api/import';
import { decodeS3PersistedImportsWire, type S3PersistedImportsWire } from '../generated/desktop-contract';

/** Explicit inspection is independent of the process-local import controller. */
export function S3PersistedImportsPanel() {
  const alive = useRef(true);
  const pending = useRef(false);
  const [busy, setBusy] = useState(false);
  const [observation, setObservation] = useState<S3PersistedImportsWire | null>(null);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const inspect = async () => {
    if (pending.current) return;
    pending.current = true; setBusy(true); setObservation(null);
    try {
      const decoded = decodeS3PersistedImportsWire(await importAPI.inspectPersistedS3Imports());
      if (alive.current) setObservation(decoded.status === 'valid' ? decoded.value : {status:'unavailable'});
    } catch { if (alive.current) setObservation({status:'unavailable'}); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  };
  return <section aria-label="Persisted S3 import records" className="my-4 space-y-2">
    <h3 className="font-semibold">Persisted S3 import records</h3>
    <p className="text-sm">Inspect up to 32 owned acquisition records, including records retained after restart. Unmarked stages cannot establish ownership and are excluded. Inspection does not resume or reconcile work.</p>
    <button type="button" disabled={busy} onClick={() => { void inspect(); }}
      className="rounded border border-[hsl(var(--launcher-border))] px-3 py-2 disabled:opacity-50">Inspect persisted imports</button>
    {busy && <p role="status">Inspecting persisted records…</p>}
    {observation?.status === 'unavailable' && <p role="status">Persisted records are unavailable or their evidence is uncertain. Existing work is preserved.</p>}
    {observation?.status === 'incomplete' && <p role="status">Inspection exceeded its capacity. No partial record list is shown; existing work is preserved.</p>}
    {observation?.status === 'complete' && <>
      <p className="text-sm">These are persisted phases and recorded bindings. They do not prove model availability, publication acknowledgment or cleanup completion.</p>
      {observation.imports.length === 0 ? <p role="status">No owned persisted acquisition records were observed.</p> : <ul>
        {observation.imports.map(row => <li key={row.acquisition_id} className="my-2 break-all text-sm">
          <p>Operation: {row.operation_id}</p><p>Acquisition: {row.acquisition_id}</p>
          <p>Persisted phase: {row.phase}</p><p>Consumer receipt: {row.receipt_present ? 'present and bound' : 'not observed'}</p>
          {row.model_binding ? <><p>Recorded model: {row.model_binding.model_id}</p>
            <p>Publication receipt: {row.model_binding.publication_id} ({row.model_binding.publication_state})</p></>
            : <p>Exact recorded model binding unavailable.</p>}
        </li>)}
      </ul>}
    </>}
  </section>;
}
