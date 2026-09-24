# Plan: Tuldok Image Generation Through Pumas

**Plan status:** `Completed`

**Current phase:** A1–A7 acceptance is complete within the limits recorded in the evidence reports. A1 includes packaged upstream discovery and migration plus concurrent active/default selection and removal regression coverage. A5 includes CUDA allocator OOM and recovery, without claiming a naturally oversized pipeline request.

**Next slice:** No acceptance work remains in this plan. Native NVFP4 encoder serving, T16 desktop shutdown, LLaDA, other-platform qualification, and the broad gateway redesign remain deferred follow-ups.

**Independent companion preparation:** `M3-TIPC` source is complete in Tuldok
`a61daeec83779868cf03b14fb5c811fcdc608fc2`. Exact
`torch-runtime-0.1.6` candidate
`74f9b593dff1e447a73571dc465efd520238ba0c56d2730393a17eee2b97426c`
passes production installation and native GPU/sidecar validation; TIPC-09 is
returned. Real cancellation/reuse on the RTX 5090 Laptop GPU returns TIPC-04,
and the refreshed Tuldok 1280×720 display/save run returns TIPC-10. This does
complete the M3-TIPC real acceptance gate; no runtime publication was required.

**Acceptance status:** `Accepted`

**Brief:** [Torch diffusion serving](../../breif/torch-diffusion-runtime.md)

**Execution ledger:** [execution-ledger.md](execution-ledger.md)

**Issues:** [issues.md](issues.md)

**Evidence:** [Baseline and evidence index](reports/baseline.md)

**Provider correction dependency:** The
[Torch Image Provider Contract Correction](../torch-image-provider-contract/plan.md)
is the sole implementation owner for the reusable generation lifetime and image
live compatibility, private result-evolution, and public projection correction.
This plan retains Tuldok companion adaptation, exact corrected-candidate
qualification, required-real GPU/browser evidence, and distribution. Its
historical evidence remains valid only for the scope and bytes originally
recorded.

## Objective

From Tuldok, a user selects an image-generation model served by Pumas, submits a
prompt, and receives an image that is displayed and can be saved for dataset
work. Deliver this using the shared runtime installer and an inference-enabled
Pumas release, while preserving the optional inference-free serving build.

## Objective acceptance

| ID | Observable criterion | Kind | Environment | Mode | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | Shared runtime controls discover, install, activate, update and remove Torch; failed/cancelled installation preserves the usable version | integration | isolated install root; network for real artifacts | automated and manual | passed | [Packaged upstream acceptance](reports/a1-packaged-acceptance.md) |
| A2 | Nunchaku Z-Image-Turbo produces a decodable PNG through the Pumas gateway; capability discovery excludes text-only models from image selection | integration | RTX 5090 Laptop GPU, complete pipeline | automated and manual | passed | [Real image](reports/nunchaku.md) |
| A3 | Tuldok discovers a served image model, sends a prompt, displays the returned PNG and saves matching image content | end-to-end | real browser, Tuldok, Pumas release, GPU | automated and manual | passed | [Real Tuldok workflow](reports/tuldok.md) |
| A4 | The local FLUX.2 Klein FP8 checkpoint generates through the same Tuldok flow with its memory policy reported | end-to-end | same host, verified matching assets | manual | passed | [FLUX.2 FP8 image](reports/flux2.md) |
| A5 | Invalid inputs, missing assets, OOM, busy/unloaded model, cancellation and runtime exit terminate predictably without corrupting lifecycle or duplicating generation | integration | fault fixtures plus GPU cancellation and recovery | automated and manual | passed | [GPU OOM recovery](reports/a5-gpu-oom-recovery.md), [GPU cancellation and reuse](reports/nunchaku.md), and A5 route/lifecycle fixtures in the execution ledger |
| A6 | Inference-enabled release works; disabled build has no Torch/llama.cpp serving or management routes and no installation/startup side effects or controls | artifact and integration | enabled and disabled release builds | automated and manual | passed | [Release evidence](reports/release-acceptance.md) |
| A7 | Existing llama.cpp installation/serving and Tuldok VLM corner detection still work | regression | fixtures plus available Qwen VLM | automated and manual | passed | [Real VLM and fixture evidence](reports/tuldok.md), [shared manager tests](reports/runtime.md) |

