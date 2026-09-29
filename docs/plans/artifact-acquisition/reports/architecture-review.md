# Composed-design review: acquisition and downstream runtime management

**Applicability:** applicable. The design changes source/consumer boundaries, durable ownership, shared transport and runtime composition. This is a reviewed design proposal, not implemented acceptance or an independent external review.

## Authority-scope declaration

The shared contract contains stable values describing selected bytes and their use lifetime. It references source authorization, model selection, package closure and consumer operation identities without taking over their policy. It cannot become an umbrella manifest for model modality, CUDA selection, UI layout, package solving, installation lifecycle and transport scheduling.

Acquisition owns transfer/recovery and byte evidence. Source readers own protocol-specific access. Model and runtime consumers own publication. Their states and versions evolve separately; they are linked by exact request identity and explicit settlement. A common storage format or package distribution is not grounds to merge their authorities.

## 1. Independent concerns and their dimensions

| Concern | What / who | How / when | Where / why |
| --- | --- | --- | --- |
| Semantic selection | Model or runtime/package owner selects exact required inputs | Resolver/domain policy before acquisition | Existing core or app-manager integration; avoid implicit artifact substitution |
| Source description/access | Source integration identifies a pinned object and scoped access | Metadata, conditional reads, credential refresh during resolution/transfer | HF/HTTP/S3 boundary; source-specific protocol knowledge stays local |
| Acquisition | One owner transfers and verifies selected files | Governed async/stream/blocking work, durable attempt and cleanup | Neutral core module; eliminate per-consumer transfer implementations |
| Storage/use | Capability-backed workspace and explicit consumer demand | Stage, seal, read, transfer ownership or release | Model filesystem or artifact input storage; preserve ordinary paths and restart custody |
| Consumer publication | Model importer or runtime installer | Its existing validation and durable commit after bytes are ready | Its existing owner; byte completion cannot promise installed/usable state |
| Projection | RPC/UI and public/embedding callers | Decode, observe, control and reconcile snapshots | Delivery boundaries; no second source of successful state |

## 2. Required versus accidental interleaving

Required: exact file/revision, permitted source, destination custody, attempt generation, validation evidence and consumer retention must stay related throughout a transfer and handoff. These facts constrain the same operation.

Accidental coupling removed: model root UUID/repo ID in every transfer; HF importer inside generic completion; URLs as identities; image adapter names in source selection; package install code inside network transport; native runtime download loops; a single completed boolean spanning acquisition/import/install/run.

A durable demand has a different lifetime from a network request and from a runtime process. Time limits govern the specific operation, not the truth of cleanup or readiness.

## 3. Caller and composition-root knowledge

Callers choose exact content and a documented evidence policy, receive verified normal files, and settle custody after their own operation. They do not reconstruct HTTP ranges, retry loops, S3 signing, partial recovery or file-integrity checks. They still own the meaningful obligations of selection/trust and consumption; the abstraction cannot hide those by assuming execution permission.

The root supplies existing runtime/network configuration, approved storage capabilities, source integrations and shutdown ownership. Core remains independent of app-manager and inference imports. A direct neutral acquirer is constructible without a model database. Managed-Python downloads can therefore bootstrap without a cycle, while their supported package-provider protocol stays separately owned.

## 4. Representative changes and forced owners

| Change | Legitimate affected owner(s) | Should not change |
| --- | --- | --- |
| HTTP pause/resume correction | Acquisition HTTP reader/lifecycle and relevant cross-consumer tests | Separate HF/native algorithms or adapters |
| Add an S3-compatible endpoint | Source configuration and actual compatibility evidence | Installer state, model-adapter registry, per-cloud download code |
| Add a new protocol source | Source reader/resolver and composition registration; contract only for genuinely new semantics | Model/index schema or runtime build logic |
| Add a runtime build or adapter wheel | Domain selection/package metadata and installer/adapter evidence | HTTP/S3 lifecycle |
| Change model importer | Model publication and handoff/settlement tests | Source protocol implementation |
| Change acquisition storage format | Acquisition store, migration and affected public projections | Installed-runtime ID meaning or model selection |
| Add Xet or a chunk cache later | Acquisition reconstruction/retention implementation and named evidence | Ordinary-file consumer API, unless a newly required guarantee is explicitly adopted |

The locality condition is semantic, not a promise of one-file changes. Tests and packaging may legitimately span owners when their actual boundary changes.

## 5. Stable interfaces versus leaked detail

