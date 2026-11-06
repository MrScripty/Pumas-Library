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

export interface TorchRuntimePreviewRequest {
  tag: string;
  build: string;
  python: string;
  adapter: string;
  /** Install the package versions a verified combination recorded (exact selection only). */
  verified_versions?: boolean;
}