## Scope and constraints

- Implement runtime packaging, installation, diffusion adapters, serving lifecycle,
  gateway contract, Pumas controls, Tuldok integration, and release verification.
- Reuse VersionManager and its state/progress/cancellation infrastructure. Only
  source resolution and installation mechanics vary by runtime.
- Use the existing `inference-plugins` gate and launcher configuration. Python
  dependencies remain external to the Rust executable.
- Target the verified Linux RTX 5090 Laptop GPU (approximately 24 GB VRAM) first;
  reject unsupported platform combinations clearly. Other-platform qualification
  is a follow-up, not an untested promise.
- Preserve concurrent unrelated repository changes and the current model-library
  mutation authority. Acquire pipeline assets through that authority.
- Model weights, runtime releases and serving state have distinct owners.
- First version returns one PNG per request. Image editing, batches, streaming
  previews, training, dataset orchestration, automatic model eviction, a general
  cold-load gateway redesign and LLaDA are outside scope.
- Tuldok repository: `/media/jeremy/OrangeCream/Linux Software/repos/owned/creative-media/Tuldok`.
  Implementation must read its current instructions before editing; this plan
  does not assume its files remain unchanged.
- Do not assume the previously used port 20617 is the Pumas gateway. It was the
  llama.cpp router. Use Pumas's actual advertised/configured gateway address.

## Binding decisions

| Decision | Owner | Basis |
| --- | --- | --- |
| Shared update lifecycle; Torch-specific compatible runtime recipe | Pumas app manager | User requirement; existing VersionManager |
| Same optional inference feature gate | Pumas RPC composition and launcher | User requirement; current Cargo feature |
| Stable library model IDs and existing asset acquisition authority | Pumas core | Current architecture and ongoing library work |
| Nunchaku first, local FLUX.2 second; LLaDA deferred | Torch adapter owner | User priorities and local inventory |
| Explicit model load before generation; advertise ready models with capabilities | Pumas serving manager/gateway | Existing loaded-model policy; deferred broader gateway brief |
| Single synchronous image response initially, bounded admission, explicit cancellation ownership | Gateway and Torch job owner | Small usable client contract without permanent job storage |
| Tuldok displays and saves the actual response | Tuldok client | User's end goal |
| Admitted image generation uses the generation-wide no-elapsed-deadline lifecycle; uncertainty is not replay authority | [Torch Image Provider Contract Correction](../torch-image-provider-contract/plan.md) | Explicit product contract `TIPC-GEN-01` |

## Proposed API contract

Finalize and document this bounded contract in M2 before integrating Tuldok:

- `GET /v1/models` retains its existing response and stable IDs; add an additive
  capability field containing `image_generation` for eligible ready models.
  Keep existing text clients compatible and expose no filesystem paths.
- `POST /v1/images/generations` accepts `model`, nonempty `prompt`, `n: 1`,
  numeric `width`/`height`, `response_format: "b64_json"`, and optional integer
  `seed` ([public contract](../../contracts/image-generation.md); the former
  `size` string is rejected). Provider execution uses the private
  [Torch provider protocol](../../contracts/torch-provider-protocol.md).
  Validate supported dimensions and pixel/response limits per adapter.
  Use documented model-specific defaults for steps and guidance initially.
- Return `{"created": <unix seconds>, "data": [{"b64_json": "<PNG bytes in base64>"}]}`.
  Record actual seed/settings in an additive metadata field for reproducibility;
  do not promise bitwise deterministic GPU output.
- Reject unsupported options and operation/model mismatches before allocation.
  Distinguish invalid input, unavailable model, busy runtime, OOM, cancellation,
  uncertain transport loss, and backend failure through stable safe outcomes.
  Elapsed duration and response silence are not image-generation failures.
- Reuse existing gateway access policy, routing and provider registry. A Torch
  sidecar address is an internal execution detail, not Tuldok configuration.
