# Execution Ledger: Local Intent API

## 2026-09-12 — Plan creation

- Created a local-only implementation plan from the intent/discovery/distribution
  brief and the user's release scope.
- Surveyed native API operations, model-reference/package-facts types, download
  lifecycle entrypoints, same-device client, RPC error projection, and current
  architecture/planning guidance.
- Selected four dependency-ordered milestones: native local resolution, managed
  acquisition, durable local desired state, and existing local transports.
- Recorded separate intent naming because existing `get_model` is indexed lookup.
- Deferred nodes, fleets, new networking, MCP, shared discovery caches, and
  inference gateway development.
- Identified existing recovery/import work as a prerequisite to evaluate before
  acquisition and durable-state integration. No claim that it is complete.
- Implementation and acceptance evidence remain pending. No runtime code changed.
- Documentation verification: relative Markdown links resolve; `git diff --check`
  passes for this documentation change. Runtime suites were not run for planning.

## 2026-09-12 — Implementation admission

- User explicitly requested implementation of this plan, with Sol medium for
  implementation and Astra medium for design and error fixing. Started M1 and
  transitioned the previously Planned plan to Active.
- Delegated the native local implementation to Sol and an independent read-only
  acquisition prerequisite assessment to Astra. Parent owns integration, review,
  documentation, and acceptance.
- Existing unrelated workflow/stub deletions and temporary recovery directories
  are outside the write set and remain untouched.

## 2026-09-12 — Source findings and baseline verification

- Astra identified hardcoded main revision in metadata, downloader, identity,
  bundle preflight, persistence and recovery. Added M2a with a concrete local
  prerequisite design; this does not broaden network exposure or complete M2.
  Evidence: [acquisition audit](reports/acquisition-prerequisites.md).
- Initial unchanged-code `api_tests`: 11 passed, 25 failed. Root failure was the
  existing llama router test expecting `llama-server` in an error now emitted as
  `Runtime spawn failed: No such file or directory`; subsequent failures included
  a poisoned registry test mutex. Astra independently reproduced the root test.
- Rerun excluding three known launcher diagnostics: 27 passed, 6 failed, 3
  filtered. Existing runtime shutdown expectation `profiles_processed >= 1`
  failed and poisoned later registry tests. No runtime/test behavior changed to
  hide these failures; relevant model/library regressions will be run separately.
- Expanded M1 write scope narrowly to existing library/context owners so the
  intent module can reuse canonical source-fingerprint validation without
  reconciliation or cache writes. Parent rejected a provisional timestamp/size
  approximation; Astra owns the canonical read-only helper and Sol integrates it.

## 2026-09-12 — M1 accepted: native local intent resolution

### Behavior and ownership

- Sol implemented `PumasApi::intent()` and shared local query/get/status logic,
  transport-independent requirements/outcomes, safe candidate/handle projection,
  native example, and public-interface tests.
- Astra added the canonical read-only fingerprint helper by sharing descriptor
  construction below existing external-asset reconciliation. Public operational
  methods retain their behavior. No new downloader, persistence schema, transport
  method, listener, runtime loader, node, fleet, or MCP operation was introduced.
- Parent and Astra review closed inconsistent summary/detail identity, revision
  and repository evidence, stale negative decisions, unknown-format evidence,
  invalid GGUF inspection, and filename-only quantization inference. Positive and
  negative decisions now require coherent current source evidence.
- Read-only matching detects changes across snapshot pages/evaluation and returns
  unavailable observation; it does not retry mutations or regenerate facts.
- `AllowUpstream` remains explicitly unsupported; ensure/release are not exposed.
- Public Rustdoc/core README state that a handle is an availability observation,
  not a lease, tensor-integrity proof, or runtime readiness promise.

### Verification and limits

- `cargo test --offline --manifest-path rust/Cargo.toml -p pumas-library --test intent_api_tests`: **9 passed** after final review fixes. Covers exact local
  matching/no writes, missing/invalid/unsupported states, ambiguity, constraints,
  unknown revision/format evidence, stale/removed files, malformed cache identity,
  contradictory cached revisions, known-invalid GGUF and header vs filename quant
  evidence. Real filesystem/index/facts producers; HF/process clients disabled.
