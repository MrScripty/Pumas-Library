# Issues: Cross-Platform Torch Runtime Management

## XP-1 — Manager and resolver hard-code Linux wheel identity

- **Severity:** High; blocks Windows/macOS exact resolution.
- **Evidence:** `torch-server/resolve_runtime.py:254–271` accepts only Linux
  x86_64 interpreter tags; `:302–305`, `:578–580`, and `:745–749` assume a
  `release+build` distribution version. macOS official CPU wheels can use the
  plain upstream release version. Rust repeats the Linux identity check in
  `torch_preview.rs:798–800` and resolver validation at `:949–957`.
- **Owner/boundary:** Python resolver and app-manager retained-preview boundary.
- **Disposition:** Fix in M1. Retain upstream distribution version separately
  from selected release/build and compare full official wheel tags.
- **Verification:** Resolver fixture/native wheel checks, serialized resolver
  manifest tests, and retained-preview validation.
- **Status:** M1 code and fixture tests complete. Native official-index scans
  remain part of M2 acceptance.

## XP-2 — Host detection and interpreter discovery are Linux-only

- **Severity:** High; blocks exact host-aware choices.
- **Evidence:** `torch_alternatives.rs:83–121` scans Linux PCI paths;
  `:150–160` encodes Linux CUDA floors; `:224–245` and `:478–495` invoke
  `python3.10`–`python3.13` and require Linux x86_64.
- **Owner/boundary:** `pumas-app-manager` release-options discovery.
- **Disposition:** Fix in M1 with target-native Python discovery and
  conservative evidence-backed host recommendations. Keep CPU as Windows
  default until Windows driver compatibility evidence exists.
- **Verification:** Native Windows/macOS interpreter and hardware fixtures;
  live official index scan on each target.
- **Status:** M1 implementation and cross-target fixtures complete. Native
  Windows/macOS discovery and index scans remain part of M2 acceptance.

## XP-3 — Runtime paths and lifecycle reject or assume Linux

- **Severity:** Critical; blocks safe install cleanup and sidecar ownership.
- **Evidence:** Torch install/identity paths contain `venv/bin/python` in
  `torch_preview.rs:684,730,1067`, `installer/torch.rs:361,697`,
  `installer.rs:1636`, and `state.rs:663`; app manager resolver/installer
  process groups are Linux-only (`torch_preview.rs:543–639`,
  `installer/torch.rs:783–830`); persistent sidecar ownership rejects every
  non-Linux target in `pumas-core/src/runtime_profiles/process_owner.rs:166–175`
  and puts `process_group` under Linux-only code at `:718–756`.
- **Owner/boundary:** Native path utility, installer/resolver owners, and
  `pumas-core` persistent process owner.
- **Disposition:** Fix in M1 using OS-native venv path APIs and process-tree
  ownership that retains one exact process generation through cancellation,
  stop, and shutdown.
- **Verification:** Native process-tree/Job lifecycle and path tests plus real
  RPC start/health/stop on Windows and macOS.
- **Status:** M1 implementation and Linux regression tests complete. Native
  Windows/macOS lifecycle and RPC acceptance remain part of M2.

## XP-4 — Current model evidence does not establish Windows/macOS inference

- **Severity:** Scope risk, not a Torch runtime-management blocker.
- **Evidence:** The existing runtime inventory and Tuldok report qualify
  distinct Linux-only Torch tuples; MPS availability and successful CPU import
  do not qualify the configured image adapters.
- **Owner/boundary:** Torch model adapter and Tuldok image-serving acceptance.
- **Disposition:** Explicitly defer image generation by FLUX.2, Nunchaku, or
  any model on Windows/macOS. Keep the accepted exact tuples unchanged.
- **Verification:** Do not add or infer image-generation claims in M1. Any
  future platform/model support requires target-specific dependency and real
  image evidence.
- **Status:** Deferred as designed; no Windows/macOS adapter or image-generation
  claim is added by this plan.

## XP-5 — Preview identifiers use a Linux-only random source

- **Severity:** High; blocks secure retained previews on Windows.
- **Evidence:** `torch_preview.rs` currently opens `/dev/urandom` to create
  retained preview identifiers. That device path does not exist on Windows.
- **Owner/boundary:** Torch preview token creation in `pumas-app-manager`.
- **Disposition:** Fix in M1 using `getrandom 0.3.4`, already present in the
  workspace lockfile, declared directly at the app-manager consumer. Preserve
  cryptographic randomness and return a typed failure if the OS RNG fails.
- **Verification:** Token generation tests on Linux, Windows, and macOS; no
  time-, process-, or hostname-derived fallback.
- **Status:** M1 implementation and Linux tests complete. Windows/macOS runtime
  token-generation execution remains part of native M2 acceptance.