- Own each request through completion, backend/runtime failure, owner
  cancellation or disconnect, or explicit runtime shutdown. Cancellation is a
  request until cleanup has stopped work sufficiently for safe reuse; retain the
  GPU lease through that point. If interruption requires process termination,
  invalidate readiness and make reload explicit. Never retry after an uncertain
  result. Bound concurrency and reject excess requests as busy.

## Simplicity and ownership review

**Applicability:** `applicable`

1. **Independent concerns:** The app manager installs runtime versions on disk;
   the library resolves model/component identity; the serving manager owns ready
   model lifecycle; Torch executes GPU work; the gateway owns transport; Tuldok
   owns prompt entry and image presentation. Installation happens on explicit
   management actions; generation happens on a client request.
2. **State, identity, value, time, policy and mechanism:** Runtime recipe version
   identifies a validated dependency set, not a model revision or API version.
   Library IDs and immutable component revisions identify model inputs. Serving
   readiness and request leases are transient. Resource policy uses current
   telemetry, not cached installation success. Recipe compatibility declares
   allowed adapter/sidecar combinations; M1 records the concrete matrix. Changing
   runtime requires readiness invalidation/revalidation; changing model assets
   requires bundle validation. No second installed-version or model-state store.
3. **Caller/composition knowledge:** Tuldok knows URL, capability, model ID and
   image fields only. The RPC composition root wires one Torch provider through
   the existing registry. The app manager hides Python/wheel installation;
   adapters hide checkpoint layouts. UI does not choose interpreter paths.
4. **Representative changes:** A dependency upgrade changes the runtime recipe
   and its qualification evidence. A checkpoint-format change affects its
   adapter and asset validation. A public request-field change affects gateway
   contract and Tuldok client. Adding an image model must not require another
   updater, gateway transport, or UI backend-selection mechanism.
5. **Stable interfaces:** Library IDs, serving observations and the bounded image
   contract cross boundaries. Python paths, wheel names, quantization details,
   startup order and GPU cancellation stay with their owning implementations.
6. **Independent evolution/failure:** Install tests use isolated roots; adapters
   have fixture and GPU checks; gateway tests substitute a controlled sidecar;
   Tuldok tests use an HTTP fixture. Real end-to-end evidence is still required.
   Sidecar failure invalidates readiness without breaking library operations.
7. **Deletion results:** Replacing the incorrect source-release recipe removes a
   broken path. Reusing the manager avoids a second updater and state store.
   Removing the Torch adapter would spread pipeline knowledge into transport;
   removing the GPU lease would permit unload/use races. No new general plugin
   registry, scheduler service, job database or automatic loading framework.
8. **Necessary complexity:** Platform-compatible dependencies stay in the recipe
   strategy; model-format variation stays in two concrete adapters; resource
   ownership stays in the serving/job lifecycle. These additions compose with
   existing installation, library and gateway machinery. Revisit this review if
   implementation requires new authorities or shared version knowledge.

## Evidence and oracle plan

- A returned image must decode as PNG, have requested allowed dimensions and
  originate from the selected real model. An HTTP 200 or base64 string alone is
  insufficient. Save prompt, model/component revisions, runtime version, seed,
  settings, duration and peak GPU/RAM use beside acceptance images. Visually
  inspect prompt relevance; do not use a brittle exact-pixel golden test.
- Tuldok browser acceptance inspects the displayed image and saved decoded content;
  a sidecar-only script or mocked browser does not satisfy A3/A4.
- Installation failure fixtures fail during download, dependency installation and
  validation; assert old-version usability and absence of a published partial
  environment. Restart the manager to prove persisted state recovery.
- Gate tests exercise actual disabled routes/startup and UI build, not just cfg
  source searches. The earlier disabled cargo check is baseline evidence only.

## Milestones

