# Plan: runtime installations and registered model adapters

**Revision:** 4 — coordinated with the separate prerequisite acquisition plan; supersedes revision 3 at the same canonical runtime plan path.

**Status:** Deferred for the 2026-10-03 baseline/S3/network assignment. The adopted design is retained; runtime implementation has not started.

**Objective acceptance:** pending; real runtime, model, migration, native-platform, and desktop acceptance has not been performed.

**Current phase:** deferred behind baseline repairs, S3 and restricted networking under the [current acquisition priority](../artifact-acquisition/plan.md#requested-development-order-and-stopping-boundary). Revisit only after that assignment or a demonstrated prerequisite; AQ-HTTP remains required.

**Exactly one next runtime slice:** none while deferred. R1 remains the first candidate only after the deferral is explicitly lifted and AQ-HTTP is accepted and integrated for its target scope.

**Canonical repository path:** `docs/plans/runtime-installations-and-model-adapters/plan.md`.

**Integration owner:** Pumas runtime integration, sharing one serial integrator with the acquisition plan for overlapping contracts/files.
**Prerequisite owner:** [Artifact acquisition](../artifact-acquisition/plan.md); [gate status](../artifact-acquisition/reports/dependency-gates.md). R1 may begin only after its target-scoped AQ-HTTP gate is ready. Read-only preparation can proceed independently.

**Baselines:** Pumas `04e7f1568f00693c0ef26c77e0150e5e4dd112ea`; Coding-Standards `39d55dc330d44ecf940364ceada9d2527f7c7ea0`.

**Current accepted integration base:** Pumas `e37bbf4b964a0e2aadf25f80ab71edd8fa6b3eb3`. Acquisition Q1 is active on its task branch; AQ-HTTP remains not ready. Runtime source changes must start from updated accepted `main` after that gate is merged.

## 1. Objective

Reuse the generic artifact-acquisition direction in `docs/breif/s3-model-fetch.md` for obtaining verified bytes. Make app-manager the coherent application-facing manager of installed external runtimes/executables, with native llama.cpp and Python/Torch as concrete implementations. Allow an open population of model adapters to be registered independently of a Pumas build. A model may use a standard loader, that loader plus supplemental requirements, or approved custom loading/execution code.

The externally meaningful chain is:

**Selected model artifact + task → selected adapter revision → effective requirements and approved component/code inputs → compatible installed runtime → bound profile/process → adapter preflight → verified loaded-model receipt → supported operation.**

The manager may install and launch an executable without that fact asserting model support. An inference provider separately implements the execution protocol. Model adapters may share that provider and runtime; registering another model adapter does not create another runtime provider or require another global enum variant.

A normal adapter addition for an existing supported host/task must not require a new Pumas build, a new RPC/gateway branch, a hardcoded UI option, or a new installation-state variant. A genuinely new execution host or public task protocol can require a runtime integration; adapter registration is not a promise to interpret arbitrary protocols.

## 2. Scope and exclusions

### Included

- Consumer integration with the separately owned [acquisition contract](../../contracts/artifact-acquisition.md). This plan does not implement a second acquisition service, source protocol, transfer store or gate authority.
- Common runtime installation identity, inspection, launch-target selection, use custody, profile binding, and removal semantics for native executables and Python environments.
- Torch and llama.cpp exercised as real consumers of that common contract. Migrate affected Ollama entry points/representations in the shared family; preserve externally managed and in-process distinctions.
- Data-only model requirement specializations and versioned registered adapter implementation packages.
- Registration, discovery, explicit selection, authorized installation, lazy loading, revision replacement, disablement, unregister/removal, and failure isolation at their owning boundaries.
- Existing generic Torch text loading, Nunchaku, FLUX.2, and standalone Z-Image represented within the same adapter host contract, with only task operations that are actually implemented and qualified advertised.
- Dependency interpretation, exact model/component references, real slot UI/serving/gateway consumers, independent deployed packages, generated DTOs, and safe retained-state migration.

### Preserved constraints

Pumas remains a reusable model library with optional runtime conveniences. Pantograph orchestration, scheduling, and workload placement are outside this change. Core/library-only operation must not import or provision inference packages to inspect metadata.

Keep upstream Torch discovery independent of qualification pins, managed Python provisioning, explicit install/bind/select/default/start/stop, typed unsupported versus inconclusive results, numeric image dimensions, and duration-unbounded admitted generation. Existing startup/loading/probe/install budgets keep their separate owners and meanings.

Keep staged publication/recovery, immutable installation evidence, exact process generations, listener/child/device custody, and compensation. Reuse those mechanisms; do not add parallel mutable installation catalogs, process owners, or model-dependency authorities.

### Excluded

Full S3/Xet/peer/chunk-CAS implementation is not a prerequisite of this runtime plan; those mechanisms remain with the acquisition workstream. A plugin marketplace, automatic downloading/importing code from model cards, arbitrary shell install hooks, a universal package manager, a native Rust dynamic-library ABI, transparent hot replacement of live Python modules, multi-tenant isolation, remote scheduling, and universal support for every model or operation are excluded. Trusted external model-adapter packages and normal registration are **included**, unlike revision 1.

## 3. Domain decisions and ownership

### D1. Distinguish runtime installations, execution providers, adapters, and requirements

A **runtime installation** is an addressable installed unit. A native unit contains its selected executable/library artifacts and build identity; a Python unit includes the interpreter/ABI and resolved distribution closure. No native installation is forced to invent Python, wheel, or sidecar fields.

An **execution provider/runtime driver** knows executable roles, launch arguments, environment setup, supported protocol/task behavior, and readiness evidence. Provider-specific launch preparation belongs behind app-manager's management interface. Existing neutral OS/process custody may remain in core as a reused mechanism; there is exactly one operational owner of a child/session.

A **model adapter** turns approved model inputs and configuration into a usable model session for a supported host/task. Its definition may select an existing loader or reference independently supplied implementation code. Its implementation owns model-specific construction, preprocessing, execution invocation, result conversion, and release hooks as needed by its supported operations. It does not install packages, choose global defaults, own subprocess adoption, or define the public gateway protocol.

**Requirements** describe what an adapter/model needs. Installing them creates or verifies a suitable environment; it does not manufacture missing loader behavior. The same adapter may serve many models, and a model may have several possible adapters. Supplements alone do not require a new executable adapter.

### D2. Use one common installation/binding contract with specialized internals

Use a validated opaque `RuntimeInstallationId` rather than a Torch release tag as the common installed-unit address. Keep release/build/platform/interpreter/package identities as typed installation facts. Native and Python installation implementations retain release/build and dependency selection, install-time verification, installed-unit publication and migration. Source-independent byte transfer, resume, transfer persistence and content-integrity checks belong to the shared acquisition owner. Source resolvers retain source-specific location and authorization semantics.

A profile stores its requested installation binding when managed. A process receipt records the actual installation, executable/role, relevant host bundle identity, generation and effective device/configuration context. The native receipt must be grounded in the manager's actual launched executable and custody; llama.cpp is not required to echo a Pumas environment ID or speak a Python sidecar handshake.

Common operations are inspect/list, resolve installation choices, explicitly install, bind, prepare/launch, observe/stop, and remove an unreferenced installation. Use the existing version manager as a migration facade or refactor it in place; do not let old tag state and a new catalog independently mutate the same installations.

Global selection is a preference for future explicitly accepted bindings, not evidence about running processes. An explicit profile is evaluated against its actual binding. An unbound/ambiguous legacy profile reports binding-required. Removal operates on exact installation references and live use, not app-wide PID guesses.

Torch and llama.cpp are the immediate concrete locality test. General executable launch uses a resolved executable and argument vector, not a binary name derived from the app ID or a universal `serve` argument. Executable roles distinguish service launch from an auxiliary command when an actual integration needs that distinction; an executable does not inherently have an HTTP endpoint. This plan initially qualifies the existing native/Python service paths, rather than advertising arbitrary command or protocol support without an implementation. External service connections do not imply permission to install, adopt, or stop their processes. Embedded ONNX retains its in-process lifecycle.

### D3. Register an open population of adapters, without global eager imports

Introduce one logical model-adapter catalog with versioned definitions and explicit registration operations. It may combine bundled definitions and installed operator registrations, but those are origin/projection distinctions, not competing authorities. Reuse catalog/persistence utilities where their semantics fit; the current mixed UI plugin config is not automatically the canonical execution contract.

Each registration references a stable namespaced adapter ID, an exact revision/content identity, a supported adapter-host API, tasks and applicability facts, required component roles, supported option contract, and either a standard loader with declarative options or a pinned implementation package/entry point. Package metadata owns that package's dependency declaration; adapter metadata references or specializes it without creating an independently editable copy of the same requirements.

Read registration metadata without importing implementation code or optional inference dependencies. Keep a verified metadata reference available before environment provisioning so Pumas can explain/install missing dependencies. Verify that metadata against the selected package bytes again during install; discovery is not authorization or proof of readiness.

For Python implementation packages, prefer the standard distribution entry-point mechanism and public `importlib.metadata` APIs over a custom import/packaging framework. The exact entry-point group and package name are owned contract choices to establish at implementation, not assumed from this review. Explicitly authorize and bind the distribution/version/digest/entry point; do not scan ambient global Python and load every advertised entry point.

No fixed supported-adapter list or compile-time adapter enum controls registration. Bound individual payloads and resource usage and paginate enumeration; arbitrary extensibility is not unbounded memory allocation. Known unsupported host API/task/requirement variants have explicit diagnostics and are not silently interpreted as generic text loaders.

### D4. Define registration and revision lifetime

The catalog distinguishes definition validity/enabled state from installation compatibility, per-process availability, and per-model loaded state. A registered adapter with missing dependencies stays inspectable. A package's presence or import success does not publish a runnable model.

Same ID/revision with identical content can be an idempotent registration. Different content claiming the same immutable identity is a conflict. Different revisions may coexist. Choose revisions explicitly or through a documented compatible selection policy; never let filesystem order, hash-map insertion, or newest-version guessing select the winner.

Updates publish a new immutable definition/package reference. Loaded sessions pin their exact definition, package/host binding, installation and generation. A disable operation blocks new admissions; existing sessions retain their resources until their documented terminal lifecycle. Explicit stop/revoke is a different operator action. Unregister removes future selection authority; physical package/installation removal waits for retained references and use custody.

A catalog refresh is atomic at its owner and produces explicit diagnostics for invalid candidates. Preserve a last known snapshot only as an explicitly labeled observation; it cannot authorize new work when current authority is unknown. Do not silently omit broken entries or continue serving with stale revocation state. Prevent factory/client caches from retaining an obsolete registration as current execution authority.

Python process generations retain a stable package/module set. A new adapter package or incompatible dependency resolution uses a new/recreated environment and process, or a documented restart; no in-place `pip install` or hot module replacement in a live managed environment. Multiple compatible adapters can share one environment and process subject to the host's actual lifecycle contract.

### D5. Compose requirements at the existing owners

Effective requirements are **runtime-host requirements + selected adapter requirements + applicable authored model supplements**. Their resolved software artifacts are handed to shared acquisition; a model-specific `GetModel` request is not used to disguise a wheel or executable as a model. Apply platform/Python markers, native runtime constraints, source/hash constraints, and code/component inputs according to their owning semantics. Reuse actual package resolvers; do not implement Python package semantics in Rust.

A standard model may have no supplements. A model requiring an extra package can still use a standard loader. A custom construction/execution requirement uses an approved adapter implementation. Native llama.cpp requirements are build/features/device/artifact requirements, not a fabricated Python dependency environment.

The current dependency `env_id` is a profile/context identity, not an installed-runtime ID. Distinguish additive supplements from legacy full-environment profiles before composition. Preserve authored pins/provenance and unresolved states; unequal hashes alone do not establish additive conflict. New additive interpretation assigns torchvision/torchaudio or native-extension requirements to actual consumers, not automatically to every model modality.

Conflicting requirements return their sources and possible explicit installation actions. A successful resolution does not waive model/task compatibility. An explicit install may group constraints transactionally but cannot silently change user pins, registries, selected adapters, or unrelated running environments.

### D6. Apply trust to executable packages, not just filenames

Approval is an explicit operator decision or configured trust policy, not a Pumas-maintained whitelist of adapter IDs. A compatible new adapter can be authorized and registered without maintainer changes or a Pumas release.

Support trusted externally supplied adapter code without making model metadata permission to execute it. Registration and dependency inspection are read-only with respect to execution. Code installation/import requires the accepted operator or automation authority, exact source identity, integrity evidence, and supported host contract.

Replace unconditional `trust_remote_code=True` in the affected managed loading family with an explicit approved-code decision. A custom adapter can use approved pinned code through the supported host; an unknown model cannot silently enter a remote-code path. Native extensions and package build steps are executable trust decisions too. Preserve the supported binary-wheel baseline; source builds or setup scripts need their own explicit procedure rather than happening inside discovery.

Python environments and subprocesses isolate dependency/lifetime state, not hostile-code permissions. This plan makes no sandbox claim. Host API objects provide only the documented resources; approved plugins still execute with the deployed process permissions. Model assets and code are separate references with distinct authority and retention.

### D7. Unify admission and task execution without hiding unsupported tasks

The app-manager managed execution service composes library facts, selected adapter, requirements, installation, profile/process custody, host preflight, load correlation, and publication. Core supplies neutral contracts and existing model/profile/serving owners; core does not depend on app-manager. Preserve native embedding and local IPC by injecting/delegating through the optional integration, not by moving all functionality into RPC-only code.

Move model-specific selection out of RPC repository checks, Python ad-hoc dispatch, probe lists and gateway allowlists. The provider advertises the tasks/operations its host actually implements. The adapter describes its task support. Public availability is the intersection with current installation compatibility and a verified loaded session; arbitrary strings in a plugin cannot create an unsupported public endpoint.

Existing text code assumes a Transformers tokenizer/model and synchronous generation. Encapsulate that as the standard text implementation, not the universal custom-adapter contract. Before exposing it through managed public admission, qualify cancellation, executor responsiveness and shared generation lifetime. A new audio/video/task operation is explicitly unsupported until its real host/gateway contract is implemented; no claim that an adapter declaration alone adds one.

Load receipts correlate exact artifact/components, adapter revision and implementation package, runtime installation/fingerprint, host bundle, process generation, operation, effective device, slot/session and supported task result. Retain producer/consumer validation; an echoed identifier alone is not launch evidence. Listing, generation, unload, cancellation and compensation use the same binding and cannot target a successor at a reused endpoint.

### D8. Preserve independent compatibility and migration obligations

Runtime releases, installation manifests, model metadata, adapter definitions/packages, adapter-host API, private sidecar protocol, and public gateway contracts have independent change reasons and versions. Adding an adapter within an existing host API does not require a sidecar wire bump or Pumas release. Changing required binding fields does require an explicit private-protocol transition; protocol 3 is the current baseline, not a permanent hardcoded proposal for the successor.

Coordinate changed internal source/DTO/generated/UI consumers in their owning slice. Give public libraries, bindings and independently installed clients explicit supported-version or breaking-release dispositions. Inspect and migrate actual supported retained states without deleting user installations or moving virtual environments as though they were portable directories.

Older profile writers currently accept future schemas. New reader checks cannot fix those deployed binaries. Migration therefore requires retiring old writers or isolating the new authoritative storage and managed resource ownership. Keep backups and interruption/reopen evidence, but never overwrite newer authored state merely because a backup exists. Physical removal is a separately authorized operation.

### D9. Consume the prerequisite contract; keep downstream authority

[Artifact acquisition](../../contracts/artifact-acquisition.md) is the single proposed handoff authority. Its [plan](../artifact-acquisition/plan.md) owns HTTP/HF and S3 source handling, transfer/control/recovery, evidence-scoped ordinary-file handoff, and the prerequisite package-file integration. The runtime plan owns selection/trust, installed-unit publication, registered adapters and bound execution.

Acquire exact approved artifacts through that interface without fake model records or source-specific downloader code. Keep the input lease until extraction/package children and cleanup finish; commit installed state only after installer verification. Model assets remain owned by the library. An installed runtime already satisfying its binding can be inspected/launched offline without querying remote sources merely to re-establish file acquisition.

Runtime R1 requires AQ-HTTP for the integrated target. R2 requires R1 and AQ-PACKAGES; it verifies the new registered-adapter consumer even though acquisition Q2 already proved the existing package consumer. Any S3 path additionally requires AQ-S3 for its endpoint/target scope. Native Xet, peers and chunk CAS remain acquisition continuation work, not hidden runtime prerequisites.

The existing HF/native/package bridges used by Q1/Q2 are prerequisite evidence and do not depend on R1/R2. Once the acquisition integrator accepts those shared files, runtime may refactor their consumer under the same contract. Exact status, scope, evidence and invalidation are owned in [dependency gates](../artifact-acquisition/reports/dependency-gates.md), not copied here.

## 4. Milestones, write sets and prerequisite gates

[Write sets](reports/write-sets.md) are part of these boundaries. Both plans use current exact source/consumer facts, preserve unrelated work and assign shared schemas, lockfiles, migrations and generated outputs to one writer.

| Slice | Coherent outcome | Dependencies / gate | State |
| --- | --- | --- | --- |
| **R1** | Shared installation identity, explicit profile binding and actual executable-bound launch for Torch and llama.cpp, including affected existing native consumers and UI addressing. | AQ-HTTP; runtime A01/A02/A09/A10/A12/A16/A17/A19/A20/A27 evidence at the relevant scope. | Planned; gated |
| **R2** | Register/list/select versioned adapter definitions; acquire through the accepted file-set contract, then install locally and lazily load an independent adapter. | R1 + AQ-PACKAGES; post-build registration, exact local package consumption and catalog lifecycle/trust evidence. S3 use additionally requires AQ-S3. | Planned |
| **R3** | One managed model-admission/task interface with existing native/Torch implementations migrated. | R2; requirements, exact receipts, slot/gateway/launch/unload consistency, real standard text/images and owned cancellation. | Planned |
| **R4** | Standalone Z-Image as a separately registered extension with no Nunchaku dependency. | R3; frozen Pumas host, exact real model and requested-size output, cancellation/reuse. | Planned |
| **R5** | Final installed/public/deployment qualification and removal of superseded execution authority. | R1–R4 implemented and each used acquisition gate ready for the promised scope; all runtime claims satisfied. | Planned |

R1 uses native and Python consumers immediately. R2 proves actual independently installed registration, not only DTOs. R4 is a genuine extension-locality test. Required producer/generated/UI changes land in the same slice as their owner, not in a final cleanup. Every gate is tied to its exact qualified scope; no all-source/all-platform acceptance is inferred.

Read-only investigation and design preparation may proceed before AQ-HTTP, but this plan does not start runtime implementation by building another downloader. Acquisition Q1 is the coordinated program's next implementation slice. Acquisition Q3 can proceed alongside later runtime work once writes are disjoint; runtime's HTTP/package integration does not wait for Xet/CAS or for unrelated acquisition release claims.

## 5. Acceptance and review

The [acceptance matrix](reports/acceptance-matrix.md) preserves A01–A28 and A30 as runtime-owned claims and records explicit acquisition/dependent-consumer dispositions for A29/A31–A34. Every implementation claim remains unsatisfied. The historical Python entry-point probe belongs to the earlier delivered package and is not production qualification; no probe is rerun or promoted here.

Composed-design review is **applicable**. All eight probes and authority scopes are answered in [architecture review](reports/architecture-review.md). Review the final artifact by its caller knowledge and change locality, not by module counts or green test totals. An adapter extension must not force edits across app-manager installation state, core provider enums, RPC, gateway and renderer.

Independent review covers migration/old writers, executable trust, session/cancellation/removal lifecycle, adapter revision/host API evolution, and final composition. Reviewers are read-only and reused for narrow repairs. Shared contracts, generated artifacts, manifests, lockfiles and plan state have one integration writer. Delegate implementation only with disjoint primary writes and explicit adjacent/shared-file rules.

## 6. Bounded preparation, blockers and re-plan triggers

Before R1, read the acquisition gate record and confirm AQ-HTTP evidence applies to the actual target/candidate. Refresh local branch/dirty state, shared active plans, public launch consumers, native artifact layout/integrity, supported legacy state and old-writer retirement/isolation. The acquisition contract owns transfer custody; runtime must not reimplement it. Missing migration/deployment facts block their unsafe effects, not bounded read-only preparation.

Before R2, establish the exact package/metadata entry-point convention and host API, supported declaration/options validation vocabulary and namespace ownership. Resolve third-party packages from a reviewed source; no guessed version pins appear in this plan. Before R4, verify the selected standard Z-Image dependency/API/model-layout tuple through the smallest real observation that can change the implementation.

Re-plan for a new trust boundary or task host, incompatible independent deployment, migration data risk, duplicated authority, unbounded/ownerless work, or propagation of adapter semantics back into shared runtime/UI code. Adding another ordinary adapter or another file within an existing owner is not itself a re-plan trigger. A new native dynamic-plugin ABI or universal install language would require a separate justified decision.

Acceptance transitions through Implemented/Verifying/Accepted only as named evidence permits. Record unavailable native/GPU/GUI/deployment legs accurately. Source review is not certification that the current or future entire codebase is standards compliant.

## 7. Adoption, authorization and links

Adopt this revision at `docs/plans/runtime-installations-and-model-adapters/plan.md` alongside `docs/plans/artifact-acquisition/plan.md`. Acquisition Q1 replaces former runtime S1a; R1 replaces S1b; R2–R5 replace S2–S5. The [cross-plan record](../artifact-acquisition/reports/dependency-gates.md) owns that disposition and readiness. Do not adopt the old Torch-only plan as another active authority.

Reconcile active upstream-Torch, cross-platform, diffusion and standards-remediation owners by superseding only affected current decisions. Preserve their historic exact-tuple evidence and unsupported/unaccepted limits. Do not delete or rewrite old plans merely to erase history; update current owner documents when their behavior lands.

A future implementation session explicitly selects this plan and `start` after its gate is ready. This planning delivery does not mutate the repository, authorize live environment migration/deletion, publish packages/releases, or rewrite shared history. Keep coherent commits and independent read-only review; no fixed commit topology is prescribed.

[Ledger](execution-ledger.md) · [Issues](issues.md) · [Source audit](reports/codebase-audit.md) · [Adapter contract](reports/adapter-contract.md) · [Architecture](reports/architecture-review.md) · [Acceptance](reports/acceptance-matrix.md) · [Write sets](reports/write-sets.md) · [Standards/source inventory](../artifact-acquisition/reports/standards-and-sources.md) · [Shared acquisition contract](../../contracts/artifact-acquisition.md).
