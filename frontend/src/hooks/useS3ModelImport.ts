import { useCallback, useEffect, useRef, useState } from 'react';
import { importAPI } from '../api/import';
import { decodeS3BundleImportParams, decodeS3AuthenticatedBundleImportParams, type S3PinnedFileParams, type S3BundleProgressWire, decodeS3ImportParams, decodeS3AuthenticatedImportParams, type S3CredentialParams, type S3ImportOutcome, type S3ImportParams } from '../generated/desktop-contract';

type Observation = { id?: string; token: object };
export type S3ImportDraft = Record<keyof Omit<S3ImportParams, 'operation_id'>, string>;

/** One dialog observes a pull-style, process-owned RPC job. Closing only ends
 * observation; a new dialog queries the existing job, never resubmits its facts. */
export function useS3ModelImport(onImported?: () => void, bundleObservation = false) {
  const [bundleProgress, setBundleProgress] = useState<S3BundleProgressWire | null>(null);
  const [snapshot, setSnapshot] = useState<S3ImportOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [commandBusy, setCommandBusy] = useState(false);
  const [observation, setObservation] = useState<Observation | null>(() => ({ token: {} }));
  const owner = useRef<object | null>(null);
  const mounted = useRef(true);
  const busy = useRef(false);
  const query = useRef<string | undefined>(undefined);
  const onImportedRef = useRef(onImported);
  const reported = useRef<string | null>(null);
  onImportedRef.current = onImported;

  const apply = useCallback((token: object, value: S3ImportOutcome): 'applied' | 'superseded' => {
    if (!mounted.current || owner.current !== token) return 'superseded';
    setSnapshot(value);
    if (value.status === 'finished' && (value.result.status === 'completed'
      || (value.result.status === 'failed' && value.result.published_model_id !== null))) {
      if (reported.current !== value.operation_id) {
        reported.current = value.operation_id;
        onImportedRef.current?.();
      }
    }
    return 'applied';
  }, []);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; owner.current = null; };
  }, []);

  useEffect(() => {
    if (observation === null) return;
    const token = observation.token;
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    owner.current = observation.token;
    query.current = observation.id;
    const current = () => active && mounted.current && owner.current === observation.token;
    async function poll(): Promise<'observed' | 'superseded' | 'unavailable'> {
      if (!current()) return 'superseded';
      try {
        const bundle = bundleObservation ? await importAPI.getS3ModelBundleImport(query.current) : null;
        const value = bundle === null ? await importAPI.getS3ModelImport(query.current) : bundle.outcome;
        if (!current()) return 'superseded';
        if ((value.status === 'running' || value.status === 'finished' || value.status === 'not_found')
          && query.current !== undefined && value.operation_id !== query.current) {
          setSnapshot(null);
          setBundleProgress(null);
          setError('The source observation did not match this operation. Do not resubmit it.');
          return 'unavailable';
        }
        if (value.status === 'running' || value.status === 'finished') query.current = value.operation_id;
        apply(token, value);
        setBundleProgress(bundle?.bundle_progress ?? null);
        if (value.status === 'finished') setError(null);
        if (value.status === 'running') timer = setTimeout(() => { void poll(); }, 500);
        return 'observed';
      } catch {
        if (!current()) return 'superseded';
        setSnapshot(null);
        setBundleProgress(null);
        setError('Source observation is unavailable. The operation may still be running; observe again without resubmitting.');
        return 'unavailable';
      }
    }
    void poll(); // The continuation observes failure and classifies superseded results.
    return () => { active = false; if (timer !== undefined) clearTimeout(timer); };
  }, [observation, apply, bundleObservation]);

  const startRequest = async (draft: S3ImportDraft, credentials?: S3CredentialParams, auxiliaries?: readonly S3PinnedFileParams[]) => {
    if (busy.current) return;
    if (typeof crypto.randomUUID !== 'function') {
      setError('This renderer cannot create an operation identity. Import is unavailable.');
      return;
    }
    const id = crypto.randomUUID();
    const decoded = decodeS3ImportParams({ ...draft, operation_id: id });
    if (decoded.status !== 'valid') {
      setError('Check all required source fields, the HTTPS origin, GGUF filename and 64-digit SHA-256.');
      return;
    }
    const authenticated = credentials === undefined ? null : decodeS3AuthenticatedImportParams({ source: decoded.value, credentials });
    if (authenticated !== null && authenticated.status !== 'valid') {
      setError('Check the explicitly supplied credential fields. They were cleared; enter them again for a new request.');
      return;
    }
    const bundle = auxiliaries === undefined ? null : decodeS3BundleImportParams({
      operation_id: id, endpoint: decoded.value.endpoint, region: decoded.value.region,
      bucket: decoded.value.bucket, addressing: decoded.value.addressing,
      family: decoded.value.family, official_name: decoded.value.official_name,
      primary_logical_path: decoded.value.filename,
      files: [{key:decoded.value.key,version_id:decoded.value.version_id,logical_path:decoded.value.filename,sha256:decoded.value.sha256},...auxiliaries],
    });
    const authenticatedBundle = bundle?.status === 'valid' && credentials !== undefined
      ? decodeS3AuthenticatedBundleImportParams({source:bundle.value,credentials}) : null;
    if ((bundle !== null && bundle.status !== 'valid') || (authenticatedBundle !== null && authenticatedBundle.status !== 'valid')) {
      setError('Check every selected file pin and portable logical path. The complete set must be valid before admission.');
      return;
    }
    const token = {};
    busy.current = true;
    owner.current = token;
    query.current = id;
    setCommandBusy(true);
    setObservation(null);
    setError(null);
    setBundleProgress(null);
    try {
      const value = authenticatedBundle?.status === 'valid'
        ? await importAPI.startAuthenticatedS3ModelBundleImport(authenticatedBundle.value)
        : bundle?.status === 'valid' ? await importAPI.startS3ModelBundleImport(bundle.value)
          : authenticated?.status === 'valid' ? await importAPI.startAuthenticatedS3ModelImport(authenticated.value)
            : await importAPI.startS3ModelImport(decoded.value);
      if (!mounted.current || owner.current !== token) return;
      if ((value.status === 'running' || value.status === 'finished') && value.operation_id !== id) {
        setSnapshot(null);
        setBundleProgress(null);
        setError('The admission result did not match this operation. Do not resubmit it.');
        setObservation({ id, token });
      } else {
        apply(token, value);
        if (value.status === 'running') setObservation({ id, token });
        if (value.status === 'rejected') { query.current = undefined; setError(value.error.message); }
      }
    } catch {
      if (!mounted.current || owner.current !== token) return;
      setSnapshot(null);
      setBundleProgress(null);
      setError('Admission result is unavailable. Observing the same operation identity; do not resubmit.');
      setObservation({ id, token });
    } finally {
      busy.current = false;
      if (mounted.current && owner.current === token) setCommandBusy(false);
    }
  };

  const cancel = async () => {
    if (busy.current || snapshot?.status !== 'running') return;
    const token = {};
    const id = snapshot.operation_id;
    owner.current = token;
    setObservation(null);
    setBundleProgress(null);
    busy.current = true;
    setCommandBusy(true);
    try {
      const value = await importAPI.cancelS3ModelImport(id);
      if (!mounted.current || owner.current !== token) return;
      const outcome = value.outcome;
      if ((outcome.status === 'running' || outcome.status === 'finished' || outcome.status === 'not_found')
        && outcome.operation_id !== id) {
        setSnapshot(null);
        setBundleProgress(null);
        setError('Cancellation observation did not match this operation. Observe again.');
        return;
      }
      apply(token, outcome);
      if (outcome.status === 'running') setObservation({ id, token });
      setError(value.accepted ? 'Cancellation requested. Waiting for the owned result.' : 'Cancellation was not accepted; finalization may already own the result.');
    } catch {
      if (mounted.current && owner.current === token) {
        setSnapshot(null);
        setBundleProgress(null);
        setError('Cancellation acknowledgement is unavailable. Observing the same operation without resubmitting.');
        setObservation({ id, token });
      }
    } finally {
      busy.current = false;
      if (mounted.current && owner.current === token) setCommandBusy(false);
    }
  };
  const observeAgain = () => {
    if (busy.current) return;
    const token = {};
    owner.current = token;
    setError(null);
    setSnapshot(null);
    setBundleProgress(null);
    setObservation({ id: query.current, token });
  };
  return { snapshot, bundleProgress,
    startBundle: (draft: S3ImportDraft, auxiliaries: readonly S3PinnedFileParams[], credentials?: S3CredentialParams) => startRequest(draft, credentials, auxiliaries), error, commandBusy, start: (draft: S3ImportDraft) => startRequest(draft),
    startAuthenticated: (draft: S3ImportDraft, credentials: S3CredentialParams) => startRequest(draft, credentials),
    cancel, observeAgain };
}