M1 is `Completed` for qualified upstream `v2.9.1`: packaged discovery,
shared-UI installation, migration from the installed legacy runtime, activation,
removal, cancellation preservation, restart recovery, and the active/default
selection/removal concurrency regressions passed. M2 is `Completed` for its
real Nunchaku gateway and relevant A5 gates. M5 is `Completed` for the A1–A7
release and optional-build evidence. No Pumas-hosted Torch runtime release is
required. Original M3 and M4 are `Completed` within their recorded evidence;
`M3-TIPC` is `Completed` with its exact-candidate real gate passed. Remaining
work listed at the top of this plan is deferred follow-up, not a blocker to this
acceptance.
Each milestone
updates this directory's ledger, issues and evidence reports. Narrow write sets
against the live checkout before edits; expand the plan for new semantic owners.

### M1 — Usable Torch runtime through shared installation

**Goal:** Install and launch a qualified sidecar from the normal Pumas runtime controls.

**Allowed write set:** `rust/crates/pumas-app-manager/src/version_manager/`,
`rust/crates/pumas-core/src/config.rs` (the actual AppId release-source owner), `rust/crates/pumas-core/src/platform/{mod.rs,filesystem.rs,linux_group.rs}` (expose existing non-replacing publication and owned process-group observation primitives), its runtime tests and `rust/crates/pumas-app-manager/examples/qualify_torch_runtime.rs`,
`launcher-data/plugins/torch.json`, `torch-server/`, runtime packaging scripts
under `scripts/`, `frontend/src/components/app-panels/TorchPanel.tsx`, `frontend/src/components/VersionSelector.tsx`
(shared selector failure feedback), `frontend/src/components/InstallDialogHelpers.ts`
(installed-version visibility independent of release availability) and
`frontend/src/components/app-panels/sections/VersionManagerSection.tsx` if required
for existing shared control integration.

**Tasks:** Resolve exact supported Python/Torch/CUDA/Diffusers/Nunchaku artifacts
from primary documentation and verify on the GPU. Define a pinned recipe with
artifact integrity and sidecar protocol identity. Discover official release tags
from `pytorch/pytorch`, but expose only tags mapped to a qualified app recipe.
Embed the sidecar and lock with the Pumas app and install Torch from official
hash-pinned wheels. Stage isolated environments, validate imports/device support
and health, publish atomically through existing state, preserve rollback, and
refuse deletion/replacement of an active environment. Handle legacy installs
without deleting user assets. Exercise normal UI install/activate/remove and
install failure/cancellation/restart recovery.

**Gate:** Healthy real sidecar and existing llama.cpp installer checks before model
loading. Packaged discovery, fresh shared-UI installation, migration from the
installed legacy runtime, activation, removal, cancellation preservation, and
restart recovery passed for the supported upstream release; see
`reports/a1-packaged-acceptance.md`. Only v2.9.1 has a qualified upstream recipe,
so this does not claim an upgrade between two PyTorch upstream tags. Record exact
dependencies, supported tags and official wheel sources in `reports/runtime.md`.
**Re-plan:** No compatible supported official wheel combination or no distributable
sidecar/recipe path.

### M2 — Nunchaku prompt-to-image through Pumas

**Goal:** A real Nunchaku image returns through the public Pumas gateway.

**Allowed write set:** `torch-server/`,
`rust/crates/pumas-app-manager/src/torch_client.rs`,
`rust/crates/pumas-core/src/models/runtime_profile.rs`,
`rust/crates/pumas-core/src/providers/`, `rust/crates/pumas-core/src/serving/`,
`rust/crates/pumas-core/src/models/serving.rs` (existing provider/lifecycle owners),
`rust/crates/pumas-core/src/runtime_profiles.rs`,
`rust/crates/pumas-core/src/runtime_profiles/`,
`rust/crates/pumas-core/src/api/*runtime*.rs`,
`rust/crates/pumas-rpc/src/provider_clients.rs`,
`rust/crates/pumas-rpc/src/handlers/`, `rust/crates/pumas-rpc/src/server.rs`,
relevant tests in these crates, and new image API documentation under `docs/contracts/`.

**Tasks:** Resolve complete Nunchaku pipeline components through existing library
APIs; use the FP4 checkpoint appropriate to the verified GPU. Add a concrete image
adapter and lifecycle-safe GPU execution. Integrate Torch provider readiness,
load/unload and image capabilities with the existing serving path. Implement the
bounded image contract, error mapping, response limits, and cancellation. The
focused provider correction now solely owns the shared generation lifetime plus
image live compatibility, private result evolution, and gateway projection;
this milestone consumes its accepted source and evidence rather than
implementing a duplicate.
Keep every executable integration under the existing inference gate. Measure a
real prompt before adding the second adapter.

