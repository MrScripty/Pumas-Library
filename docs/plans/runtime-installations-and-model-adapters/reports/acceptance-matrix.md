# Runtime acceptance matrix — revision 4

All runtime-owned claims below remain **pending**. Acquisition gate evidence is tracked only in the companion plan; it does not replace the new runtime consumer observations. No production acceptance is executed in this delivery.

Owners below are roles to assign: runtime integrator (R), core/dependency owner (C), adapter/host owner (H), desktop/distribution owner (D), independent reviewer (V). Each row names a criterion, evidence kind, environment and execution mode. More specific methods below are part of the row.

| ID | Observable criterion | Evidence kind | Required environment / mode | Owner; slice | Status |
| --- | --- | --- | --- | --- | --- |
| A01 | Two different configurations of one Torch release coexist, reopen, bind and remove independently; failed/cancelled sibling installation preserves the other. | Integration + system | Representative managed host; automated | R; R1 | pending |
| A02 | A preferred installation never substitutes for the actual B-bound profile; include same-release variants and stale/racing selection. | Contract + integration + system | Controlled boundary plus real launch; automated | R; R1/R3 | pending |
| A03 | Missing packages, symbols, native requirements or device support reject selected adapter admission before model load; unrelated compatible adapters remain eligible. | Contract + integration | Controlled failures plus representative import environment; automated | H; R2/R3 | pending |
| A04 | No model supplements is valid; declared unresolved/invalid/inapplicable-context and failed queries do not become empty requirements. | Focused + contract | No material platform dependency; automated | C; R3 | pending |
| A05 | Compatible additive declarations compose; real conflicting pins/sources/hashes/Python/markers retain provenance; old complete-environment semantics are not silently weakened. | Contract + integration | Established package parser/resolver; automated | C; R3 | pending |
| A06 | Changed interpreter, distributions, adapter code or host bundle invalidates relevant evidence even with unchanged Torch version; hardware facts do not become package identity. | Integration + contract | Representative installation; automated | R/H; R1–R3 | pending |
| A07 | Loaded receipt correlates actual model/components, adapter revision/package, installation, bundle, process generation, operation, effective device and slot; mismatched ready responses cannot publish. | Contract + integration | Controlled host/native integration responses; automated | R/H; R3 | pending |
| A08 | Replacement/cancellation/disconnect/failed cleanup cannot publish or mutate a successor; required installation/adapter/device custody lasts through actual cleanup. | System + integration | Controlled real children; representative host plus simulated device work where scoped; automated | R/H; R3 | pending |
| A09 | Removal obeys live and persisted installation/adapter references; unrelated profiles continue; bind/start/remove races have one owning transition. | Integration + system | Representative filesystem/process host; automated | R; R1/R2 | pending |
| A10 | Supported legacy state imports idempotently or under verified one-shot conditions; interruptions reopen safely; authored data retained; old writers retired or isolated. | Contract + system | Representative retained-state replicas plus actual deployment facts; either | C/R; R1/R5 | pending |
| A11 | Binding-bearing private protocol accepts its actual supported host and rejects retained incompatible sidecars without fabricated defaults; independent adapter packages use compatible host API. | Contract + system | Installed host plus protocol fixtures; automated | H; R2/R3 | pending |
| A12 | Actual slot UI, serve, explicit launch/trial, listing, generation and unload use the same managed binding; direct/external control cannot mint owned readiness. | Integration + user-workflow | Representative desktop/backend; either | D/R; R1/R3 | pending |
| A13 | Standard Z-Image real load and requested-size generation work with no Nunchaku distribution or attempted import; cancellation and later reuse succeed. | System + user-workflow | Required-real supported model/assets/GPU and resolved environment; either | H; R4 | pending |
| A14 | Existing Nunchaku and FLUX.2 exact-tuple evidence is refreshed through the new contract; their own unsupported cases and cleanup remain correct. | System + user-workflow | Required-real qualified assets/hardware; either | H; R3/R5 | pending |
| A15 | Installed packaged Pumas outside checkout discovers and runs a separately installed adapter; actual UI bind/load/operation and resulting artifact are observed. | Release-artifact + user-workflow | Required-real packaged target and relevant model hardware; either | D/H; R4/R5 | pending |
| A16 | Changed DTOs regenerate from canonical source; real producer/preload/renderer preserve identity, typed failures and stale-invocation handling; no literal adapter menu list. | Contract + integration | Representative Rust/Node toolchain; automated | D; owning slice | pending |
| A17 | Library-only/no-default remains coherent without adapter imports/provisioning; changed native support is verified per supported OS without implying all-task support. | Integration + system | Required toolchains and required-real target OS for changed lifecycle; automated | R/D; R1/R5 | pending |
| A18 | Representative native build change, model supplement, independent adapter addition, host API change, revision update and UI change match the architecture locality matrix. | Architecture review | Source/installed change paths; manual | V; each material boundary/final | pending |
| A19 | Changed public/embedding/local-IPC/binding/independent clients have explicit version/cutover dispositions and matching evidence; public factory absence is not assumed from search. | Contract + system | Actual consumer/deployment facts; either | R/C; R1/R5 | pending |
| A20 | Native llama.cpp uses shared install/bind/observe/stop/remove contracts; R1 proves actual executable startup/stop, R3 proves model load and supported output. | System | Required-real native bundle/model and representative supported host; either | R; R1/R3 | pending |
| A21 | Freeze Pumas build, install/register an independently authored adapter package for an existing task, load and execute it without editing/rebuilding Pumas or adding core enum/UI cases. | System + release-artifact | Required-real built Pumas and separate package; either, with recorded workflow | H/D; R2 registration, R3 execution, R4 real adapter | pending |
| A22 | A standard model uses an existing adapter; extra requirements using the same behavior need no new implementation; genuinely custom behavior uses a registered implementation and does not fall back to generic loading. | Contract + system | Controlled models plus representative supported real path; automated | C/H; R3 | pending |
| A23 | Catalog discovery has no adapter imports or implicit installs; duplicate/conflicting/malformed records are visible; list pagination remains complete with a catalog larger than one page. | Contract + integration | Controlled package/catalog corpus; automated | R/H; R2 | pending |
| A24 | Exact registration is idempotent; conflicting same-ID/revision content fails; v1/v2 coexist as selected; disabling prevents new admissions while pinned sessions retain their version until explicit cleanup. | Contract + system | Controlled persistence/process revisions; automated | R/H; R2/R3 | pending |
| A25 | Unapproved model/plugin code cannot import/build/install through discovery or model load; approved immutable code takes the declared path; changed package bytes or wrong origin fails before use. | Security/contract + system | Controlled packages, sentinel side effects and real trust boundary; automated | H/R; R2/R3 | pending |
| A26 | A registered task implementation not shaped as a Transformers model/tokenizer executes its supported contract; unsupported task/host versions reject without creating gateway capability; text work does not block cancellation/control. | Contract + system | Controlled custom implementation plus actual host process; automated | H; R3 | pending |
| A27 | Native launch selects the correct executable role and library/build context, preserves argv boundaries, rejects invalid paths, and distinguishes owned/external/in-process operations; no Python placeholders or universal serve argument. | Contract + system | Representative native files/processes and supported OS; automated | R; R1 | pending |
| A28 | Compatible adapters share an installation; incompatible dependency closures use separate selected installations; adding/updating/removing one never hot-mutates shared live packages or loses references. | Integration + system | Real isolated environments and controlled conflicts; automated | R/H; R2/R3 | pending |
| A30 | Retained exact Torch/adapter artifact closure is acquired and installed from local inputs with network denied during installation. Reject wrong hash/build/wheel and direct-URL redownload; preserve original provenance and verify installed closure. | Contract + system | Real supported Python/package tool and approved wheel set; automated | R/H; R2 | pending |