- `cargo test --offline --manifest-path rust/Cargo.toml -p pumas-library --lib cached_package_facts`: **2 passed** after source-coherence fixes. Includes
  same-length file/metadata changes, current revision/repository/identity checks,
  and observation without metadata/cache/feed writes.
- `cargo test --offline --manifest-path rust/Cargo.toml -p pumas-library --test api_tests -- --skip runtime --skip launch_llama_cpp`: **30 passed, 6 filtered**.
  Six unrelated runtime-profile cases excluded after the documented unchanged-code
  baseline failures; this does not claim the full suite passes.
- `cargo test --offline --manifest-path rust/Cargo.toml -p pumas-library --lib package_facts`: **28 passed** before final helper-only coherence additions;
  the final helper subset was rerun afterward.
- `cargo clippy --offline --manifest-path rust/Cargo.toml -p pumas-library --lib --test intent_api_tests --example intent_model -- -D warnings`: **passed**
  after final fixes. Native example also passed Cargo check.
- Exact changed Rust files formatted; documentation relative links and
  `git diff --check` pass. Evidence is Linux/native and controlled local artifacts;
  no external HF, remote transport, persistence-migration, or all-platform claim.
- Existing HF directory/file target mismatch (I8) returns `Incomplete`. No
  canonical package-fact/load-target behavior was changed to hide it.
- Concurrent unrelated runtime-profile/mmproj changes appeared during this work;
  they were preserved and are not part of M1 or its acceptance claim.

M1 and A1 are accepted within these limits. The plan remains Active with partial
acceptance. **Exactly one next slice: M2a**, the audited immutable-revision
acquisition prerequisite. M2–M4 and their acceptance claims remain pending. No
commit was created.

## 2026-09-12 — M2a admission and compatibility decision

- Continued on the user's instruction. Sol owns acquisition, persistence, and
  importer implementation; Astra owns revision identity design and error review.
- Preserve public `DownloadRequest`, notification payloads, and binding inputs.
  Add an optional revision field to public `PersistedDownload`: external Rust
  struct literals must supply this field. This is a source-breaking change to
  that lower-level record, despite preserving the operational request contract.
- The existing locked atomic JSON store remains authoritative. Support validated
  v4-to-v5 upgrade, mapping legacy records to main; v5 writes explicit revision
  evidence. Older v4 readers must refuse v5. Failed validation must preserve the
  original document and all custody state. No downgrade rewrite is supported.
- Ticket recovery cannot carry an immutable pin today. Refuse non-main ticket
  recovery and protect pinned destinations from generic legacy recovery paths;
  ordinary persisted resume must retain the exact commit. Full recovery lifecycle
  acceptance remains deferred to M2's existing gate.
- Constructor inventory also includes `src/tests.rs`; admit mechanical fixture
  initialization there, without changing runtime behavior or unrelated tests.

## 2026-09-12 — M2a accepted: immutable-revision acquisition foundation

- Sol implemented private revision threading through managed preflight, metadata
  and cache isolation, bundles, worker requests/retries, admissions, persisted
  resume, and owned normal/Diffusers import. Astra implemented validated identity
  and bound provenance reads, corrected identity stability across implicit file
  expansion, reviewed migration outcomes, and fixed diagnosed fixture failures.
- Public operational requests continue with main. Pinned identity and destination
  include the complete commit for every selector kind; discovered files remain
  evidence without renaming an already planned pinned destination.
- Store v5 and validated locked v4 upgrade preserve custody. Malformed and proven
  pre-publication failures preserve original bytes; uncertain post-publication
  outcomes return failure without guessing rollback. The public persisted-record
  constructor compatibility change is documented in the core README.
- Provenance guards refuse conflicting metadata/markers before admission and
  recheck under destination ownership before execution/restored finalization.
  Ticket recovery remains unavailable for pins; native persisted resume retains
  the exact revision. No broader crash/recovery acceptance claim was added.
- Verification: **404 tests passed** across store (61), importer (24), recovery
  (30), HF (221), managed API (19), identity (10), intent integration (9), and
  existing relevant API regression (30) suites. Three unit-harness entries are
  subprocess helpers exercised by parent tests; six known runtime API cases were
  filtered for the previously recorded I7 baseline failures.
- HF loopback fixtures initially failed under socket restrictions; authorized
  outside-sandbox reruns passed. Six old fixtures were updated to preserve their
  lifecycle fault-injection claims while supplying valid preflight provenance.
  The strict production guard was retained.