**Gate:** A2 and the relevant A5 cases, plus regression checks for text routing.
Record request/response, decoded PNG and resource measurements in `reports/nunchaku.md`.
**Re-plan:** Asset format mismatch, resource policy cannot run within host limits,
or existing lifecycle cannot safely represent Torch without additional ownership.

### M3 — Tuldok generation workflow

**Goal:** A user generates, views and saves a Nunchaku image from Tuldok.

**Allowed write set:** Pumas `frontend/src/components/app-panels/`,
`frontend/src/components/model-serve/`, `frontend/src/hooks/useTorchProcess.ts`, `frontend/src/hooks/useServingStatus.ts`,
`frontend/src/utils/runtimeProviderDescriptors.ts`, `frontend/src/types/api-runtime-profiles.ts`
(shared provider declarations and serving status decoder),
associated client bindings only as required by M2 contracts; Tuldok `ai_http.py`,
`image_generation.py` (bounded image transport and cancellation), `app.py` (existing HTTP route owner), `static/app.js`,
`static/index.html`, associated styles, tests and user documentation.

**Historical tasks:** Expose the gateway URL and managed Torch serving state in Pumas. Add
Tuldok image-generation controls separate from corner detection, capability-filtered
selection-only model list, prompt, size and optional seed, pending/cancel/error
states, image display and save. Decode bounded validated image responses; preserve
existing corner workflow. Provide a useful unsupported-endpoint message
when configured with the llama.cpp router instead of Pumas gateway.

**Historical gate:** Browser fixture coverage for failure/cancel/output handling
and the recorded real Tuldok-to-Pumas image display/save workflow passed within
the original candidate, resolution, and protocol scope in `reports/tuldok.md`.

**Status:** `Completed` within that historical scope.

#### M3-TIPC — Tuldok contract companion

**Goal:** Align the existing Tuldok image consumer with `TIPC-GEN-01` and return
TIPC-10 without changing discovery, datasets, or VLM transport.

**Status:** `Completed`

**Allowed write set:** Tuldok `image_generation.py`, its focused tests,
`tests/browser_images_real.cjs`, relevant user documentation, and this plan's
`reports/tuldok.md`, ledger, and issues.

**Completed source tasks:** Preserve the already-correct numeric width/height request, requested
dimensions, 1280×720 defaults, and explicit socket/watcher cancellation. Remove
the remaining consumer generation deadline and deadline-specific outcome;
preserve uncertain outcomes without automatic replay; and avoid narrowing
legitimate public metadata to current examples. Tuldok commit `a61daee` passed
all 47 unit tests.

**Gate:** TIPC-10: a supporting model completes a real 1280×720 Tuldok-to-Pumas
request through the exact installed corrected candidate, displays it, and saves
it. Record new evidence without relabeling the earlier smaller-image result.

This companion may be prepared against the current public contract independently
of TIPC-M1 source completion, but its required-real gate waits for the exact
installed corrected candidate.

**Result:** Passed against isolated production-installed
`torch-runtime-0.1.6`. The browser displayed 1280×720, saved exact PNG SHA-256
`cdf9ae632f621606032d91975f86f960d546b39349a83ce6b851b50390fb52a3`,
and automatically imported it. TIPC-10 is returned.

**Re-plan:** Tuldok's current app structure or dataset import boundary materially differs.

### M4 — FLUX.2 through the same path

**Goal:** The local FLUX.2 Klein FP8 model works through the existing image flow.

**Allowed write set:** `torch-server/` adapters/tests, M1 runtime recipe only if
qualification requires it, and model-component declarations through existing
library-owned interfaces; existing `serving_torch.rs`, Torch client load DTO and
gateway ready-slot adapter filter for the second concrete adapter. No new client
transport or updater.

