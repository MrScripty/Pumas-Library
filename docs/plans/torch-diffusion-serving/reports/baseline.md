# Baseline and evidence index

Planning baseline recorded 2026-09-13 from repository inspection and the preceding
session's host checks. These facts are not implementation acceptance.

- Torch already dispatches through the shared app-manager VersionManager and
  uses the RPC `inference-plugins` gate.
- The current source-release recipe points to `pytorch/pytorch`; the experimental
  `torch-server` does not provide the required complete diffusion-serving path.
- The gateway currently advertises ready loaded models; the broader discovery
  redesign is separately deferred in the [gateway brief](../../../breif/unified-inference-gateway.md).
- Tuldok `ai_http.py` currently discovers models and performs vision chat; it
  needs a separate image-generation request and browser workflow.
- Earlier host inspection identified RTX 5090 Laptop GPU, approximately 24 GB
  VRAM. Recheck live availability before inference qualification.
- Earlier command passed:
  `cargo check --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features --offline`.
  No release/gateway absence claim follows from this alone.

Implementation evidence: [runtime](runtime.md), [Nunchaku](nunchaku.md),
[Tuldok](tuldok.md), [FLUX.2](flux2.md), and
[release acceptance](release-acceptance.md). The facts above describe the
planning baseline, not the current implementation.
