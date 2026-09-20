# Plan: Torch Image Provider Contract Correction

**Plan status:** `Active`

**Objective:** Establish one reusable generation lifetime in which an admitted
image, text, or future generation remains active until completion,
backend/runtime failure, owner cancellation or disconnect, or explicit runtime
shutdown. Correct the Torch image provider against that lifetime, prove live
compatibility before admission, and preserve the public image contract and
immutable runtime installation model.

**Acceptance status:** `pending`

**Current phase:** `TIPC-M1` source is committed at Pumas
`826a270958f7182bd8108c879b4b7fa1da5cab7d`; the Tuldok companion is committed
at `a61daeec83779868cf03b14fb5c811fcdc608fc2`. Controlled Pumas, sidecar, and
Tuldok evidence passes. The qualified native Torch failure test and the exact
candidate/GPU/browser handoffs remain pending in their declared environments.

**Next integration slice:** execute the updated native failure test in the
qualified Torch environment, then hand source commit `826a2709` to the Tuldok
image plan for exact-candidate construction and TIPC-04/TIPC-09/TIPC-10.

**Execution ledger:** [execution-ledger.md](execution-ledger.md)

**Issues:** [issues.md](issues.md)

## Baseline and placement

- Pumas revision reviewed: `74d4f7f0a5345e2dba476313d9cb409f4fceeb61`.
- Coding-Standards revision reviewed:
  `366c1d90a24bbfb50973f62b155a5f3396c0f107`.
- Tuldok revision reviewed: `6e9e6ec32d4dd719af0e051baafacecd190864c2`.
- Material local differences at reconciliation: untracked
  `docs/breif/future.md` was unrelated and remains untouched. The user-supplied
  `docs/plans/torch-provider-lifetime-and-protocol-correcitons.md` was a matching
  draft and has been reconciled into this canonical plan rather than retained as
  competing authority. Tuldok had no local changes.

This focused plan is warranted because the existing
[Tuldok image plan](../torch-diffusion-serving/plan.md) is already active at
distribution acceptance and owns broader UI/runtime delivery. Moving this
independent contract correction into its current slice would obscure acceptance
and displace its priority. This plan is the sole executable owner of the
reusable Pumas generation lifetime and the Pumas/Torch image correction; the
existing plan owns only the named Tuldok and exact-candidate handoffs below.

This is a revision baseline, not an exact-HEAD implementation restriction.
Implementation must recheck relevant source and local changes before editing.

## Boundary and binding decisions

| Concern | Binding owner and decision |
| --- | --- |
| Generation transport/lifetime for `/v1/chat/completions`, `/v1/completions`, `/v1/images/generations`, and future generation routes | `pumas-rpc`; one connection-bounded, duration-unbounded policy |
| Public `POST /v1/images/generations` decoding, routing, and public response/error projection | `pumas-rpc`; keep public and private contracts separate |
| Private `POST /api/images/generate` encoding/decoding and compatibility interpretation | `TorchClient` in `pumas-app-manager` |
| Image inference, cancellation, worker and device ownership | `torch-server` |
| Installed artifact identity and qualification | Existing VersionInstaller/qualifier; immutable archives remain authoritative |
| Lifetime | No admitted generation has a total, response-read, idle, or elapsed-duration deadline; a justified connection-establishment budget is not a generation budget |
| Image execution shape | Synchronous request/response with bounded admission; no durable jobs, reconnection, resumption, public cancellation API, or automatic replay |
| Compatibility | Use the existing protocol-version mechanism and allocate Torch protocol `3`; do not add a parallel lifetime capability or compatibility registry |
| Candidate identity | Reserve unused recipe identity `torch-runtime-0.1.6` for the corrected candidate; publication and default activation remain separately authorized |
| Contract evolution | Requests remain closed and strictly typed; valid results require the canonical fields but permit unknown additive private result fields within the whole-payload bound |

Protocol `3` is selected because the inspected provider and recipe are protocol
`2`, no Torch protocol `3` allocation exists, and a protocol-2 sidecar can accept
the request while retaining the old deadline. Exact protocol evolution therefore
rejects that ambiguity without inventing a second capability mechanism. The
installed repository inventory contains `0.1.1`–`0.1.4`, the source recipe names
`0.1.5`, and the reviewed GitHub release inventory contains no Torch runtime
asset; `0.1.6` is consequently reserved for this candidate. Recheck that
allocation immediately before construction.