- Clippy passed with warnings denied for the library, tests, and native example.
  RPC and UniFFI consumers passed offline Cargo check. Rust 2021 formatting,
  documentation links, and diff whitespace checks passed.
- Detailed commands, migration outcome semantics, moving-main and retry/reopen
  evidence, and limits are in the [M2a report](reports/immutable-revision-implementation.md).
- Existing workflow/stub deletions, recovery scratch directories, and concurrent
  runtime-profile/mmproj edits remain outside this work and were preserved.

M1 and M2a are accepted. The plan remains Active with partial acceptance.
**Exactly one next slice: M2**, beginning with its lifecycle prerequisite gate
before connecting managed acquisition to the public intent language.
`AllowUpstream`, ensure/release, and transport projections remain unimplemented;
nodes, fleets, and new networking remain deferred. No commit was created.

## 2026-09-12 — M2 admission and verification prerequisite

- User requested continuation. Started M2 with Astra reviewing lifecycle and
  integrity prerequisites and Sol implementing revision resolution/orchestration.
- Audit found pinned progress identity still used legacy identity projection;
  Astra corrected it. More substantially, existing-file skips and recorded hashes
  do not prove acquired byte integrity. Add verification under the existing task
  and destination owners before allowing pinned completion/import.
- Admit only bound verification helpers in download_recovery.rs and canonical
  facts production in importer.rs beyond the existing M2 write set. Immutable
  tree rehydration supplies per-file evidence on resume without a new store
  schema. Unknown auxiliary integrity remains unknown.
- Preserve unresolved custody as blocked/unavailable; public status uses a
  read-only snapshot and pinned local requirement, not a second downloader or
  ephemeral-only operation identifier. Broad crash recovery remains unaccepted.

## 2026-09-12 — M2 accepted: managed native intent acquisition

- Sol implemented native immutable upstream selection, actionable ambiguity,
  stable pending requirements, and read-only progress projection over the
  existing managed download owner. Public operational download requests retain
  legacy behavior; no transport, desired-state store, or network listener added.
- Pinned execution verifies exact admitted file evidence before promotion and
  import, including existing finals and complete partials. Cold execution
  rehydrates file sizes/hashes and rejects contradiction with durable primary
  identity. Bound filesystem verification preserves failed bytes.
- Pinned import must publish canonical package facts before reporting completion.
  Parent added failure-injection coverage for canonical-facts publication and a
  native acquisition example that always awaits managed download shutdown.
- Astra review and fixes closed selector-ID collisions, completed explicit-branch
  lookup after live state cleanup, explicit commit substitution, mismatched
  active artifact constraints, and handled upstream/integrity failures being
  incorrectly counted as failed mutation effects. Actual cache/IO and panic
  failures retain ownership. Cold fixtures explicitly observe settled tasks and
  order cache removal before execution evidence lookup.
- Final verification: **433 tests passed** across the managed HF, native intent,
  API lifecycle, importer, recovery, artifact identity, store, and relevant
  operational integration suites. Three subprocess helpers ignored; six unrelated
  runtime API cases excluded for the recorded baseline reasons. No full-runtime
  suite or all-platform claim. All core tests/examples pass Clippy with warnings
  denied; RPC without default features and UniFFI pass Cargo check.
- Real upstream gate passed for the 1,185,376-byte public
  `ggml-org/test-model-stories260K` artifact at immutable commit
  `479896ec924af6d40fd419ab8f4d1eb2101de00d`. Independent local size/SHA-256 match;
  both canonical facts records present. Isolated temporary library and registry;
  no runtime execution. Exact evidence and commands are in the
  [M2 report](reports/managed-intent-acquisition.md).
- I2 broader crash/recovery evidence and I8 canonical directory load-target
  correction remain owned separately. M2 rejects known unsupported acquisition
  layouts instead of claiming availability for them.

M2, A2, and A3 are accepted within these bounds. The plan remains Active with
partial overall acceptance. **Exactly one next slice: M3**, beginning with its
store/lifecycle design gate for durable local ensure/release. M4 existing local
transport projections remain planned; nodes, fleets, and new networking remain
deferred. Unrelated dirty files were preserved. No commit was created.

## 2026-09-12 — M3 admission and persistence prerequisite re-plan

