export interface TorchRuntimeOptions {
  builds: string[];
  pythons: Array<{ id: string; label: string }>;
  adapters: string[];
  preset: { tag: string; build: string; python: string; adapter: string };
  installed: TorchInstalledConfig[];
}

export interface TorchInstalledConfig {
  tag: string;
  build: string | null;
  python: string | null;
  adapter: string | null;
  qualification: 'qualified' | 'unverified';
}

export interface TorchRuntimePreviewRequest {
  tag: string;
  build: string;
  python: string;
  adapter: string;
}

export interface TorchRuntimePreview {
  previewId: string;
  tag: string;
  build: string;
  python: string;
  adapter: string;
  expiresInSeconds?: number;
  qualification: 'qualified' | 'unverified';
  artifacts: Array<{ name: string; version: string; url: string; sha256: string }>;
}

export interface TorchCapabilityProbe {
  status: string;
  scope?: string;
  reason?: string;
  error?: string;
}

export interface TorchRuntimeProbeReport {
  recorded_at?: string;
  stale?: boolean;
  staleReasons?: string[];
  status: 'passed' | 'partial' | 'failed';
  core_status: 'passed' | 'failed';
  adapter_status: 'not selected' | 'unavailable' | 'inconclusive';
  capabilities: Record<string, TorchCapabilityProbe>;
}