**Tasks:** Verify the actual local checkpoint layout and matching encoder/VAE/
tokenizer assets. Implement its concrete adapter, model-specific size/settings
validation and measured explicit offload policy. Check both adapters against the
same qualified environment; avoid silently upgrading dependencies at model load.

**Gate:** A4 through Tuldok, Nunchaku regression, memory/latency and image evidence
in `reports/flux2.md`.
**Re-plan:** Checkpoint requires unsupported conversion or incompatible runtime
versions; decide supported recipe versions before extending machinery.

### M5 — Release and optional-build acceptance

**Goal:** Deliver a tested release and prove the library-only build remains optional.

**Allowed write set:** Relevant tests from M1–M4, `scripts/launcher/`, existing
Cargo feature declarations if gate closure requires changes, related frontend
feature guards, release documentation, and this plan's evidence. Product fixes
return to their owning milestone rather than broadening this slice implicitly.

**Tasks:** After the focused correction passes its owned gate, construct an
identifiable `torch-runtime-0.1.6` candidate (rechecking that allocation first)
and record source revision, recipe, lock, archive checksum, bundled sidecar
protocol/capabilities, and client interoperability. Install those exact bytes
through production VersionInstaller in an isolated launcher root while preserving
existing installations. Build the normal release and run the actual Tuldok flow
for both models.
Build with `PUMAS_INFERENCE_PLUGINS=false`, check route/control absence and no Torch
startup/install side effects. Exercise runtime failure, cancel followed by another
request, unload during work, insufficient memory, and existing llama.cpp/Tuldok
VLM behavior. Use current GPU telemetry before loads and explicitly manage
competing models. Record usable latency/defaults and known limitations; preserve
evidence rather than declaring success from build completion.

**TIPC handoff gate:** TIPC-04, TIPC-09, and TIPC-10 were returned individually
after their specific evidence passed. They do not require M5 closeout or a
Pumas-hosted runtime publication.

**Milestone gate:** All A1–A7 are satisfied with release identity and
reproducible commands in `reports/release-acceptance.md`. No required installed-
artifact, GPU, or browser check may be replaced by mocks. Qualification does not
authorize publication or production-default selection.
**Re-plan:** Any required acceptance fails, the selected model does not support
1280×720, or the reserved recipe identity collides; fix/select a supported model
or identity and rerun only the affected evidence.

## Blockers and unresolved facts

M1 source implementation, exact-candidate package qualification, GPU
cancellation/reuse, and browser acceptance are complete for TIPC-04, TIPC-09,
and TIPC-10. The earlier assumption that Torch discovery required a Pumas-hosted
runtime bundle was incorrect. The manager now targets official PyTorch releases
and maps only the qualified `v2.9.1` tag to an embedded, hash-locked recipe.
Packaged discovery, install, migration from the installed legacy runtime,
activation, removal, cancellation preservation, and restart recovery passed;
see [A1 packaged acceptance](reports/a1-packaged-acceptance.md). Only v2.9.1 has
a qualified upstream recipe, so an upstream-to-upstream upgrade is not claimed.
Both complete pipelines now generate through Tuldok. A5 acceptance passed,
including a real
CUDA allocator OOM returning sanitized HTTP 507; follow-up inference succeeded
with the same loaded model, and teardown succeeded. This does not verify a
naturally induced high-resolution pipeline OOM. See [A5 GPU recovery evidence](reports/a5-gpu-oom-recovery.md),
[runtime evidence](reports/runtime.md), and [issues](issues.md).

## Re-plan triggers

- Additional installation or model-state authority appears necessary.
- Compatible dependencies cannot support the selected checkpoints or host.
- Concurrent changes alter serving, library acquisition or Tuldok contracts.
- New gateway auto-loading or broader discovery becomes necessary for acceptance.
- Required release/GPU/browser evidence is unavailable or fails.

## Final acceptance

- Acceptance status: `passed`.
- Deferred follow-ups: native NVFP4 encoder serving, T16 desktop shutdown,
  LLaDA, other-platform qualification, and the broad gateway redesign; revisit
  after the user selects the next scope.
- Final status: `Completed`; operation `start` admitted on 2026-09-13. A1–A7 acceptance was completed on 2026-09-23 within the limitations in the linked reports.