- User requested continuation. Astra independently inspected the index/store and
  existing reconciliation lifecycle while parent audited deletion entrypoints.
- ModelIndex uses WAL synchronous NORMAL and unversioned additive schema setup;
  runtime task shutdown aborts without draining; ModelLibrary deletion removes
  the index row before asynchronous filesystem effects. Public ensure cannot
  inherit durable acknowledgement or exclusion guarantees from those behaviors.
- Admitted M3a: the crate-private authoritative declaration/deletion-claim store
  and process-crash transaction evidence. Public ensure/release and reconciliation
  remain M3's gated next integration, with no additional scheduler or owner here.
- Exact proposed store and lifecycle designs are recorded before implementation.
  This re-plan does not accept M3 or its broader crash/recovery claims.

- Reviewed and admitted the exact M3a store design before Sol implementation:
  existing index connection, component schema v1, FULL writer synchronization,
  atomic additive migration, normalized requirement/consumer-derived ID, typed
  immutable target, and UUID creation generations. Binding preserves that UUID
  for idempotent retry; release/re-ensure allocates a new one to prevent ABA.
  Sol owns the store and a second Sol owns on-disk/process tests; Astra reviews.

## 2026-09-12 — M3a accepted: authoritative declaration storage

- Sol implemented the private component-versioned store on the existing index
  connection; a second Sol implemented on-disk and subprocess tests. Parent and
  Astra reviewed schema admission, canonical identity, transaction exclusion,
  migration preservation, and the boundary with future lifecycle integration.
- Immediate transactions own declaration creation/binding/release and deletion
  claims. Writer connections use FULL synchronization. Snapshot reads reject
  partial/future schema, malformed/cross-inconsistent rows, and contradictory
  retention/deletion custody. Catalog clearing does not cascade into authority.
- Review fixed namespace/index discovery, first-installer transaction ordering,
  read snapshot coherence, canonical JSON/UUID/quantization handling, local target
  loss, and retention/claim overlap. A shared production commit hook makes the
  before-COMMIT process test exercise the actual INSERT/readback boundary.
- Final tests: **378 passed**: index 74 (including 11 new storage tests), intent 7,
  managed HF 233, API HF 25, public intent 9, and relevant API 30. One ignored
  process helper is invoked by its parent test; six runtime cases remain excluded
  for previously documented unrelated baseline failures. No full runtime-suite,
  power-loss, or all-platform claim.
- All core library/tests/examples pass Clippy with warnings denied. RPC without
  default features and UniFFI pass Cargo check. Exact Rustfmt edition 2021,
  documentation relative links, and whitespace checks pass.
- The [M3a report](reports/m3a-declaration-store.md) records scope and evidence.
  Historical binaries ignore the new tables; downgrade with live declarations
  or claims remains unsupported. No public ensure/release method, scheduler,
  filesystem deletion guard, transport, or new networking was exposed.

M3a is accepted. Full M3 and A4 remain pending. **Exactly one next slice: M3b**,
owned ensure/release, immutable planning/admission, actual deletion/relocation
exclusion, startup/retry reconciliation, and their cross-component crash gates.
The lifecycle design and deletion inventory identify the required bounded scope
amendments before that implementation. Unrelated working-tree changes remain
untouched; no commit was created.


## 2026-09-12 — M3b integration admitted and active

The user requested continued implementation. Admitted the exact owner and
deletion composition reports and extended the write set for runtime ownership,
shared persistence injection, pure HF destination planning, lower library/merge
guards, and explicit shutdown. Sol implements finite ownership, HF planning,
and the public desired-state service; Astra handles reconciliation lifecycle
integration and deletion correctness. Existing native root grants and stores
remain the sole custody authorities. Unconfigured lower destructive operations
fail closed, including path-only source merge; this compatibility limit is
explicit. M3 acceptance remains pending cross-component verification.

Integration review identified two additional existing mutation paths: constructor
partial-file promotion and explicit planned migration. Admitted their narrow
correction: defer startup partial publication to the configured download owner,
and make migration share the same claims/capability operations. Unsafe split
transfer, cross-filesystem copy/delete, and partial duplicate transfer preserve
source state and report refusal. No additional task scheduler or store is added.


## 2026-09-12 — M3b accepted; M4 admitted

