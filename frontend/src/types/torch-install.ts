import type {
  TorchAlternativeMatch as DesktopTorchAlternativeMatch,
  TorchAlternativesOutcome as DesktopTorchAlternativesOutcome,
  TorchCapabilityProbe as DesktopTorchCapabilityProbe,
  TorchInstalledRuntime as DesktopTorchInstalledRuntime,
  TorchRuntimeOptionsOutcome as DesktopTorchRuntimeOptionsOutcome,
  TorchRuntimePreview as DesktopTorchRuntimePreview,
  TorchRuntimePreviewOutcome as DesktopTorchRuntimePreviewOutcome,
  TorchRuntimePreviewRejectionReason as DesktopTorchRuntimePreviewRejectionReason,
  TorchRuntimeProbeOutcome as DesktopTorchRuntimeProbeOutcome,
  TorchReleaseOptionsOutcome as DesktopTorchReleaseOptionsOutcome,
  TorchVerifiedRuntime as DesktopTorchVerifiedRuntime,
  TrialTorchRuntimeOutcome as DesktopTrialTorchRuntimeOutcome,
} from '@pumas/desktop-contract';

// Declared once by the Rust producer (pumas-rpc contract) and generated.
export type TorchRuntimePreviewOutcome = DesktopTorchRuntimePreviewOutcome;
export type TorchRuntimePreviewRejectReason = DesktopTorchRuntimePreviewRejectionReason;
export type TorchRuntimePreview = DesktopTorchRuntimePreview;
export type TorchReleaseOptionsOutcome = DesktopTorchReleaseOptionsOutcome;
export type TorchRuntimeOptions = DesktopTorchRuntimeOptionsOutcome;
export type TorchInstalledConfig = DesktopTorchInstalledRuntime;
export type TorchVerifiedRuntime = DesktopTorchVerifiedRuntime;
export type TorchCapabilityProbe = DesktopTorchCapabilityProbe;
export type TorchRuntimeProbeReport = DesktopTorchRuntimeProbeOutcome;
export type TorchStartupTrialOutcome = DesktopTrialTorchRuntimeOutcome;
export type TorchAlternativeMatch = DesktopTorchAlternativeMatch;
export type TorchAlternativesOutcome = DesktopTorchAlternativesOutcome;

/** `general` for no extras, otherwise the extras the install added. */
export function torchExtrasLabel(extras: readonly string[] | null | undefined): string {
  if (!extras) return 'unknown extras';
  return extras.length === 0 ? 'general' : extras.join(' + ');
}

/** One line summary of an installed runtime's configuration and capabilities. */
export function torchInstalledSummary(config: TorchInstalledConfig): string {
  const named = (status: string) => config.capabilities
    .filter((capability) => capability.status === status)
    .map((capability) => capability.name.replaceAll('_', ' '));
  const available = named('available');
  const unavailable = named('unavailable');
  // Checks that ran and failed are not the same as checks that never ran.
  const capabilities = available.length > 0
    ? `serves ${available.join(', ')}`
    : unavailable.length > 0
      ? `no capability imports (${unavailable.join(', ')} unavailable)`
      : 'capabilities unchecked';
  return [
    config.build ?? 'unknown build',
    config.python ?? 'unknown Python',
    torchExtrasLabel(config.extras),
    config.qualification,
    capabilities,
  ].join(' · ');
}

export interface TorchRuntimePreviewRequest {
  tag: string;
  build: string;
  python: string;
  /** Former single-adapter choice, still accepted by the backend. Prefer `extras`. */
  adapter?: string;
  /** Optional extras added to the general modalities every install carries. */
  extras?: string[];
  /** Install the package versions a verified combination recorded (exact selection only). */
  verified_versions?: boolean;
}
