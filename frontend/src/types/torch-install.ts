import type {
  TorchRuntimePreview as DesktopTorchRuntimePreview,
  TorchRuntimePreviewOutcome as DesktopTorchRuntimePreviewOutcome,
  TorchRuntimePreviewRejectionReason as DesktopTorchRuntimePreviewRejectionReason,
} from '../generated/desktop-contract';

export type TorchRuntimePreviewOutcome = DesktopTorchRuntimePreviewOutcome;
export type TorchRuntimePreviewRejectReason = DesktopTorchRuntimePreviewRejectionReason;
export type TorchRuntimePreview = DesktopTorchRuntimePreview;

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

export interface TorchCapabilityProbe {
  status: string;
  scope?: string;
  reason?: string;
  error?: string | null;
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

export interface TorchStartupTrialOutcome {
  success: boolean;
  tag: string;
  profileId: string;
  startupStatus: 'passed' | 'failed';
  healthStatus: 'passed' | 'failed' | 'not_checked';
  protocol: number | null;
  capabilities: string[];
  generation: string | null;
  startedByTrial: boolean;
  error?: string | null;
  cleanup: 'not_needed' | 'stopped_owned_generation' | 'manual_stop_required';
}

export interface TorchAlternativeMatch {
  tag: string;
  build: string;
  python: string;
  wheelUrl?: string;
  sha256?: string;
}

export interface TorchAlternativesOutcome {
  selectedTag: string;
  selectedBuild: string;
  selectedPython: string;
  status: 'matches' | 'none' | 'inconclusive';
  incomplete: true;
  dependenciesNotChecked: true;
  checkedBuilds: string[];
  matches: TorchAlternativeMatch[];
  issues: string[];
}