Keep validated artifact descriptions and file-use capability stable. Do not leak `reqwest::Response`, `object_store` types, pip report parsing, HF URL templates, raw auth strings, journal layouts or mutable cache paths into app-manager consumers. Source-scoped opaque version evidence can be retained without asking callers to interpret it.

Do not force all existing HTTP use into this owner: search metadata, health probes and generation are not artifact transfer merely because they use HTTP. Reuse a network stack without equating distinct lifetimes.

## 6. Independent verification, failure and replacement

Source failure can leave a resumable attempt without corrupting published models/installations. Consumer failure can leave a verified input available under retained demand, without making it runnable. A registry update cannot alter a selected acquisition. A read-only model-library consumer need not acquire execution infrastructure.

The source reader is replaceable behind conditional-read/evidence semantics. The storage implementation is replaceable only through its actual migration contract. Public model/install facades retain their completion meanings. Q1's two existing consumers establish this separation before runtime refactoring begins.

## 7. Deletion test and retained machinery

| Mechanism | What happens if removed | Admission decision |
| --- | --- | --- |
| Source-neutral acquisition owner | Byte lifecycle reappears separately in HF, native and package consumers | Necessary shared responsibility |
| Minimal artifact specification | Identity, verification and file-set completeness become unstructured caller assumptions | Necessary contract; no universal workflow manifest |
| Workspace/read custody | Cancellation/eviction/restart can invalidate files still in use | Necessary; extend existing capability/task mechanisms |
| Durable demand/attempt settlement | Crashes between transfer and import/install cannot be reconciled safely | Necessary state, owned once |
| Consumer publication owner | Acquired bytes get conflated with installed/imported success | Must remain separate |
| Package solver and S3 protocol dependency | Difficult standardized semantics need bespoke implementations | Reuse maintained libraries/tooling |
| Extra daemon, global CAS, VFS, coalescing scheduler | Q1–Q3 still deliver their requested outcomes | Not admitted now |
| Mirrored transfer journals or permanent per-source byte loops | No necessary capability is lost after the selected consumer cutover | Remove/disposition at each owning slice |

## 8. Cumulative complexity and scope

The intended retained structure is one deep acquisition service, a narrow source-access boundary with real HTTP/S3 implementations, an owned store/workspace implementation, and existing model/native/package consumers. Files can be split by lifecycle/authority; there is no prescribed layer count or class framework.

The shared contract closes the prerequisite without pulling runtime execution into acquisition. Q1 uses the existing native path; Q2 uses the existing package path. No new requirement forces runtime R1/R2 to exist before their prerequisites can be accepted. Store migration and credential/representation proof are inherent complexity; contain them rather than reduce them to unchecked booleans.

## Bounded library and mechanism decisions

**HTTP:** existing core uses reqwest/Tokio and hashing facilities. Retain those capabilities; isolate the specific file-read semantics and keep provenance/integrity/custody with acquisition. Review legacy helper calls before removing a public helper, and retain it only as a supported delegating facade when its existing promise fits.

**S3:** `object_store` is the first candidate, not an installed/qualified dependency claim. Its documented conditional/range/version operations and S3 endpoint/credential builder make it a plausible source adapter. Before Q3 expands around it, build the smallest reader against the pinned Cargo toolchain and observe: explicit endpoint/addressing, supplied/refreshable credentials, versions/If-Match, precise range/error propagation, cancellation, retry control and optional-feature isolation. Record the exact crate version, feature set, licenses, provenance and supported OS. Stop when those facts decide suitability. If a necessary guarantee cannot be represented, evaluate a direct maintained S3 SDK against the same requirement; no automatic fallback or in-house signer. References: [GetOptions](https://docs.rs/object_store/latest/object_store/struct.GetOptions.html), [S3 builder](https://docs.rs/object_store/latest/object_store/aws/struct.AmazonS3Builder.html).

**Persistence:** the current versioned JSON store already owns attempt/admission/release and uncertain publication. Generalize its proven mechanism and explicitly version records; use one ledger authority for migrated acquisitions. A new database/service is not selected merely because the output is generic. If real contention/transaction/volume requirements invalidate that mechanism, record the deciding evidence and re-plan at the persistence owner before changing the store.

**Package tooling:** use the current supported report/resolution mechanism and a real local-input install probe. Test a valid wheel set plus an alternate same-version wheel and a missing dependency with network denied. That determines the exact handoff before independent adapter registration depends on it; it does not require a universal lockfile or private pip API.