### Generation lifetime contract

`TIPC-GEN-01`: the explicit Pumas product contract is that elapsed duration and
response silence never make an admitted generation fail. This applies to image,
text, streaming, non-streaming, and future generation operations; it is the
generation rule, not an image exception. The explicit Pumas generation contract
takes precedence over the generic remote-operation deadline requirement for
generation operations.

Connection establishment, health, startup, model loading, installation,
non-generation inference such as embeddings, cancellation cleanup, and shutdown
may retain independently justified budgets. Those budgets must not become a
maximum generation duration. Bounded admission, busy rejection, retained
resource ownership, explicit cancellation, and operator stop prevent this
contract from authorizing an unbounded queue or detached work. Record the
generation-wide policy in the appropriate architecture/contract owner and amend
ADR 0002 for its Torch image application during implementation; do not modify
Coding-Standards in this repository task.

## Required behavior

### Shared generation lifetime, cancellation, and uncertainty

- Give all public generation routes one transport/lifecycle policy with a
  connection budget but no client-wide, per-request total, response-read, or
  idle deadline. Apply it to `/v1/chat/completions`, `/v1/completions`, and
  `/v1/images/generations`, and make it the required seam for future generation
  routes. Do not apply it to models, embeddings, health, startup, loading,
  installation, or shutdown merely because they share an HTTP client today.
- Remove the shared gateway/client limits that currently cap chat/completions,
  the Python 600-second image deadline, the Rust image transport deadline,
  deadline-derived generation outcomes, and the stale 615-second image policy.
  Trace middleware and intermediaries so no inherited timeout recreates a
  generation limit.
- An owning request abort or connection loss requests cancellation through the
  real gateway/private route. Distinguish cancellation requested, cleanup or
  termination pending, cancellation completed, and runtime/process failure
  through existing lifecycle/diagnostic mechanisms.
- Each generation provider retains its admitted work and resources until its
  own completion or sufficient cleanup. Dropping the gateway future or provider
  connection requests cancellation but does not prove provider work stopped.
- Retain worker and device custody until the underlying work has stopped enough
  for safe reuse. Dropping a Rust future, Python task, or thread wrapper is not
  proof of termination. Refuse unsafe unload or device reuse while execution or
  cleanup is active.
- A connected stalled operation may retain its admitted resources until a
  terminal event or authorized stop. Transport loss before a terminal result is
  an unknown outcome: never claim compute stopped and never replay automatically.
- Streaming bytes are progress information, not lease renewal. A connected
  generation may also legitimately remain silent until its terminal result.

### Current route and proof boundary

