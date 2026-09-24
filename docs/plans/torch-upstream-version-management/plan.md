# Plan: Upstream Torch Version Management

**Plan status:** `Completed`

**Owner:** Pumas runtime integration

**Authorization:** The repository owner selected this scope on 2026-09-24 after
reviewing the active Torch deployment inventory.

## Execution state

**Current phase:** U1–U4 are accepted for the approved Linux x86_64 / installed
CPython scope. Focused tests, production builds, generated-contract checks, and
read-only cross-layer reviews passed; the real v2.9.0 CPU install and managed RPC
lifecycle passed on 2026-09-24. U5 retains the exact Torch 2.10 CUDA 13.0 / FLUX.2
Tuldok evidence. A real interactive Pumas desktop image run and image generation
across other Torch tuples are not claimed by this plan.

**Blocker:** None within the authorized scope.

**Next slice:** No acceptance work remains in this plan. A real desktop-driven
Tuldok repeat or broader user-defined image-model plugin contract would need a
separately scoped follow-up.

## Product contract

Every stable upstream PyTorch release tag (`vMAJOR.MINOR.PATCH`) is discoverable
without a Pumas qualification allowlist when the complete GitHub listing fits
the 20-page / 120-second request budget. If a page fails or the budget is
exceeded, discovery returns an error and never presents a partial listing as
complete. A release is installable when the user can select an official CPU,
CUDA, or ROCm binary wheel that matches Linux x86_64 and an already installed
CPython 3.10–3.13 interpreter, and the full sidecar dependency set resolves to
binary artifacts. For dynamic choices, the retained preview records the exact
version, build, interpreter, adapter, wheel URLs, and hashes consumed by
installation.

"Any release" describes discovery and exact-tuple resolution, not a promise
that every old tag has a wheel for every interpreter or build. Unsupported
combinations and network-inconclusive checks remain distinct. Pumas does not
provision Python or compile Torch from source in this plan. XPU and operating
systems other than Linux x86_64 remain outside the supported provider/runtime
contract. The fixed `v2.9.1` CUDA 13.0 / CPython 3.12 preset remains available as
a separately qualified, previously accepted exception backed by the embedded
hash-pinned requirements lock. Its preview exposes the three direct wheel URLs;
transitive wheel URLs are chosen from that lock during install, so this preset
does not claim the dynamic path's full per-wheel URL preview. Every other tuple
for `v2.9.1` uses the normal retained preview path and does not inherit the
preset's qualification.

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
manifest, or other platform support require a new exact handoff.