## Required procedures and proof boundaries

### Native and Python management (A01/A02/A09/A20/A27)

Use an isolated launcher root. Through actual management APIs create or explicitly import verified native and Python installations, inspect their IDs and family-specific evidence, bind profiles, launch/observe/stop and reopen. Exercise distinct variants of a release where available; fixtures do not substitute for unavailable real variant evidence. A native profile receipt must point to the exact launched executable and build. It must not be satisfied by an arbitrary stub declaring its family. R1 may qualify native lifecycle before R3's model/task integration, but final acceptance needs both.

### Registered package after build (A21/A23/A24/A25/A26)

Build Pumas once and record the artifact identity. Prepare a separate versioned adapter distribution and metadata; keep it out of Pumas source/build inputs. Register metadata without imports, inspect needed dependencies, authorize exact package provisioning into a compatible new installation, select and bind, load through the real host and call a supported operation via the managed/public path. Record the package/definition/runtime/session identities. Change the adapter revision and verify new admissions versus existing sessions. Duplicate a registration identity and inject a broken import. Test unsupported host/task, unapproved code and changed bytes with side-effect sentinels. No `include_str!`, hardcoded enum, UI literal or source edit is allowed to make the independent adapter reachable.

R2 closes only actual registry/host discovery/probe obligations; R3 closes actual operation behavior. Standard Z-Image in R4 repeats the test with a real model, rather than treating a toy adapter as model qualification.