| Route or surface | Current provider/response path | TIPC-M1 proves | Provider-lifecycle owner and disposition |
| --- | --- | --- | --- |
| `POST /v1/chat/completions` | Ollama and llama.cpp through the same generic gateway handler, shared HTTP client, and buffered `proxy_response` | Actual registry/route wiring uses the duration-unbounded generation transport; one controlled long/silent transport integration plus wiring tests covers the shared implementation | Ollama `OllamaProviderApi` and llama.cpp `LlamaCppRuntime` serving owners retain provider-native cancellation/cleanup. TIPC-12 does not claim their external workers stopped. |
| `POST /v1/completions` | Ollama and llama.cpp through that same generic buffered path | Same shared-construction proof as chat; route tests must fail if either provider bypasses the generation transport | Same provider owners; provider cleanup evidence remains with each provider lifecycle owner. |
| `POST /v1/images/generations` | Torch-only gateway adapter → TorchClient → synchronous private image endpoint | Image transport/lifetime, disconnect propagation, uncertainty, compatibility, and projection under TIPC-01–TIPC-03/TIPC-05–TIPC-08/TIPC-11 | This plan owns sidecar cleanup evidence; the Tuldok image plan owns required-real GPU reuse evidence TIPC-04. |
| Torch text handlers | Not registered for public chat/completions; current sidecar handlers call `_generate` synchronously | Nothing; this surface is not an implicit prerequisite or TIPC-12 representative | [Desktop/Torch remediation Milestone 3](../current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/plan.md#milestone-3-make-the-torch-surface-truthful-and-schedulable) owns its lifecycle/responsiveness. |
| Future generation route | Not yet admitted or verified | Nothing until its route is implemented through the generation transport | Admission requires `TIPC-GEN-01` plus a named provider cancellation/cleanup owner and fit-for-claim evidence. |

One controlled non-image request may decide the transport behavior because
chat and completions for both registered providers necessarily traverse the same
handler, client, and buffered response function. Separate registry/route tests
prove that shared construction. The inference does not cross the provider
boundary: a dropped Pumas request proves a cancellation signal/connection loss,
not Ollama or llama.cpp worker cleanup.

The named Pumas integration owners for those provider boundaries are
`rust/crates/pumas-rpc/src/handlers/serving_ollama.rs` and
`rust/crates/pumas-rpc/src/handlers/serving_llama_cpp*.rs`, together with the
external Ollama and llama.cpp implementations that own inference after request
admission. No provider-native cleanup acceptance is inferred here; a future
Pumas claim about either provider requires evidence admitted by that owner.

### Live compatibility and readiness

- Make handshake fields strict. Distinguish malformed handshake, protocol
  mismatch, missing capability, connection unavailability, process/profile
  replacement, model-load failure, busy state, and unsupported operation at
  their owning boundaries.
- Verify the running sidecar's protocol and required capability before model
  load/publication and again before admission where process ownership could have
  changed. Bind the observation to the existing process/profile identity; an
  endpoint URL or prior successful model listing is insufficient after
  replacement. Keep ready-slot and supported-operation checks independent.
- Runtime qualification checks artifact/checksum, recipe identity/environment,
  recipe-declared protocol, and actual bundled sidecar protocol/capabilities.
  Recipe-to-sidecar agreement proves internal consistency; client-to-live-
  sidecar agreement proves interoperability. A recipe label, listener liveness,
  or successful health request alone proves neither compatibility, readiness,
  nor the revised lifetime behavior.
- Rollback selects a compatible client/provider pair, not merely an older
  runtime. Existing installations remain intact.

### Requests, results, and projections

- Keep private requests closed and typed, including numeric dimensions,
  required fields, supported semantics, and the current unknown-field policy.
- Require valid `png_base64`, `seed`, `steps`, `guidance`, `memory_policy`, and
  `duration_seconds`. Preserve canonical PNG, dimensions, base64, and size
  obligations without inventing ranges or closed enums from current examples.
- Accept unknown additive result fields, including `peak_vram_bytes`, while
  enforcing the response-size bound over the entire received payload. Reject
  invalid required data and oversized payloads.
- Project only public-owned fields. Do not expose unknown private fields, paths,
  tracebacks, or arbitrary provider messages. Preserve safe typed errors and
  remove generation-deadline outcomes. A connection-establishment failure is an
  unavailable/connect outcome, never proof that admitted generation was
  cancelled or completed.

### Knowledge owners updated with implementation

Add `docs/contracts/generation-lifetime.md` as the single generation-wide policy
owner and link that boundary from `docs/ARCHITECTURE.md`. Update
`docs/contracts/torch-provider-protocol.md`,
`docs/contracts/image-generation.md`, `torch-server/README.md`, relevant module
and qualification documentation, and ADR 0002 without duplicating the general
policy. The repository has no separate ADR amendment convention, so append a
dated amendment to ADR 0002 that preserves its historical decision and replaces
only its Torch-specific lifetime, compatibility, and result-evolution clauses.

The private protocol must name both directions: Rust TorchClient produces the
generation request; Python produces health/protocol/capability, generation
result, and provider-failure messages consumed by Rust; Pumas gateway produces
the public response/error consumed by the external application. Historical GPU
reports keep their original scope. Durable product docs must not describe the
new behavior before the implementation exists.

## Acceptance and evidence ownership

Each claim has one execution/evidence owner. Referencing plans may consume the
evidence but do not independently prescribe or re-prove the same correction.

| ID | Claim and deciding evidence | Kind | Environment | Mode | Execution/evidence owner | Status |
| --- | --- | --- | --- | --- | --- | --- |
| TIPC-01 | Elapsed time beyond the former limit does not cancel valid work — controlled worker/clock regression | integration | representative controlled worker/clock | automated | This plan | passed |
| TIPC-02 | Production image transport permits a connected, silent, long request — configuration inspection plus controlled transport integration | integration | representative real gateway/private transport | automated | This plan | passed |
| TIPC-03 | Disconnect and explicit owner cancellation propagate while resources remain held through pending cleanup — current-route transport/lifecycle evidence | system | representative real Pumas/Torch route with controlled worker | automated | This plan | partial — controlled route/worker passed; qualified native run pending |
| TIPC-04 | Actual GPU cancellation permits reuse only after sufficient worker/device cleanup — required-real device evidence | system | required-real supported GPU and exact candidate | either | [Tuldok image plan M5](../torch-diffusion-serving/plan.md#m5--release-and-optional-build-acceptance) | pending |
| TIPC-05 | Malformed, incompatible, missing-capability, unavailable, and replaced runtimes cannot gain false listing/readiness/admission — handshake, serving, listing, and admission tests | integration | representative controlled sidecars/process replacement | automated | This plan | passed |
| TIPC-06 | Artifact consistency and live interoperability are independently checked, including mixed versions — installer/qualifier and client tests | integration | representative isolated installer plus mixed clients/sidecars | automated | This plan | passed |
| TIPC-07 | Additive results succeed while invalid required data and oversized whole payloads fail — decoder contract tests | contract | deterministic decoder fixtures | automated | This plan | passed |
| TIPC-08 | Private additions do not leak publicly and safe error projection remains coherent — gateway projection tests | integration | representative gateway/provider fixtures | automated | This plan | passed |
| TIPC-09 | The exact candidate installs and runs through production installation — isolated installed-artifact qualification | release-artifact | required-real isolated launcher root and candidate bytes | either | [Tuldok image plan M1/M5](../torch-diffusion-serving/plan.md) | pending |
| TIPC-10 | A supported real 1280×720 request traverses Tuldok → Pumas → installed candidate, displays, and saves — required-real browser/GPU evidence | user-workflow | required-real browser, GPU, Tuldok, Pumas, exact candidate | either | [Tuldok image plan M3-TIPC](../torch-diffusion-serving/plan.md#m3-tipc--tuldok-contract-companion) | pending |
| TIPC-11 | A lost response preserves uncertainty and causes no automatic replay — controlled transport-failure evidence | integration | representative controlled gateway/private transport | automated | This plan | passed |
| TIPC-12 | Registered non-image generation routes necessarily use the shared duration-unbounded Pumas transport; a connected silent request has no duration/byte-cadence failure, disconnect drops the Pumas request, and Pumas does not replay. This claim does not assert provider-native cleanup. | integration | representative real route wiring plus controlled gateway/provider transport | automated | This plan | passed |

Controlled time must replace ten-minute regression waits. A short wait does not
prove the absence of a long production timeout, and harness watchdogs are not
production generation policy. Fixtures cannot satisfy real route, installed-
artifact, GPU, or browser claims. Source completion may precede environment-
gated evidence, but this correction is not accepted until all twelve claims,
including the three named handoffs, pass.

## TIPC-M1 — shared generation lifetime and provider correction

**Status:** `Active — source complete; qualified native evidence pending`

**Goal:** Establish the shared Pumas generation lifetime and correct the live
Pumas/Torch image compatibility and result contract as one implementation unit.

**Allowed write set:** `torch-server/{image_api.py,model_manager.py,serve.py,validate_runtime.py,README.md,runtime/runtime.json,tests/**}` and
`torch-server/tests_native/test_image_failures.py`;
`rust/crates/pumas-app-manager/src/{lib.rs,torch_client.rs}` and focused tests;
`rust/crates/pumas-app-manager/src/version_manager/installer/{torch.rs,torch_tests.rs}`;
`rust/crates/pumas-rpc/src/handlers/{serving_torch.rs,openai_gateway.rs,openai_gateway_images.rs,openai_gateway_tests.rs}`;
`rust/crates/pumas-rpc/src/server.rs` where it owns the active shared gateway
client; `docs/contracts/generation-lifetime.md`, `docs/ARCHITECTURE.md`, and the
three image/Torch contract/ADR documents named above; relevant existing module
documentation; and this plan directory.
Dependency locks, public runtime defaults, release metadata, and Tuldok are
excluded. Expand only after recording a newly proved semantic owner.

`torch-server/model_manager.py` was added during execution after the existing
repeated load-cancellation test proved that its shield loop could retain task
cancellation indefinitely instead of observing the executor worker's terminal
state. It is the load/device-custody owner for the same cleanup invariant; no
unrelated model-loading behavior is admitted by this expansion.

`rust/crates/pumas-app-manager/src/lib.rs` was added after the RPC integration
proved it is the public re-export owner for the new typed Torch handshake and
image errors consumed by `pumas-rpc`; no unrelated app-manager API is admitted.

**Tasks:**

- [x] Recheck current source, local changes, protocol and recipe allocation;
  record any material drift before expanding the write set.
- [x] Implement one generation transport/lifecycle seam without total/read/idle
  duration failure for current text and image generation routes; preserve
  independently bounded non-generation operations.
- [x] Implement cancellation/cleanup, unknown-outcome, handshake, process-
  identity, admission, strict-request, extensible-result, payload-bound,
  PNG/dimension, and public-projection decisions above.
- [x] Replace the native deadline fixture with owner cancellation or disconnect
  while retaining its cleanup-before-lease-release assertion. Put continued
  execution beyond the former 600-second boundary in the controlled-clock
  lifetime regression rather than a wall-clock wait.
- [x] Evolve the bundled sidecar and recipe coherently to protocol `3`; verify
  recipe/sidecar consistency separately from live client interoperability.
- [x] Add the controlled and real-route evidence for TIPC-01–TIPC-03,
  TIPC-05–TIPC-08, TIPC-11, and TIPC-12, including mixed protocol versions,
  replacement, and a representative non-image generation route.
- [x] Update the named contracts, ADR, README, module docs, and qualification
  instructions only after their behavior is implemented.
- [x] Hand the corrected source identity and passing owned evidence to the
  Tuldok image plan for candidate construction and TIPC-04/TIPC-09/TIPC-10.

**Gate:** TIPC-01–TIPC-03, TIPC-05–TIPC-08, TIPC-11, and TIPC-12 pass at their
declared fidelity; documentation matches implementation; no production
total/read/idle generation timeout remains; protocol-2 or malformed/replaced
sidecars cannot be admitted. The plan remains pending until TIPC-04, TIPC-09,
and TIPC-10 are returned by their owner.

### Supporting gates

- `cargo test --manifest-path rust/Cargo.toml -p pumas-app-manager`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-rpc --features inference-plugins`
- `python3 -m ruff check torch-server`
- `python3 -m ruff format --check torch-server`
- `python3 -m unittest discover -s torch-server/tests`
- In the resolved real Torch environment:
  `python3 -m unittest discover -s torch-server/tests_native -p test_image_failures.py`
- Existing documentation link validation and `git diff --check`.

The unit suite may use local fakes for contract behavior. The native failure
test proves real Torch worker/lease cleanup without claiming GPU execution;
TIPC-04 still requires the named real GPU evidence.

**Re-plan triggers:** Protocol `3` or recipe `0.1.6` is allocated before work;
the existing process/profile identity cannot close replacement races; safe
cleanup requires a new execution authority; or the bounded write set reaches an
unrelated public/runtime owner.

## External handoffs and sequencing

1. This reconciled authority precedes production implementation.
2. TIPC-M1 is one Pumas-generation/Torch-image unit, including its tests and
   documentation.
   Tuldok may independently remove its remaining client deadline against the
   unchanged numeric public contract.
3. Candidate construction and provider-level qualification require the
   corrected bundled implementation, not completed Tuldok UI work. Recipe
   allocation is rechecked immediately beforehand.
4. Combined acceptance requires revised Pumas, aligned Tuldok, and the exact
   installed candidate. Distribution may consume that evidence only under its
   existing separate authority.

The Tuldok companion owner must preserve its already-correct numeric
width/height request, 1280×720 defaults, dimensions, PNG display/save, explicit
consumer cancellation, watcher/socket cleanup, and no automatic retry. Its
remaining work is removal of the 630-second generation deadline and deadline
error, avoiding closed metadata examples, controlled unknown-outcome evidence,
and a real 1280×720 browser run on a supporting model. It does not redesign
discovery, datasets, or VLM transport.

GPU/browser absence blocks TIPC-04/TIPC-10, not TIPC-M1 source work. Candidate
construction waits for the corrected bundle. Publication is neither authorized
by nor a prerequisite of this correction. Shared files are integrated serially
between these plans.

## Reconciliation dispositions and exclusions

| Existing authority | Disposition |
| --- | --- |
| [Tuldok image plan](../torch-diffusion-serving/plan.md) | Retains Tuldok adaptation, corrected-candidate construction/installation, real GPU/browser evidence, and later distribution. Its prospective image deadlines are superseded by this plan. Historical image evidence retains its original scope. |
| [Desktop/release/Torch remediation](../current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/plan.md) | Retains text semantics, usage, responsiveness, and shutdown. Generation lifetime plus the image-specific compatibility/result subset of DRBT-A5 are delegated here; TIPC evidence is necessary but not sufficient for DRBT-A5. |
| [Rust/RPC remediation](../current-standards-remediation-2026-09-03/rust-library-and-rpc/plan.md), [frontend](../current-standards-remediation-2026-09-03/frontend-and-ui/plan.md), [governance](../current-standards-remediation-2026-09-03/governance-and-verification/plan.md), and [local intent API](../local-intent-api-2026-09-12/plan.md) | No ownership transfer and no dependency on completion; their unrelated work remains unchanged. |

Excluded: editable dependency profiles/model-details UI, text request/response
semantics and usage accounting beyond generation lifetime, generic load/model-
execution descriptions, Nunchaku/Klein migration, packaging redesign, durable
jobs/retry, unified gateway/discovery, unrelated text remediation, new platforms,
general desktop shutdown, publication/default selection, and deletion of old
versions. User-deferred broad GPU fault matrices, NVFP4 work, and T16 desktop
shutdown remain deferred unless a specific TIPC claim demonstrates a prerequisite.

## Composed-design review

**Composed-design applicability:** `applicable`

1. **Modules:** shared generation transport, gateway projection, TorchClient
   protocol/transport, sidecar work owner, and installer/qualifier retain
   distinct responsibilities.
2. **Dimensions:** generation duration policy is separate from connection/setup
   budgets; live protocol/capability/process identity is separate from immutable
   artifact/recipe identity and model ready-slot state.
3. **Composition:** one existing gateway seam owns generation transport while
   RPC and serving roots wire the same clients/providers; no registry, job
   service, or cache is added.
4. **Representative changes:** a protocol change touches private endpoints and
   qualification; a public-field change touches gateway/Tuldok, not inference;
   a recipe change touches artifact qualification, not live semantics.
5. **Interfaces:** the generation transport policy, public numeric image DTO,
   private versioned messages, process identity, ready-slot check, and
   VersionInstaller remain stable boundaries.
6. **Independent evolution, failure, replacement, and testing:** the shared
   generation transport can evolve or fail without absorbing provider worker
   ownership; each provider may be replaced only with its own cancellation and
   cleanup contract. Clocks/workers, controlled image and non-image transports,
   mixed sidecars, production installer, real GPU, and browser each decide only
   their declared claims.
7. **Deletion:** removing the obsolete deadlines and closed-result assumption
   reduces policy; no compensating abstraction is introduced.
8. **Complexity:** protocol `3`, process-bound revalidation, and retained cleanup
   custody are required to distinguish old live behavior and safe reuse.

## Blockers

- Pumas and Tuldok Passeur profiles and named registrations are configured with
  the user-confirmed subscription and pass Doctor/configuration checks. The
  Passeur/Muse client-name incompatibility is corrected in local Passeur commit
  `4f4ef0f`; its full tests, typecheck, build, direct Muse handshake, and a
  read-only MCP `listTools` handshake passed. Passeur source has since advanced
  independently to clean commit `49724a8`, while the ignored configured `dist/`
  runtime still has the older CLI surface and corrected client identifier; this
  drift must be reconciled before the next dispatch. The Codex host currently
  runs one exact configured process for each named server, and the Pumas process
  owns its repository lease, but this conversation exposes neither namespace's
  four task operations. A duplicate SDK/resource startup closes because the
  healthy attached owner already holds that lease. Recover the host-to-model
  tool attachment before retrying the recorded assignments. The current Codex
  configuration neither marks these servers `required` nor overrides the
  documented one-second optional-server catalog grace; setting the two servers
  required is the narrow next diagnostic, but needs global configuration
  authority and a fresh session. Do not kill the owner, start a competing
  coordinator, or substitute a custom MCP client that bypasses human approval
  elicitation.
- Required-real GPU and browser availability may delay TIPC-04/TIPC-10 only.
- Candidate build/install access may delay TIPC-09 only.

## Final acceptance

The plan becomes `Accepted` only when TIPC-01 through TIPC-12 are passed by their
named owners and the exact candidate/source identities are recorded. Qualification
does not authorize publication, activation as the production default, removal of
old installations, or completion of either broader plan.
