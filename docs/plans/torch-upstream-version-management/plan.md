# Plan: Upstream Torch Version Management

**Plan status:** `Active`

**Owner:** Pumas runtime integration

**Authorization:** The repository owner selected this scope on 2026-09-24 after
reviewing the active Torch deployment inventory. On 2026-09-25, the owner
authorized Pumas-managed stable CPython provisioning with no host-Python or
user-Python-selection requirement.

## Execution state

**Current phase:** The managed-CPython provider, automatic-choice UI, resolver
candidate scan, retained interpreter provenance, and Core-default profile have
passed code review and targeted Linux checks. Local Linux 0.7.0 AppImage and deb
packages were rebuilt from `ce9170d4`; artifact validation, standalone startup,
and packaged RPC health smoke pass, and the packed Electron bundle contains the
`get_torch_release_options` registration. In an isolated Linux RPC run, the
current source used managed CPython 3.14.7 to scan Torch 2.14.0 and resolve a
44-artifact CUDA 13.2 Core runtime preview (`python3.14`, adapter `none`). A
separate CPU Core preview resolved 25 artifacts and installed Torch 2.14.0
through its retained preview with managed CPython 3.14.7. The installer checked
Torch identity and CPU operation; the installed version was explicitly
selected. The install ran with `python`, `pip`, and `pypy` absent from the
backend's child `PATH`, then its isolated temporary root was removed. The
toolbar-linked package has not been replaced or published. Image evidence
remains limited to its exact previously recorded tuples.

**Blockers:** The v2.14.0 managed sidecar start/health/owned-stop path and CUDA
device use remain untested. Native Windows and macOS acceptance and provider
license inventory also remain pending.

**Next slice:** Exercise the installed `v2.14.0` runtime through managed
sidecar start, health/protocol validation, and owned stop. Then run native
Windows/macOS acceptance. Keep this Torch install path separate from FLUX.2,
image generation, and Tuldok claims, which require their own exact runtime
evidence.

## Product contract

Every stable upstream PyTorch release tag (`vMAJOR.MINOR.PATCH`) is discoverable
without a Pumas qualification allowlist when the complete GitHub listing fits
the 20-page / 120-second request budget. If a page fails or the budget is
exceeded, discovery returns an error and never presents a partial listing as
complete. A release is installable when an official CPU, CUDA, or ROCm binary
wheel matches the native host and a stable standard CPython provisioned by Pumas,
and the full sidecar dependency set resolves to binary artifacts. CPython 3.10
is the sidecar runtime minimum, with no configured upper minor cap. Candidate
Python versions come from wheel compatibility tags and the pinned managed
provider catalog; the manager selects the newest stable candidate whose complete
resolution succeeds. For dynamic choices, the retained preview records the exact
version, build, interpreter and provider identity, adapter, wheel URLs, and
hashes consumed by installation.

"Any release" describes discovery and exact-tuple resolution, not a promise
that every old tag has a wheel for every interpreter or build. Unsupported
combinations and network-inconclusive checks remain distinct. Pumas provisions
its own Python runtime and does not compile Torch from source in this plan. XPU
remains outside this plan's accepted provider/runtime contract. Windows and
macOS support is authorized but not yet accepted under the
[cross-platform Torch runtime plan](../torch-cross-platform-runtime-management/plan.md).
The fixed `v2.9.1` CUDA 13.0 / CPython 3.12 preset remains available as
a separately qualified, previously accepted exception backed by the embedded
hash-pinned requirements lock. Its preview exposes the three direct wheel URLs;
transitive wheel URLs are chosen from that lock during install, so this preset
does not claim the dynamic path's full per-wheel URL preview. Every other tuple
for `v2.9.1` uses the normal retained preview path and does not inherit the
preset's qualification.

Normal installation does not require the user to pick a Python interpreter.
The manager provisions its pinned Python provider, discovers exact official
wheels for the selected release and provider-catalog candidates, filters
GPU channels by detected display-device vendor, and returns a recommendation
when its bounded scan and host checks support one. The manager selects Python
automatically; advanced controls can select only a returned build/profile
combination. Every install still requires the
existing full dependency preview and retained hash lock. Core Torch runtime is
the default. On Linux, Pumas-owned FLUX.2 dependencies remain an explicit
advanced profile; they are not part of an upstream Torch release. Windows and
macOS offer only the core profile until a platform-specific adapter is accepted.

Installing a release does not activate it or make it the default. Selection,
default selection, sidecar startup, and stopping remain explicit version/profile
operations. A successful import or health check qualifies only its exact
runtime tuple and does not qualify every image adapter or model.

## Objective

Let a user discover stable PyTorch releases, review a complete compatible
official binary-wheel resolution, install into an isolated managed environment,
inspect the result, select the installed version, and start/stop its Torch
sidecar through the existing Pumas RPC API and desktop controls. Preserve the
existing Pumas gateway and Tuldok image workflow for exact qualified adapters.

