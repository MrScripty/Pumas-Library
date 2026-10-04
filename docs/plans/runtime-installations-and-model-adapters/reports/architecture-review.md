# Composed-design review: runtime management as an acquisition consumer

**Applicable.** This replaces the revision-three composition review for the current runtime plan. It is a source-grounded design review, not external independent review or implementation acceptance. The companion [acquisition review](../../artifact-acquisition/reports/architecture-review.md) owns the shared lower-layer design.

## Authority scope

Core owns model facts, authored supplements, exact model/component references, profile state, stable binding values and existing custody/publication invariants. Acquisition owns selected bytes and their handoff. App-manager owns installed external runtime configuration, native/Python build selection, installation and its coherent lifecycle interface. Runtime integration owns supported protocol/operation mechanics. Model adapters own model-specific construction/execution and requirements; their catalog owns immutable registration/revision authority. Public transports and UI project those decisions. No one manifest/catalog absorbs all these authorities.

## 1. Independent concerns and dimensions

Acquisition obtains selected files for an authorized consumer through an owned transfer into approved storage. Installation consumes them using native/package semantics, validates and publishes an installation. A profile selects that installed unit; a process receipt records the actual launched unit/executable/generation. Registered adapter code constructs and executes the selected model through a supported host/task. These concerns have independent who/what/how/when/where/why dimensions: their selection authority, implementation, change cadence and terminal results differ even when one user action composes them.

## 2. Necessary and accidental interleaving

A model load necessarily binds model/component revisions, adapter definition/package, installation content, host code/protocol, profile/device settings, process generation and operation. Its required acquisition demand is retained until installation consumes the files, but network source location is not runtime identity.

Remove tag-as-installation, global-preference-as-process-identity, dependency-profile-hash-as-installed-unit, adapter-as-provider, literal adapter menus, repository-ID loading branches, imported code during discovery, and one success boolean covering acquired/installed/loaded. Keep material lifecycle obligations visible instead of hiding them in a service name.

## 3. Caller and composition knowledge

A caller supplies model/task/profile or an explicit install/registration action, receives a typed result and controls its documented lifetime. It should not derive runtime paths, choose source URLs, implement pip/HTTP recovery, infer adapter IDs from repo names, inspect PID text as ownership or manually reconstruct publication safety.

The existing optional composition builds app-manager from core/acquisition/contracts and private clients, not the reverse. No new daemon/global runtime/process registry is introduced. The generic library and metadata inspection remain usable without inference provisioning.

## 4. Change locality

| Representative change | Owners that change | Owners that should remain generic |
| --- | --- | --- |
| Add an ordinary model adapter | Adapter package/definition, registration and real qualification | Acquisition, provider enum, installer state, RPC/gateway model branches, hardcoded UI options |
| Add a runtime build | Runtime/native/package selection and probes | Acquisition lifecycle or model taxonomy |
| Add S3 retrieval | Acquisition/source integration | Adapter loader, runtime installed-ID meaning |
| Change model supplements | Core authored dependency interpretation and affected admission | Generic source transport or global modality package list |
| Change host/task protocol | Host/protocol authority and actual producer/consumers | Unrelated source reader or acquisition store |
| Change input retention/recovery | Acquisition owner and affected installer consumption tests | A second runtime download engine |
| Change device/profile settings | Profile/driver/process binding | Immutable remote content identity |

## 5. Stable values and leaked implementation detail

Use RuntimeInstallationId, exact model/artifact references, adapter revision and constructed launch/load receipts. Consume the shared file lease rather than reqwest/S3/pip transport types. Native installations have no fabricated Python fields or forced Pumas-sidecar handshake; actual launched executable custody supplies native identity. Keep approved-code decisions explicit without creating a universal install-script language.

## 6. Independent evolution and failure

A registered adapter may be missing dependencies without disappearing from the catalog. Acquisition can succeed while installation fails. A new package revision does not hot-replace a running module set. Changing a global default does not rewrite bound profiles. Unsupported tasks remain unsupported despite metadata claims. Process replacement/cleanup cannot act on a successor. Installed runtime operation does not require a new remote download when its binding is already satisfied.

Independent package, host API, source schema, persisted profile and public gateway contracts receive their own compatibility decisions. Their common repository or release does not make them one versioned artifact.

## 7. Deletion test

| Mechanism | Deletion result | Decision |
| --- | --- | --- |
| Common installed-unit identity/binding | Same-release variants and actual launched identity become ambiguous | Retain |
| Deep managed execution service | Every UI/RPC/gateway caller rebuilds adapter/requirements/process admission | Retain within app-manager |
| Open registered adapter definitions and packages | Every new model implementation requires a Pumas build/list change | Required by user scope |
| Existing process/device custody | Cancellation and replacement safety disappear | Extend, never duplicate |
| Shared acquisition consumer interface | Runtime-specific download code returns | Consume prerequisite, not a new runtime module owner |
| Generic orchestration engine, marketplace, dylib ABI | Requested current outcomes still work | Exclude |
| Duplicate catalogs/compatibility aliases without a supported consumer | No accepted capability lost | Remove at the owning cutover |

## 8. Cumulative complexity and plan composition

Retain the necessary installation, adapter, dependency, execution and custody responsibilities but remove repeated interpretation. Acquisition Q1/Q2 supplies a proven handoff first using old consumers; runtime R1/R2 supplies richer new consumers afterward. The dependency is one-way and stage-specific. Neither plan owns the other's acceptance state.

Review the actual implemented representative changes, not file count or test totals. Re-plan if ordinary adapters still force edits across transport, GUI and global provider code, or if a second mutable installation/download/process owner is introduced. Final code review includes executable trust, retained-state migration, lifecycle and registered-package extension against a frozen host.