- Durable native ensure/release/status/list operations now compose under the
  existing primary. Commit precedes pure canonical planning; immutable binding
  and generation checks precede admission. Caller cancellation does not detach
  effects; shared shutdown drains local work and then downloads independently
  of its waiter.
- Existing native root grants, strict persisted custody, and deletion claims
  guard destructive paths, including discovered migration and constructor
  bypasses. Explicit compatibility limits are recorded in the M3b report.
- Real crash tests cover accepted declaration, upstream admission, and published
  artifact boundaries. Interrupted active custody resumes its exact existing
  writer; manual pause/error remains blocked. Same-pin repair after a branch
  change passes. The repair test found and fixed an existing noncanonical
  review-reason append in the partial-metadata importer.
- Relevant verification: 877 tests passed, six subprocess helpers exercised by
  parent tests; core Clippy -D warnings, RPC no-default-feature check, UniFFI
  check, formatting and whitespace checks passed. I7 runtime exclusions remain.
- M3 and A4 are accepted within the report's bounded Linux evidence. No commit
  was created; unrelated working-tree changes remain untouched.

M4 is now admitted. It extends seven prefixed operations through existing
authenticated IPC and the closed RPC command/outcome contract. The scope adds
PrimaryState dispatch and the existing RPC shutdown call to the transport write
set. Real process evidence will use the existing RPC binary and its IPC owner;
actual upstream pending/disconnect checks can use a child-only rate-limited
CONNECT proxy and the already-verified public pinned fixture, without TLS
interception, a new production endpoint, or any new listener. Offline local
process/contract checks remain independent. Nodes, fleets and new networking
features remain deferred.


## 2026-09-12 — M4 accepted; local intent plan complete

- Added all seven intent operations through the existing authenticated IPC and
  loopback RPC contracts. `PumasLocalClient::intent()` mirrors the native facade;
  RPC uses collision-free `intent_` methods and closed typed commands/outcomes.
  Strict receiver shapes retain native defaults; infrastructure errors use the
  existing public projection. No desktop or binding migration was required.
- Actual registry-backed child owners verify native/IPC Available parity,
  authentication, invalid/unsupported inputs, reconnect, forced restart and
  generation-safe durable release. Actual RPC binary consumers verify both
  transports and ordinary restart. Both real pinned HF acquisition gates pass,
  including the same pending writer after consumer disconnect, verified
  availability, retained bytes after release and successful graceful shutdown.
- The live gate exposed expected root contention being archived as an owner
  failure. Admitted a narrow reconciliation correction: opportunistic work stays
  dirty with bounded retry, explicit refresh retains its conflict, and neither
  poisons shutdown. Actual effect/join failures remain observable. Held-root
  regressions and both live process gates verify the correction.
- Updated two test producers: restart setup preserves normalized model evidence;
  tracked persisted download fixtures explicitly initialize revision. Production
  strict decoding and availability assertions remain intact.
- 267 relevant M4 integration/regression tests pass. Both RPC build variants,
  core and RPC Clippy with warnings denied, export-contract/UniFFI compatibility,
  exact formatting, whitespace and document links pass. The report records
  ignored helpers, older manual RPC cases and the unchanged timing-test limit.
- Existing operational native, UniFFI, RPC, Electron and frontend contracts were
  inventoried and retained. Public examples, core/RPC guides and architecture
  document the intent service, side effects, pending state, retention, upgrades
  and deferred capabilities.

All objective claims A1–A7 and milestones M1–M4 are accepted within their recorded
scope. The [M4 report](reports/m4-local-transports.md) holds final evidence. I7
runtime baseline exclusions, I8 directory-contract limits and broader historical
power-loss/lifecycle guarantees remain outside this acceptance. Nodes, fleets,
remote discovery and additional network features remain deferred until after
the next Pumas release. Unrelated working-tree changes were preserved; no commit
was created.

## 2026-09-14 — Commit preparation

Reviewed the remaining intent/storage/transport write set against the accepted
M1–M4 scope after separating Torch and conversion work. All nine local intent
API tests and five durable desired-state tests pass on the current combined
checkout; RPC with inference disabled also passes cargo check. The initial
sandboxed test attempt could not create its IPC fixture; the same tests passed
with local socket permission. Prior M4 process/acquisition evidence and its
explicit compatibility limits remain the acceptance record. No broad GPU
matrix or new live acquisition was repeated for commit preparation.