The image-serving plan remains accepted for its recorded Nunchaku and FLUX.2
tuples. This plan expands the upstream runtime-management claim and owns the
updated PRG-I17 disposition.

## Acceptance

| ID | Observable criterion | Evidence required |
| --- | --- | --- |
| U1 | All stable upstream release tags are listed independently of Pumas recipes when the listing completes within the 20-page / 120-second budget; prereleases/nightlies are excluded; installed releases remain inspectable when upstream discovery is unavailable. Page failures and budget exhaustion are explicit errors, never silently truncated success. | Release fixtures including more than ten pages, end-of-list, page failure/no partial success, page-budget/deadline limits, cache completeness, filters, and offline installed-version query; live 63-tag discovery including v2.9.0. |
| U2 | Every dynamic build/interpreter selection runs an exact preview. Official origins, binary wheel compatibility, complete dependencies, hashes, and retained preview identity are validated before installation. The fixed qualified preset follows its separately documented hash-lock exception. | Python resolver and Rust preview tests; typed rejection values distinguish unsupported, invalid report, network-inconclusive, and generic-inconclusive outcomes without parsing diagnostic text; generated Electron/preload/React contract checks. |
| U3 | A non-preset release installs into staging from the retained lock, passes identity/core probes, and publishes only after validation. Failure/cancellation preserves installed, active, and default state. | Managed installation tests and an isolated real v2.9.0+cpu / CPython 3.12 installation on Linux x86_64; see the [RPC acceptance summary and artifact manifest](reports/v2.9.0-cpu-rpc-acceptance.json). |
| U4 | The installed runtime can be inspected, explicitly selected, started by a managed Torch profile, health/protocol checked, and stopped by its owned generation through Pumas RPC or the desktop. | Composed desktop test plus a real older-release RPC lifecycle on Linux x86_64; see the [RPC acceptance summary](reports/v2.9.0-cpu-rpc-acceptance.json). |
| U5 | The current real Tuldok image result remains accurately scoped to its exact Torch 2.10 CUDA 13.0 / FLUX.2 tuple; no general model/plugin or all-release image-generation support is inferred. | Reuse [2.10 GPU and Tuldok evidence](../torch-diffusion-serving/reports/upstream-v210-cu130-flux2-e2e.md); update the runtime inventory in [the progress report](../torch-diffusion-serving/reports/upstream-version-manager-progress.md). |
| U6 | For each selected stable release, the manager discovers that release's exact official CPU/CUDA/ROCm wheel matches for Pumas-managed CPython candidates, filters GPU families against detected display devices, and supplies a host-aware default when compatibility is established. Build and Pumas dependency profile remain optional advanced settings; Python selection is automatic. Partial discovery stays inconclusive; a wheel match still requires the full retained dependency preview. | Dynamic official-index scan fixtures and live `v2.14.0` scan; managed-provider candidate/fallback tests; NVIDIA driver-floor/unknown/partial-scan manager tests; typed RPC and generated-contract checks; desktop preview tests for automatic defaults, release-bounded overrides, null recommendations, and Pumas profile labeling. No all-release image qualification is inferred. |

## Ownership and verification

- `rust/crates/pumas-app-manager/src/version_manager/` owns official release
  selection, build/interpreter selection, retained previews, installation
  transactions, identity, selection, and process lifetime.
- `rust/crates/pumas-core/src/network/github.rs` owns complete upstream release
  pagination and cache completeness behavior.
- `torch-server/resolve_runtime.py` owns pip dry-run resolution and exact
  official binary artifact/hash provenance. It does not own runtime publication.
- `rust/crates/pumas-rpc/src/contract.rs` and
  `rust/crates/pumas-rpc/src/handlers/mod.rs` expose manager actions and the
  typed preview outcome; `rust/crates/pumas-rpc/src/contract/export.rs` owns its
  generated public type. `electron/src/preload.ts` and
  `frontend/src/components/TorchInstallPreview.tsx` validate and present that
  result.
- The focused Python, Rust, RPC, Electron, and frontend suites are necessary but
  not sufficient. Required-real acceptance installs and starts an older stable
  release through an isolated launcher root. The exact 2.10 real GPU/Tuldok run
  supplies image evidence for that tuple; it does not establish the desktop
  control flow or other tuples.
- Reproduce and classify the previously reported unrelated full Rust test
  failures before using the aggregate Rust gate as green.

## Implementation handoffs

The authorized implementation uses disjoint write sets:

1. Rust version-manager and Torch RPC source/tests.
2. Python Torch resolver and its tests.
3. Torch desktop components and focused composed lifecycle tests.
4. Root-owned plan, PRG-I17, evidence inventory, final integration, and commit.

The CPU/CUDA/ROCm channel vocabulary is shared between the Rust manager and
Python resolver. Changes to wire fields, generated contracts, the plugin
manifest, or other platform support require a new exact handoff. The separate
Windows/macOS handoff is
[Cross-Platform Torch Runtime Management](../torch-cross-platform-runtime-management/plan.md);
its pending acceptance does not change this plan's recorded Linux evidence.
