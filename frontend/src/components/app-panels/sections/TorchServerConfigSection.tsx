/**
 * Torch Server Configuration Section
 *
 * Allows configuring the Torch inference server: host, port,
 * LAN access toggle, and max loaded models.
 */

import { useState, useEffect, useCallback } from 'react';
import { Settings, AlertTriangle, Loader2 } from 'lucide-react';
import { api, isAPIAvailable } from '../../../api/adapter';
import { getLogger } from '../../../utils/logger';

const logger = getLogger('TorchServerConfigSection');

export interface TorchServerConfigSectionProps {
  connectionUrl?: string;
}

export function TorchServerConfigSection({
  connectionUrl,
}: TorchServerConfigSectionProps) {
  const [host, setHost] = useState('127.0.0.1');
  const [port, setPort] = useState(8400);
  const [lanAccess, setLanAccess] = useState(false);
  const [maxLoadedModels, setMaxLoadedModels] = useState(4);
  const [isSaving, setIsSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Fetch current config from server
  const fetchConfig = useCallback(async () => {
    if (!isAPIAvailable()) return;

    try {
      const result = await api.torch_get_status(connectionUrl);
      if (result.success && result.config) {
        setHost(result.config.host || '127.0.0.1');
        setPort(result.config.api_port || 8400);
        setLanAccess(result.config.lan_access || false);
        setMaxLoadedModels(result.config.max_loaded_models || 4);
      }
    } catch (err) {
      logger.debug('Failed to fetch Torch config', { error: err });
    }
  }, [connectionUrl]);

  useEffect(() => {
    void fetchConfig();
  }, [fetchConfig]);

  const handleApply = async () => {
    if (!isAPIAvailable()) return;

    setIsSaving(true);
    setError(null);
    setSaved(false);

    try {
      const result = await api.torch_configure({
        host,
        api_port: port,
        lan_access: lanAccess,
        max_loaded_models: maxLoadedModels,
      });
      if (result.success) {
        setSaved(true);
        setTimeout(() => setSaved(false), 2000);
      } else {
        setError(result.error || 'Failed to apply configuration');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      setError(message);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="w-full space-y-3">
      <div className="text-xs uppercase tracking-wider text-[hsl(var(--launcher-text-muted))] flex items-center gap-2">
        <Settings className="w-3.5 h-3.5" />
        <span>Server Configuration</span>
      </div>

      <div className="space-y-3 px-3 py-3 rounded-lg bg-[hsl(var(--launcher-bg-secondary)/0.3)] border border-[hsl(var(--launcher-border)/0.3)]">
        {/* Host */}
        <div className="flex items-center gap-3">
          <label htmlFor="torch-host" className="text-xs text-[hsl(var(--launcher-text-muted))] w-28 shrink-0">Host</label>
          <input
            id="torch-host"
            type="text"
            value={host}
            onChange={(e) => setHost(e.target.value)}
            className="flex-1 text-xs px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))] focus:outline-none focus:border-[hsl(var(--accent-primary)/0.5)]"
          />
        </div>

        {/* Port */}
        <div className="flex items-center gap-3">
          <label htmlFor="torch-port" className="text-xs text-[hsl(var(--launcher-text-muted))] w-28 shrink-0">Port</label>
          <input
            id="torch-port"
            type="number"
            value={port}
            onChange={(e) => setPort(parseInt(e.target.value, 10) || 8400)}
            min={1024}
            max={65535}
            className="flex-1 text-xs px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))] focus:outline-none focus:border-[hsl(var(--accent-primary)/0.5)]"
          />
        </div>

        {/* Max Loaded Models */}
        <div className="flex items-center gap-3">
          <label htmlFor="torch-max-models" className="text-xs text-[hsl(var(--launcher-text-muted))] w-28 shrink-0">Max Models</label>
          <input
            id="torch-max-models"
            type="number"
            value={maxLoadedModels}
            onChange={(e) => setMaxLoadedModels(parseInt(e.target.value, 10) || 1)}
            min={1}
            max={16}
            className="flex-1 text-xs px-2 py-1.5 rounded bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))] focus:outline-none focus:border-[hsl(var(--accent-primary)/0.5)]"
          />
        </div>

        {/* LAN Access Toggle */}
        <div className="flex items-center gap-3">
          <label htmlFor="torch-lan-access" className="text-xs text-[hsl(var(--launcher-text-muted))] w-28 shrink-0">LAN Access</label>
          <div className="flex items-center gap-2">
            <button
              id="torch-lan-access"
              onClick={() => setLanAccess(!lanAccess)}
              className={`relative w-9 h-5 rounded-full transition-colors ${
                lanAccess
                  ? 'bg-[hsl(var(--accent-primary))]'
                  : 'bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.5)]'
              }`}
            >
              <span
                className={`absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
                  lanAccess ? 'translate-x-4' : 'translate-x-0.5'
                }`}
              />
            </button>
          </div>
        </div>

        {lanAccess && (
          <div className="flex items-start gap-2 px-2 py-1.5 rounded bg-[hsl(var(--accent-warning)/0.1)] text-[hsl(var(--accent-warning))]">
            <AlertTriangle className="w-3.5 h-3.5 shrink-0 mt-0.5" />
            <span className="text-xs">
              Enabling LAN access exposes the API to all devices on your network.
              Only enable this if you trust your local network.
            </span>
          </div>
        )}

        {/* Error */}
        {error && (
          <p className="text-xs text-[hsl(var(--accent-error))]">{error}</p>
        )}

        {/* Apply button */}
        <div className="flex justify-end">
          <button
            onClick={handleApply}
            disabled={isSaving}
            className="px-3 py-1.5 text-xs font-medium rounded transition-colors bg-[hsl(var(--accent-primary))] text-white hover:bg-[hsl(var(--accent-primary)/0.8)] disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isSaving ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : saved ? (
              'Saved'
            ) : (
              'Apply'
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
