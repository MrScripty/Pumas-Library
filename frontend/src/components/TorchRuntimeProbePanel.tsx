import { useEffect, useState } from 'react';
import { api } from '../api/adapter';
import type { TorchInstalledConfig, TorchRuntimeProbeReport } from '../types/torch-install';

function labelForCapability(key: string): string {
  return key.replaceAll('_', ' ');
}

export function TorchRuntimeProbePanel({ tag }: { tag: string }) {
  const [report, setReport] = useState<TorchRuntimeProbeReport | null>(null);
  const [installedConfig, setInstalledConfig] = useState<TorchInstalledConfig | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [refresh, setRefresh] = useState(0);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setReport(null);
    setInstalledConfig(null);
    setError(null);
    void api.get_torch_runtime_probe(tag).then((result) => {
      if (active) setReport(result);
    }).catch((cause: unknown) => {
      if (active) setError(cause instanceof Error ? cause.message : String(cause));
    }).finally(() => {
      if (active) setLoading(false);
    });
    void api.get_torch_runtime_options().then((options) => {
      if (active) setInstalledConfig(options.installed.find((entry) => entry.tag === tag) ?? null);
    }).catch(() => {
      // A saved probe can remain useful when build metadata is unavailable.
    });
    return () => { active = false; };
  }, [tag, refresh]);

  return (
    <section className="mt-3 rounded border p-3 text-xs" aria-label={`Torch probe results for ${tag}`}>
      <div className="flex items-center justify-between gap-2">
        <strong>Saved install-time checks · {tag}</strong>
        <button type="button" onClick={() => setRefresh((value) => value + 1)} disabled={loading} className="underline disabled:opacity-50">Refresh results</button>
      </div>
      {installedConfig && <p className="mt-2">
        Installed: {installedConfig.build ?? 'unknown build'} · {installedConfig.python ?? 'unknown Python'} · {installedConfig.adapter ?? 'unknown adapter'} · {installedConfig.qualification}
      </p>}
      {loading && <p role="status">Loading saved probe results…</p>}
      {error && <p role="status">Probe results unavailable: {error}</p>}
      {report && (
        <>
          {report.recorded_at && <p className="mt-2">Recorded {new Date(report.recorded_at).toLocaleString()}</p>}
          {report.stale && <p role="alert" className="mt-2">Saved checks are stale. {report.staleReasons?.join('; ')}</p>}
          <p className="mt-2">
            Core: <strong>{report.core_status}</strong> · Image adapter: <strong>{report.adapter_status}</strong>
            {report.status === 'partial' && ' · Partial support; inspect the adapter checks below.'}
          </p>
          <ul className="mt-2 space-y-1">
            {Object.entries(report.capabilities).map(([key, capability]) => (
              <li key={key} className="flex flex-wrap gap-x-2">
                <span className="capitalize">{labelForCapability(key)}:</span>
                <strong>{capability.status}</strong>
                {capability.scope && <span>({capability.scope})</span>}
                {(capability.error || capability.reason) && <span>{capability.error || capability.reason}</span>}
              </li>
            ))}
          </ul>
          {report.capabilities['sidecar_startup']?.status === 'not tested' && (
            <p className="mt-2">Socket startup was not tested during installation. It is attempted when a model is served through a Torch profile.</p>
          )}
          {report.capabilities['sidecar_startup']?.status === 'failed' && (
            <p role="alert" className="mt-2">Startup failed: {report.capabilities['sidecar_startup'].error || 'See the Torch runtime log.'}</p>
          )}
        </>
      )}
    </section>
  );
}