### Supplements and execution (A04/A05/A22/A28)

Use three cases: existing generic loader with no custom supplements; same loader with an actual additional requirement; a selected custom implementation with behavior not supplied by generic Transformers. Resolve with the selected package tool and preserve markers, sources, hashes and Python constraints. A historical complete-environment profile must retain its selected interpretation or be explicitly migrated. Name/dependency presence cannot invent runtime support. Test reuse of a compatible environment and explicit separation of incompatible ones.

### Standard and existing image models (A13/A14)

Record exact artifact/component refs, adapter package revision, resolved dependencies, interpreter/Torch/build, host identity, hardware and request. Standard Z-Image must run without Nunchaku installed or imported. Validate actual output bytes/dimensions, including 1280×720 when that declared adapter contract supports it; exercise cancellation and subsequent reuse. Nunchaku and FLUX.2 retain separately recorded qualified tuples. An import probe, successful load or simulated image result is not the generation/user-workflow claim.

### Migration, compatibility and desktop (A10/A11/A12/A15/A19)

Enumerate actual retained records and deployed consumers before migration. Test disposable replicas with interruption at authoritative transitions. Retire old writers or isolate new storage and managed resource ownership; do not rely on new-reader guards to constrain old code. Preserve authored selections or give an explicit binding-required result. Old sidecars/clients receive their supported compatibility outcome. Run installed desktop selection/registration/binding/loading outside the checkout; startup/jsdom/serialization alone does not prove the workflow.

### Cost and scope

Keep evidence at its owner and exercise negative cases at the boundary that can cause the forbidden effect. Use existing test frameworks and package tooling. No new general registry verifier, fingerprint service or property framework is justified merely to inflate evidence counts. Expensive hashing occurs at owned integrity transitions, not each generation. A frozen-build test supplies unique extension evidence; a mocked registry cannot replace it.

## Supporting checks

Use the repository's pinned toolchain and documented Rust/Node/Python commands in the [shared supporting commands](../../artifact-acquisition/reports/acceptance-matrix.md#planned-test-placement-and-supporting-commands). Select actual focused tests for each owning slice, then affected aggregate/static/feature/target checks. Confirm contract-export outputs and command from current source before running it. No Pumas command from that guide was executed in this review.

Record candidate, command/procedure, inputs, environment, result, limitations and evidence location for every executed claim. Required unavailable hardware, native OS, GUI or deployed-consumer evidence remains unsatisfied. Passing lint/typechecks is not full behavior acceptance, and test success is not itself evidence of architectural simplicity.


## Acquisition claims and retained runtime obligations

Shared-layer proof is owned by the [acquisition acceptance matrix](../../artifact-acquisition/reports/acceptance-matrix.md). Gate status is owned by its [dependency record](../../artifact-acquisition/reports/dependency-gates.md). The old claim IDs below are explicitly dispositioned rather than deleted without a trace.

| Prior runtime claim | Acquisition-owned proof | Runtime obligation that remains |
| --- | --- | --- |
| A29 shared HF/native transfer | AC03, AQ-HTTP | R1 consumes the accepted handoff; no fake model record or replacement downloader. |
| A31 acquisition/handoff crash and cancellation | AC04–AC06 | A08/A09/A10 exercise the new installer/adapter/process consumer and retain input custody through its cleanup. |
| A32 source identity, credentials and trust | AC01/AC02/AC07/AC13 | A25 enforces executable/adapter authorization; source support cannot grant code execution. |
| A33 input leases and immutable cache | AC05/AC06/AC10 | A08/A09/A28 exercise runtime/adapter consumers without live mutation or premature release. |
| A34 bootstrap/offline/recovery | AC04/AC09/AC12/AC15 | A17 plus R1/R2 integration prove installed runtime operation remains possible without unnecessary remote acquisition. |

A30 remains runtime-owned because registered adapter installation is a new consumer. AQ-PACKAGES proves the same contract first with the existing package installer, without waiting for that registry. Runtime gate evidence includes exact installed closure, private protocol and task semantics in addition to shared byte evidence.

For R1/R2 admission record the required acquisition gate's exact source/contract/target evidence. Re-run only affected shared claims when this consumer changes their material meaning. Do not require full S3/Xet/CAS completion for an HTTP/package operation, and do not advertise S3 consumption before AQ-S3 is ready.
