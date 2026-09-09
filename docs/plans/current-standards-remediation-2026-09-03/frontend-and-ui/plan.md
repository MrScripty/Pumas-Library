# Plan: Frontend and UI Standards Remediation

**Plan status:** `Active`

**Current phase:** Complete the remaining M4 consumer migration. Milestones 0
through 3 and the selected catalog/search, ticket-recovery, display-cache, and
startup source checkpoint are accepted. Commit `2b081fba` includes generated
producer/preload/renderer conformance and Linux cold/warm GUI evidence;
`2b9553a0` preserves that UI after the backend collision correction. Neither
checkpoint closes all M4, XR-S1, or M5 claims. The approved approximately
one-second main-owned marker barrier remains the reveal authority.

**Next slice:** Quantization source-file discovery (FE-I26, remaining FE-I23).
Bound shared discovery consumers, reject extension-matching directories as model
files, and preserve inspection errors instead of reporting absence. Direct
importance-matrix options, exact targets and supplied-calibration preflight are accepted below;
content quality and immutable input custody remain outside that bounded check.
FE-I28 remains open on recurrence: accepted test-only admission
diagnostics below improve the next failing wait, without establishing its cause.
Do not repeat passing suites without a new deciding observation. Pending download
cleanup replay remains separately unadmitted.
Successful native setup now reconfigures and rebuilds current source below;
interrupted native recipe invalidation is accepted below. Later external mutation
provenance and independent probe/setup coordination remain outside acceptance.
Retained base-format readiness is accepted below.
Managed setup-versus-conversion exclusion is accepted below. Backend setup
observation/retry is accepted through standalone
Rust, RPC and the optional desktop bridge below. Native llama.cpp artifact verification/guarded repair
is accepted below; its former healthy-output skip is superseded by NBUILD below.
Installer custody, setup import verification/repair, direction-specific llama.cpp
artifacts and public quantization import-probe lifetime are accepted below.
Independent probe/direct/external-use exclusion and stronger containment remain
outside the accepted managed-execution guarantee.
Managed quantization target/calibration admission is accepted below; execution
readiness and lower-level backend preconditions are not closed by it.
NVFP4/Sherry script progress is accepted below, closing FE-I27.
Atomic manager-local admission, retained Rust-worker
observation and standalone/RPC shutdown composition are accepted below. Unique
staging and non-replacing publication are also accepted within the stable-parent
boundary below. Foreground cancellation, dual-pipe draining and direct-child
reaping are accepted below. Cooperating Linux group cleanup is accepted below;
escaped descendants remain outside that bounded contract. No new
quantization GUI mutation is admitted. Terminal authority and NVFP4/Sherry
script-progress projection are accepted below.
The optional conversion dialog now uses setup start/observation across uncertain
responses and reopen (FE-I25); see the accepted dialog ledger. Backend custody,
aggregate shutdown and core/RPC/desktop setup observation are accepted within
their documented evidence boundaries below.
The basic GGUF/safetensors format workflow is accepted within its documented
Linux and simulated-native-execution evidence boundary.
Conversion operation contracts (FE-I24), progress reads (FE-I14), import-picker
(FE-I11), and standalone-backend/link-health slices are accepted.
The user explicitly prioritizes API/UI contracts ahead of Pending download
cleanup replay; neither the remaining M4 work nor Pending replay is accepted.

**Acceptance status:** `partial`

## Direct Importance-Matrix Option Validation

Status: `Accepted`; see the
[ledger](execution-ledger.md#2026-09-08--direct-importance-matrix-option-validation).
Move managed `force_imatrix` backend applicability into the
shared pure option check: valid targets on NVFP4/Sherry with `force_imatrix=true`
return the existing `InvalidParams` before direct filesystem/probe/progress
effects. False remains accepted; llama.cpp retains its existing IQ/forced
calibration requirements and execution behavior. Target rejection keeps precedence.

Root owns core `src/conversion/{targets.rs -> options.rs,mod.rs,manager.rs,
llama_cpp.rs,nvfp4.rs,sherry.rs,types.rs}`, core README and four plan documents.
root_diagnostics owns `src/conversion/target_tests.rs`. Root serializes verification
and commits. No public type/schema/GUI/runtime/dependency changes. Direct option
regressions must fail before repair and pass after; existing managed checks and
valid llama.cpp and nonforced catalog paths remain accepted. Run minimal
conversion/full core-RPC tests, strict lint, formatting and plan checks; all passed.

Composed-design review: applicable. Extend the existing private check to
`validate_options(backend, target, force_imatrix)` rather than adding a separate
one-flag Module. It owns target/applicability ordering and errors; catalogs stay
with providers and calibration file/lifecycle policy stays with existing owners.
Four callers gain the same pure policy, with no retained state or interleaving.
Deleting it would restore duplicated policy. Re-plan for additional option
semantics or native calibration/execution requirements. Acceptance is local
automated fixtures, not real tools/models/hardware.

## Direct Quantization Target Validation

Status: `Accepted`; see the
[ledger](execution-ledger.md#2026-09-08--direct-quantization-target-validation).
The existing exact backend-qualified catalog match becomes
shared policy for managed admission and every built-in direct `quantize` entry,
before readiness probes, progress publication or staging. Direct checks also
precede filesystem inspection; managed source lookup remains unchanged. Unsupported
values return the existing managed `InvalidParams` messages. No normalization,
aliases, new targets or fallback to backend defaults. Catalog providers remain
authoritative; this does not certify installed-tool/hardware support.

Root owns core `src/conversion/{targets.rs,mod.rs,manager.rs,llama_cpp.rs,
nvfp4.rs,sherry.rs,types.rs}`, core README and four plan documents.
root_diagnostics owns new `src/conversion/target_tests.rs`. Root serializes
Cargo, formatting, integration and commits. No GUI/schema/dependency changes.
Focused actual direct-call rejections and existing managed catalog tests prove
the contract; valid catalog values must reach later source/environment checks.
Minimal conversion/full core-RPC tests, strict lint and formatting passed.
Evidence is local automated fixtures, not real native/model/GPU execution.

Composed-design review: applicable. Private `validate_target(backend, target)`
owns the matching/error policy and obtains identity/catalog from the backend;
callers only choose when to validate. Catalog changes remain with their providers,
policy changes stay in one Module, and no public trait method or runtime is added.
Deleting the helper would duplicate policy across four consumers. The pure check
has no interleaving or retained-state requirement. Existing force-imatrix,
calibration, source and execution policies are unchanged. Re-plan if external
backend implementors require a new public contract or target aliases.

## Calibration File Preflight

Status: `Accepted`; see the
[ledger](execution-ledger.md#2026-09-08--calibration-file-preflight).
Share supplied-file validation between managed quantization
admission and direct llama.cpp execution, before imports, staging or native
effects. Preserve existing required-path rules and managed error messages.
Every supplied path, including optional calibration, must identify a nonempty
regular file that can be opened and yield a byte. Recheck the opened handle's
metadata; do not read whole calibration datasets or infer text/content quality.
Missing/nonfile/empty inputs return `InvalidParams`; other inspection/open/read
errors retain contextual `Io`. Valid symlink paths keep existing semantics.
Callers must keep paths and contents stable/readable throughout preflight and
execution; this is not immutable custody or protection against concurrent path
replacement. No new native format/content constraint is invented.

Root owns core `src/conversion/{calibration.rs,mod.rs,manager.rs,llama_cpp.rs}`,
core README and four plan documents. root_diagnostics owns direct
tests in `src/conversion/llama_cpp/readiness_tests.rs`; root serializes Cargo,
formatting, review and commits. No GUI, schema, dependencies or live-model work.
Focused helper and direct-call tests plus existing manager admission tests prove
supplied-file rejection and valid input acceptance. Run core/RPC default tests,
minimal conversion tests, affected strict lint and formatting; all passed. Acceptance is
automated local-filesystem/simulated-tools evidence, not real-model quality.

Composed-design review: applicable. A private `validate_file(path)` Interface
owns file classification, bounded read and error policy; callers only decide
whether calibration is required and when preflight occurs. Manager admission
and direct execution use the same Module, with a fresh read at each invocation.
No receipt, cache, new runtime or retained state is added. Deleting it would
duplicate file/error policy in both callers; policy changes remain local while
worker lifetime and native execution stay with their existing owners. Re-plan
before immutable custody, content parsing or broader native execution changes.

## Download Admission Wait Diagnostics

Status: `Accepted`; see the
[ledger](execution-ledger.md#2026-09-08--download-admission-wait-diagnostics).
FE-I28 remains unresolved. This bounded slice enriches the
two existing one-second admitted-ID wait failures with the last observed
admission milestone and worker-thread completion. It does not infer the exact
pending effect or diagnose the original timeout from a last-observed milestone.

Write set: the two pause/admission tests and their private diagnostic helper in
core `src/model_library/hf/download.rs` (root_diagnostics); this plan, ledger,
issues and parent plan (root). Root serializes verification, integration and commits. No new
production hooks, public APIs, persisted state, runtime, deadline, concurrency
configuration, live-library or GUI changes. Existing fixture holds remain intact.

Acceptance: focused automated simulated evidence that diagnostic reads preserve
unobserved/observed milestones and worker completion without locks or awaits;
both existing lifecycle tests still pass. Run the original combined core/RPC
configuration once to exercise the enriched assertions in their failing-suite
context, plus affected strict core lint and formatting; all passed. A passing run accepts
only diagnostic fidelity, not FE-I28 stability. Composed-design review is
not-applicable: private assertion context reuses existing test-only observers,
without changing a production Interface or lifecycle. Re-plan if deciding
evidence needs new production hooks or a different ownership boundary.

## Native Setup Interruption Invalidation

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--native-setup-interruption-invalidation).
NINVALID prevents a failed or interrupted llama.cpp recipe from
leaving usable-looking tools eligible for subsequent reads/execution. A private
native setup module owns `launcher-data/llama-cpp/setup-incomplete`: create the
zero-byte regular file under the existing root lease before source/build/venv
effects; remove it only after artifact/import verification and final cancellation
check. No destructor clears it. Busy or pre-cancelled setup never invalidates a
previous environment. A lease-release failure after successful publication is
cleanup failure, not evidence that the recipe's verified environment is invalid.

Any occupied marker blocks readiness and execution. Only an absent marker or an
existing zero-byte regular marker admits an explicit repair recipe; directories,
symlinks and nonempty files refuse mutation without removal or overwrite.
Inspection I/O errors remain failures, not absence. Synchronous boolean helpers
remain conservative. Only successful explicit repair clears the marker; retained
setup receipts are operation history, not current installation validity.

The persisted contract is process-exit/reopening visibility on stable local
paths, not power-loss durability, hostile/manual marker deletion, older-binary
overlap or positive provenance. Absence means no recorded incomplete operation
under this contract; existing installations still undergo normal artifact/import
checks. No migration, legacy decoder, startup repair or live-library rewrite.
Later external mutation provenance remains open. Other installers are outside
this native source/build invariant. Direct execution and independently requested
probes still require caller coordination with setup; the marker is not a lock.

Consumers: the retained ProbeOwner specification for sync/async status and
availability, `has_imatrix`, and the single llama.cpp quantize entry before import,
staging or native effects. That entry covers managed and direct GGUF/safetensors/
IQ paths; managed execution already holds the root lease. Public path getters and
catalog metadata remain locators/descriptions, not installation authority.

Exact write set: core `src/conversion/{native_setup.rs,mod.rs,backend_setup.rs,
readiness.rs,llama_cpp.rs,manager/setup_tests.rs,llama_cpp/readiness_tests.rs}`
(root_diagnostics); RPC `src/handlers/mod.rs` regression (root_capability); core
README and `src/api/conversion.rs`, RPC README, this plan, ledger/issues and parent
plan (root). New native_setup.rs is private. Root serializes Cargo, formatting,
integration and commits. Reports use messages; scope changes return to root.
No GUI/generated contracts, dependencies, extra runtime, real installs/builds,
models/GPU, live data or unrelated files.

Composed-design review: applicable. Native setup owns installation-invalidity
presence; SetupOwner owns operation receipts/lease/cancellation; ProbeOwner owns
read lifetime. Required interleaving is marker creation before recipe effects,
verification before removal, and managed execution checks under the lease.
Callers gain restart-safe refusal without knowing marker paths or setup history;
direct-use exclusion obligations are unchanged. Marker representation changes
stay in native_setup; recipe changes stay in backend_setup; status/probe scheduling
stays in readiness. Dependencies carry native-root identity and inspection
results, not setup snapshot/status. The private module can be verified without
native builds; setup and consumers share policy, not lifecycle state. Deleting
it would spread marker path, type and failure rules across recipe/probe/execution.
One marker replaces unsafe absence of interruption authority; no registry,
version cache, new operation identity or runtime is added.

Acceptance NINVALID (satisfied): focused/integration, automated, simulated native
tools on Linux plus actual temporary filesystem reopening/process-exit evidence.
Verify failed/cancelled setup retains invalidity, fresh backend reads and all
execution routes refuse without import/staging/native effects, explicit retry
restores eligibility, and unexpected marker entries remain untouched. Actual RPC
dispatch must project not-ready then fresh readiness without schema changes.
Supporting gates: core/RPC default/minimal tests, strict lint, formatting and five
canonical plan checks passed, with initial failures and final rechecks retained
in the ledger. FE-I28/FE-I29 remain open. No real model/tool/GUI or Windows/macOS acceptance.
Re-plan for power-loss durability, supported old-binary overlap, broader installer
population or new direct/probe exclusion requirements.

## Native Setup Source And Build Coherence

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--native-setup-source-and-build-coherence).
NBUILD removes the usable-binary shortcut from the llama.cpp
setup recipe. Every admitted recipe configures and clean-builds both targets
against the current checkout after clone/update, then checks artifacts and
Python imports before success. Existing retained setup observation/retry and
root exclusion remain unchanged. A normal unsuccessful optional pull still
warns and uses local source; this does not promise the latest upstream revision.
CUDA configuration is explicitly ON or OFF from the existing compiler check,
so a previous CMake ON value cannot survive absent-compiler detection.

The build inputs are current checkout contents, existing declared Release/CUDA
configuration and selected host tools/environment; outputs stay in the existing
CMake build directory. Setup owns regeneration of those outputs, not deletion
of source or venv. Both output-entry guards precede CMake cleaning. No cache
authority currently covers source edits, configuration and tools: a Git HEAD
receipt would omit material inputs. Therefore an admitted setup rebuilds; this
is not a timestamp/HEAD cache or a reproducible-build promise. Successful setup
does not prove provenance after later external edits. Failed/interrupted native
recipes are now covered by NINVALID above, within its stated persistence limits.

Exact write set: core `src/conversion/backend_setup.rs` and
`src/conversion/manager/setup_tests.rs` (root_diagnostics); core README and
`src/api/conversion.rs`, this plan, its ledger/issues and parent plan (root).
root_capability provides read-only design/source review. Reports use messages;
scope changes return to root. Root serializes Cargo, formatting, integration and
commits. No GUI/generated output, new dependencies, runtime/receipt machinery,
real installs/builds/models/GPU, live library or unrelated files.

Composed-design review: not-applicable to this local recipe correction; no new
module, interface, lifecycle or permanent coordination mechanism is introduced.
Build policy stays in the recipe; command custody and exclusion stay with the
existing setup runner/lease. The codebase-design deletion check rejected a new
revision receipt whose policy and persistence would otherwise spread to reads
and execution without proving all build inputs. Development decision: implement
the reversible setup-success fix and retain interrupted/external mutation
provenance as a separately owned FE-I26 prerequisite.

Acceptance NBUILD (satisfied): focused/integration, simulated Linux, automated
public setup fixtures prove updated-source rebuild despite usable old outputs,
unchanged-source rebuild, explicit CUDA configuration, no success/Python work
after configure/build failure, retry and retained cancellation cleanup. Existing
artifact repair/refusal fixtures remain required. Supporting gates: core/RPC
default/minimal suites, strict lint, formatting and five canonical plan checks
passed; counts and commands are in the ledger.
Controlled tools prove recipe ordering/effects, not real CMake, Git network,
native ABI/GPU compatibility, release artifacts or Windows/macOS behavior.
Re-plan if durable provenance, new readiness enforcement, source pinning or
broader cleaning authority is needed for this setup-success claim.

## Retained Base-Format Readiness

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--retained-base-format-readiness).
Replace the base Python probe's raw child loop and unretained
blocking task with the existing ProbeOwner mechanism. Overlapping reads share
active work; later reads refresh, and setup shutdown closes/drains the base
probe alongside quantization probes before runtime shutdown. The synchronous
boolean surface remains caller-owned and conservative on inspection failure.
Missing interpreter or normal nonzero import exit means not ready; signal,
timeout, spawn and cleanup failures become `ConversionFailed`, not false.
Closed/cancelled async reads return `ConversionCancelled`. This intentionally
aligns base error classification with quantization probes, including RPC
operation-failure/cancellation codes instead of an internal I/O error or false.
No payload schema, method name or boolean-success shape changes.

Base setup invokes the shared synchronous import runner directly inside its
retained setup worker, with its cancellation token and existing five-second
budget. It does not enter the public probe owner or nest blocking-pool work.
Keep one unchanged base import specification, no independent installer/probe
state projection, no installs from reads and no new setup/probe exclusion claim.

Exact write set: core `src/conversion/{manager.rs,readiness.rs,setup.rs,
manager/probe_tests.rs}`, `src/api/conversion.rs`, `README.md`; RPC
`src/handlers/mod.rs` and `README.md`; this plan, its ledger/issues and parent
plan. root_diagnostics owns the conversion source/tests; root_capability may own
only the RPC dispatch regression. Root owns facade/README/plan records, serial
Cargo/fmt, review and commits. Reports use messages; scope changes return to root.
No GUI/generated output, dependencies, real packages/models/GPU, live data,
release work or unrelated file edits.

Composed-design review: applicable. Public read lifecycle belongs to ProbeOwner;
installer repair belongs to SetupOwner. Only imports and the existing command
runner are shared. Admission/cleanup order is required; observation request
lifetime is not process lifetime. Callers retain the same aggregate shutdown
obligation and need no new runtime or manager handle. Probe lifecycle changes
stay in readiness; installation changes stay in setup; import changes have one
specification owner. Dependencies carry import text, paths and cancellation,
not another owner's state. Read and setup results remain independently testable
and cannot authorize one another. Deleting retained base ownership would return
cleanup/observation obligations to every caller. Reusing the current owner and
runner replaces the raw loop without adding a registry, scheduler or fallback.

Acceptance BPROBE (satisfied): automated focused/integration simulated Linux
fixtures through public manager reads prove overlap/drop/fresh results,
signal/timeout/spawn error distinctions, shutdown cancellation and closed
admission, no installation effects and observed process cleanup. Existing owner
and setup tests support one-blocking-thread and retained setup cancellation.
Actual RPC dispatch proves false versus redacted failure/cancellation rather
than successful false on infrastructure failure. Core/RPC default/minimal tests,
strict lint, formatting and plan checks passed as supporting gates. No real imports,
GUI or Windows/macOS acceptance. Re-plan for changed imports, new lifecycle
machinery, setup/probe locking or an incompatible downstream error consumer.

## Managed Setup And Conversion Exclusion

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--managed-setup-and-conversion-exclusion).
Continue FE-I26 by extending the existing exclusive root
setup lease to every managed conversion worker. Acquire before execution reads
or shared-script deployment; retain through native cleanup, output publication
and indexing, and observe release before publishing terminal progress.
Contention fails the admitted operation explicitly, without waiting, installing
or automatically retrying. Conservatively exclude other managed conversions at
the same stable physical root too: base conversions deploy shared scripts.
Independent roots remain independent. Direct backend execution, independently
requested readiness probes, external tools, hostile path replacement and abrupt
runtime/process destruction remain outside this claim.

Exact write set: `rust/crates/pumas-core/src/conversion/{setup.rs,workers.rs,
manager.rs,manager/admission_tests.rs,manager/output_tests.rs}`, core
`src/api/conversion.rs` and `README.md`, RPC `README.md`, `electron/README.md`, this plan, its ledger
and issues, and the parent plan. root_diagnostics owns the listed conversion
source/tests only; root owns facade documentation and plan/README records,
serial Cargo, formatting, review and commits. No new dependencies, runtime,
GUI mutation, real installation/model/GPU work or unrelated file edits.
Reports use agent messages; scope changes return to root before edits.

Composed-design review: applicable. Setup and conversion keep separate operation
identities, cancellation and terminal owners; the root file lease owns only
environment exclusion. Required interleaving is acquisition before environment
use and release after cleanup/publication, not request lifetime. Callers retain
the existing shutdown obligation but no longer coordinate managed setup versus
execution. Changes to exclusion/release stay with the lease helper; conversion
admission/terminal projection stay with WorkerOwner; recipes and public
transports do not acquire duplicate policy. Dependencies carry an owned file
resource and a work result, not a setup state representation. Recipes and
conversion bodies remain independently testable; neither supplies the other's
terminal authority. Deleting the helper would reintroduce the same acquisition,
work-panic observation, lease release and failure composition in every dispatch branch. No second
registry, lifecycle, scheduler or compatibility path is admitted. Brief retained
blocking acquisition/release must not park a blocking-pool worker for the whole
conversion, which would deadlock hosts with one blocking thread.

Acceptance EXCL (satisfied): focused/integration, simulated Linux, automated;
plus system, representative Linux local filesystem, automated for cross-process
locking. Temporary-root evidence proves both contention directions, independent roots,
lease retention through cancellation and dropped shutdown observation, terminal
release after success/failure/no-child panic, and operation with one blocking thread.
Use managed dispatch and existing worker/lease fixtures, not real model tools.
Affected core/RPC default/minimal suites, strict lint, formatting and plan checks
passed. FE-I28 records an unresolved unrelated download-fixture timeout from the
initial minimal run; its unchanged recheck is not a fix claim.
No Windows/macOS, graphical workflow, hostile-root or real tool claim.
Re-plan if lifecycle custody, lock identity or the excluded direct-call/probe
population must change to satisfy this bounded claim.

## Backend Setup RPC And Desktop Projection

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--backend-setup-rpc-and-desktop-projection).
Continue FE-I26/FE-I23 with `start_backend_setup` and
`get_backend_setup` on the existing RPC and optional desktop bridge. Require
an exact snake_case QuantBackend value; start additionally accepts omitted/null
or canonical `expected_previous_operation_id`. Reject extras, aliases, unknown
backends and malformed tokens before dispatch. Use the existing redacted setup
started/status outcomes; no backend echo, new lifecycle, persistence or GUI
controls. JSON-RPC correlation binds the reply to the requested backend; callers
retain that backend alongside its snapshot. Core remains the sole setup/retry
owner. Reads do not install; managed setup/execution exclusion is tracked in the
current exclusion slice above.

Rust owns params, schemas, runtime decode and redacted output constructors.
Generate both desktop/frontend declarations and AJV validators using the existing
Draft7 generator; keep its dialect/vocabulary and current-format discriminator.
New commands are additive; existing base setup commands and independently usable
Rust methods are unchanged. Preload validates new requests and responses; the
same RPC methods are registered with or without inference plugins. Older RPC
servers reject new methods; no fallback to the blocking setup command.

Exact write set: `rust/crates/pumas-rpc/src/{contract.rs,contract/export.rs,
handlers/conversion.rs,handlers/mod.rs}`, `rust/crates/pumas-rpc/README.md`,
`rust/crates/pumas-core/README.md`, `electron/src/{preload.ts,rpc-method-registry.ts,ipc-validation.ts}`,
`frontend/src/types/api-bridge-links.ts`, the six files in
`{electron,frontend}/src/generated/desktop-contract{.ts,.validators.js,.validators.d.ts}`,
`electron/scripts/desktop-contract-conformance.test.mjs`,
`electron/tests/{preload-rpc-contract.test.mjs,ipc-validation.test.mjs}`,
`frontend/conformance/desktop-catalog.test.tsx`, `electron/README.md`, this plan,
its ledger/issues, and the parent plan. root_diagnostics owns only the listed RPC
files and RPC README, reporting through messages and escalating scope changes.
Root owns desktop/frontend/generated/docs, serial Cargo/fmt/generation/commits.
No real installs, models, GPU work, release builds, live library mutation or
unrelated file edits. Composed-design review is not applicable: this adds
selection to existing adapters without changing state/lifecycle composition.

Acceptance BRPC (satisfied): actual RPC dispatch proves backend routing and idle,
invalid/obsolete token, retained setup and shutdown outcomes with controlled
temporary environments; producer request probes and generated decoders agree on
all selected variants and reject malformed cases. Built preload and main IPC tests prove
backend/token forwarding, response validation, no automatic retry and rejection
before IPC. Core/RPC default/minimal suites, strict lint, generation freshness,
desktop tests and frontend types/lint/builds support this contract. No graphical
workflow, real package/GPU, release-artifact or Windows/macOS claim. Re-plan for
new lifecycle, outcome authority or required GUI mutation.

## Backend-Specific Rust Setup Observation

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--backend-specific-rust-setup-observation).
Continue FE-I26/FE-I23 by exposing start/status/guarded retry
through ConversionManager and PumasApi for all four built-in environments.
Reuse each existing SetupOwner, its backend identity, snapshot and retry CAS;
do not add a parallel state machine or expand the downstream backend trait.
PythonConversion selects the base owner used by the existing conversion methods.
Memory-only reads never install or probe. None does not retry retained results;
only the selected owner's matching terminal ID admits a successor. Stale IDs
return that owner's current snapshot; invalid IDs or retries without a record
fail explicitly. Records remain readable after shutdown closes admission.
Managed setup-versus-conversion exclusion is tracked in the current slice above.

The standalone Rust contract is independently usable without transport or GUI.
RPC/desktop projection requires separate protocol/generated-consumer evidence
and follows this prerequisite; no quantization GUI mutation is admitted here.
Existing base setup methods and ensure-method semantics remain supported.

Exact write set: `rust/crates/pumas-core/src/api/conversion.rs`,
`rust/crates/pumas-core/src/conversion/{manager.rs,setup.rs,types.rs}`,
`rust/crates/pumas-core/src/conversion/linux_group.rs` (verification-discovered
procfs disappearance classification; preserve fail-closed custody),
`rust/crates/pumas-core/src/conversion/manager/setup_tests.rs`,
`rust/crates/pumas-core/tests/api_tests.rs`, `rust/crates/pumas-core/README.md`,
this plan, its ledger/issues, and the parent plan. Root owns source/docs and
serial Cargo/fmt/integration/commits; root_diagnostics owns only setup_tests.rs,
reports via its message, and must escalate scope changes. No other files, real
tools, network installation or library data writes. Existing test fixtures own
their temporary roots. Composed-design review is not applicable to this additive
selection surface: owner, lifecycle and composition remain unchanged; the
existing owner is selected by its own identity, not parallel positional policy.

Acceptance BSETUP (satisfied): public manager controlled Linux integration proves
backend selection, shared ensure identity, observational reads, guarded retries,
overlap and shutdown. PumasApi contract checks prove forwarding and rejection
without real installation. Default/minimal core/RPC suites, strict lint, format,
and five plan contracts support acceptance. No real installer/GPU/Windows/macOS
or transport/GUI claim. Re-plan if a new lifecycle or wire shape is required.
The first minimal suite exposed ESRCH when a task disappears during procfs stat
reading. A deterministic held-inode regression reproduced the read_stat failure;
the reader recognizes this disappearance without suppressing other observation
failures. The shared cleanup correction and original full suite passed;
see ledger for the bounded write-set expansion and evidence limits.

## Native llama.cpp Setup Artifacts

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--native-llamacpp-setup-artifacts).
The converter and both native outputs must be usable before Python setup or
success. Output directories, symlinks and other occupied entries refuse CMake
cleaning; source/venv and unrelated files are preserved. Stable paths remain
required. Controlled Linux repair, invalid-artifact refusal, retry and retained
cleanup evidence is recorded in the ledger, not real upstream build/ABI evidence.
The former healthy-output skip policy is superseded by
[Native Setup Source And Build Coherence](#native-setup-source-and-build-coherence).

## Retained Quantization Readiness Probes

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--retained-quantization-readiness-probes).
Continue FE-I26. Replace built-in public path-only
quantization readiness with required-artifact and isolated import checks using
the shared setup runner. Async callers share one retained blocking probe per
backend; later reads observe the prior worker then admit a fresh probe. Dropping
a caller never drops ownership. Close all setup/probe admission before aggregate
shutdown awaits; retain errors and interrupted joins. Reads do not create setup
records, acquire install leases or install anything. Sync boolean reads remain
caller-owned and conservative on failure; callers must finish them before shutdown.

Add a trait async-readiness method with an explicit unavailable error default,
preserving downstream implementations without an inline blocking fallback.
Built-ins implement it; manager status/catalog and Python-dependent execution
await it. GGUF-only llama.cpp remains independent of Python. Public/wire data
types and GUI feature composition do not change. Five seconds bounds each
interactive probe's execution (up to three sequential probes for a status list),
not fail-closed cleanup. Setup retains its separate thirty-second import budget.
Quantization cancellation is checked before and after its shared probe; it does
not cancel another status reader's probe. Execution/cleanup failures propagate.

Agent owns private readiness.rs with retained lifecycle and fixture tests. Root
owns backend/manager/trait wiring, shared import visibility, affected fixtures,
manager consumer tests, API lifecycle Rustdoc and plan records. Root serializes
Cargo/fmt/commits. Captured paths/imports never retain the owning backend. This
private read owner has different results/admission from setup and does not reuse
its mutation snapshots; the command runner remains sole process-cleanup owner.

Acceptance PROBE (satisfied): controlled Linux fixtures prove import truth/failure,
fresh reads, overlap, dropped waiters, shutdown closure and retained cleanup,
current-thread responsiveness, and actual manager/backend consumers. Require full
default/minimal core/RPC tests, strict lint, formatting and five plan contracts.
No real package/GPU/ABI, setup-versus-conversion exclusion, base-format probe
modernization or Windows/macOS execution claim. Re-plan if this needs a new wire
shape, runtime, installation effect or unrelated process supervisor.

## Direction-Specific llama.cpp Artifacts

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--direction-specific-llamacpp-artifacts).
This FE-I26 prerequisite corrects aggregate checks that required a converter even
for GGUF but missed Python for safetensors and imatrix for IQ/forced requests.
Keep aggregate readiness advisory and inspect selected-route artifacts again
before staging or subprocess creation. Every route requires llama-quantize;
safetensors-only additionally requires converter and Python; IQ/forced requests
require llama-imatrix. Preserve mixed-source GGUF preference. Regular, nonempty
files are required; executable artifacts additionally require Unix execute bits.
Other targets have no Unix-mode claim; OS loader/access/ABI checks remain at
execution. No probes, install, cache, new owner, dependency or wire changes.

Root owns llama_cpp.rs, trait/manager Rustdoc, the existing manager output fixture,
fixture write isolation and assertion diagnostics in backend_setup.rs and plan records; agent owns
llama_cpp/readiness_tests.rs. Root serializes Cargo,
formatting and commits. The private metadata predicate owns the common artifact
rule; route selection remains beside the existing pipeline. This passes the
codebase-design deletion test without adding a readiness service or public type.
Async execution uses async metadata and preserves inspection I/O failures;
existing boolean summaries conservatively report inspection failure as false.

Acceptance ART (satisfied): controlled Linux public-backend fixtures prove GGUF
without Python/converter, conditional safetensors/imatrix rejection before
staging/spawn, valid routes, invalid aggregate artifacts and typed inspection
failure. Full default/minimal
core/RPC tests, strict lint, formatting and five plan contracts support this
bounded claim. Public import probes and their lifetime, real package/GPU/ABI
compatibility, source-file validation, stable artifact custody, and Windows/macOS
execution remain unaccepted; no new quantization GUI mutation is admitted.

## Quantization Setup Import Verification

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--quantization-setup-import-verification).
Complete the dependency-repair
part of FE-I26 by checking imports inside retained setup before skipping pip and
again before declaring success. A present interpreter alone is insufficient.
Reuse an existing venv when imports fail; never delete or recreate it as repair.
Missing interpreters may be created. A normal unsuccessful probe or missing
interpreter permits repair; cancellation, timeout, spawn/observation or cleanup
failures do not. Setup probes use isolated Python (-I -B -c), no model loading,
and a thirty-second command budget for native ML runtime imports. This is not
an interactive readiness query or hardware/algorithm compatibility proof.

Agent owns conversion/backend_setup.rs and colocated probe tests; root owns
conversion/manager/setup_tests.rs and frontend/parent plan, ledger and issues.
Root serializes Cargo, formatting, integration and commits. No public/wire
types, features, GUI, dependencies, live installations or models change.
NVFP4/Sherry probe the imports used by their embedded scripts; llama.cpp probes
locally declared dependency modules, not an unvendored upstream script's full
requirements. Existing command custody and root exclusion remain authoritative.

Composed-design review: not applicable to a new architecture. Existing recipes
own skip/install/success decisions and reference their dependency imports; the
existing runner retains probe cleanup and cancellation. A shared private helper
keeps the same verification sequence in the three actual recipes without a new
owner, task, persistent marker, public configuration or validation-history flag.
Removing it would restore interpreter-presence authority or duplicate the same
sequence. Public is_ready surfaces deliberately remain outside this setup-only
claim; no synchronous probes are added to async conversion consumers.

Acceptance IMP (satisfied): automated controlled Linux recipe tests show failed
imports trigger repair without venv recreation, healthy imports skip pip, pip
success with failed imports fails setup, and retry can repair the same retained
environment. Probe tests preserve cancellation/timeout/cleanup failures. Existing
installer/worker tests remain valid. Supporting gates: conversion tests, full
default/minimal core/RPC tests, strict lint, formatting and five plan contracts.
No real dependency/GPU or GUI workflow claim. Re-plan if this requires changing
public readiness, executing model code, or replacing installer lifetime owners.

## Quantization Installer Custody Admission

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--quantization-installer-custody).
Extend the existing retained
SetupOwner to the three built-in quantization recipes. Each concrete backend
owns an Arc to its setup owner; the manager retains the same owners and closes
all setup admission before any drain await. Direct backend users get additive
inherent shutdown_setup methods; the public backend trait is unchanged. Caller
drop does not release installer work or exclusion. Repeated/interrupted shutdown
drains the same receipts and preserves failures. The existing launcher-data lock
serializes base and quantization setup across aliases/processes; other owners
report contention rather than joining. Lock files/directories must remain stable.

Agent owns new private conversion/backend_setup.rs recipes; root owns
conversion/{setup.rs,manager.rs,mod.rs,llama_cpp.rs,nvfp4.rs,sherry.rs,types.rs},
api/conversion.rs, new conversion/manager/setup_tests.rs and frontend/parent
plan, ledger and issues. Root owns integration, formatting, serial Cargo and
commits. No trait additions, wire schemas, GUI, dependencies, live installers
or model data changes. Private program paths allow controlled fixture execution
without mutating process-global PATH. Production defaults retain existing tools.

Composed design is applicable. Recipes own filesystem/command sequencing;
SetupOwner owns admission, cancellation, lease and terminal receipts; its existing
runner owns child/group cleanup; backends own their recipe selection and expose
ensure/drain; manager composes all owned drains. Required ordering is acquire,
recipe, child cleanup, lease release, receipt. No recipe captures its backend or
owner, so there is no ownership cycle. Callers need ensure and explicit shutdown,
not recipe steps or child handles. Recipe changes stay in the recipe Module;
lifetime changes stay in SetupOwner. The existing base owner is extended, not
duplicated. Deleting recipes would spread command policy back through backends;
deleting retained setup would restore request-owned installers. Private Programs
varies executable identity for tests; it adds no runtime, global config or registry.
The single shared lease deliberately trades simultaneous setup for exclusion.

Acceptance INST (satisfied): automated controlled Linux executables traverse each
real backend recipe to prove retained work after dropped waiters, same-owner
joining, failure receipts and cancellation cleanup. Manager tests prove all-owner
admission closes before waiting, interrupted shutdown resumes, and independent
owners cannot mutate the leased root. Existing base setup and worker tests stay
valid. Supporting gates: default/minimal core/RPC suites, strict lint, formatting
and five plan contracts, all passed. No real dependency installation, GPU readiness, new GUI,
hostile process escape or Windows/macOS execution claim. Existing interpreter-
present readiness shortcuts remain FE-I26 follow-up; do not call custody full
preflight. Re-plan if a new public lifetime representation becomes necessary.

## Managed Quantization Request Admission

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--managed-quantization-request-admission).
FE-I26 now rejects invalid
managed quantization inputs before worker admission. Direction selects the
backend; target names must match its catalog exactly, including backend identity.
Preserve existing omitted-target defaults. IQ targets require calibration even
when force is false; forced imatrix is llama.cpp-only. Every supplied calibration
path must name a nonempty regular file at inspection. Missing optional NVFP4/
Sherry calibration remains allowed. Inspection is not content/readability proof
or retained file custody; callers must keep supplied files stable through use.
No new containment promise or restriction to the model root is introduced.

Root write set: core `conversion/manager.rs`, `conversion/types.rs`, new private
`conversion/manager/admission_tests.rs`, and frontend/parent plan, ledger and
issues. Subagent reviews callers/catalogs read-only and reports to root; root
owns all edits, serial Cargo and commits. No GUI, wire schema, dependencies,
installers, live models or direct backend execution changes.

Composed-design review: not applicable to a new architecture; this adds input
checks in the existing managed-request preparation owner, not a Module or
lifetime mechanism. Catalogs retain target authority, request preparation owns
cross-field admission, and WorkerOwner still owns atomic capacity/lifetime.
Deleting the checks would again defer invalid input to spawned workers. Core,
PumasApi and RPC managed starts share this owner; direct QuantizationBackend
calls are lower-level execution with existing preconditions, not managed starts.
Python format-conversion admission is unchanged.

Acceptance QREQ (satisfied): automated public-manager contract tests with isolated
library/file fixtures prove precise InvalidParams rejection without progress,
staging or backend setup effects. Focused preparation tests cover each backend's
catalog/default, IQ/force cross-fields and supplied-file checks. Supporting gates
are conversion tests, default/minimal core/RPC suites, strict lint, formatting
and five plan contracts, all passed. No model algorithm, GUI workflow, installer readiness,
hostile-path replacement, calibration content or hardware claim. Re-plan if
request validation requires a new public representation or lifetime owner.
Installer custody is accepted in the subsequent admission above; remaining
preflight stays FE-I26.

## Python Quantization Progress Admission

Status: `Accepted`; see the
[execution ledger](execution-ledger.md#2026-09-08--python-quantization-progress).
FE-I27 is resolved by routing
NVFP4/Sherry stdout through the same nonterminal observations and deferred
failure contract as base Python conversion. Keep worker receipts as the only
terminal authority. Setup/loading map to SettingUp, calibrating to Calibrating,
training to Training, quantizing to Quantizing, exporting to Writing. Unknown
duration remains indeterminate; complete remains Writing at 95% pending cleanup.
Sherry announces each epoch before training it, so observed phase progress is
`(epoch - 1) / epochs_total`, never completed epochs inferred from an announcement.
Missing epoch pairs remain indeterminate; invalid provided pairs fail after
cleanup. Epoch transport fields stay private, leaving public/wire types unchanged.

Write set: subagent owns new private core `conversion/script_process.rs` and
colocated tests. Root owns core `conversion/{mod.rs,manager.rs,nvfp4.rs,sherry.rs,
progress.rs}` and `conversion/manager/output_tests.rs`, plus frontend/parent
plan, issues and ledger. No scripts, GUI production files, schemas, dependencies,
native cleanup implementation, feature gates or model data change. Root owns
serial formatting, Cargo, integration and commits; other agent work is read-only.

Composed design is applicable: the script runner owns stdout decoding and sticky
diagnostics; native runner retains process cleanup; tracker owns synchronized
phase snapshots; worker owns terminal receipts; backends retain arguments and
output publication. Required ordering is decode, cleanup, failure decision,
publication and receipt. Three callers no longer need to duplicate that order
or diagnostic handling. Script protocol changes stay in the script runner and
tracker; process-policy changes remain in native execution. The private runner
composes existing owners without a new task, registry, runtime or public type.
Deleting it would replicate protocol/failure policy in three callers. It can
evolve independently of native cleanup because it consumes its existing result.
This removes the base-conversion local parser, not layers a second parser over it.

Acceptance (automated, satisfied): focused stage/epoch checks, real controlled
Linux script-runner tests for mixed output and deferred failure/cancellation,
and actual NVFP4/Sherry backend fixtures held through phases prove observable
progress and no early publication. Existing base-conversion, four-backend output
and worker/UI terminal-contract evidence remains applicable. Supporting gates:
default/minimal core/RPC tests, strict lint, formatting and five plan contracts
passed. The ledger retains the initial unrelated download-test timeout and
successful isolated/full reruns; it does not claim that timeout's cause is fixed.
No actual GPU/model conversion or new graphical workflow claim. FE-I26 setup,
hardware/preflight and stronger containment stay open. Re-plan if exposing epoch
fields publicly or changing script execution behavior becomes necessary.

## Conversion Terminal Authority Admission

Status: `Accepted` for terminal authority; see the
[execution ledger](execution-ledger.md#2026-09-08--conversion-terminal-progress-authority).
FE-I27 is systemic across script observations and worker outcomes:
Python complete/error records previously exposed terminal state before native
cleanup, publication and indexing. Managed worker receipts own terminal status;
scripts own only nonterminal progress and failure diagnostics. Complete records
mean Writing at 95%, not operation completion. Script failure remains local
until native cleanup, then returns failure even if the script exits zero.
Terminal snapshots resist late script observations; successful receipt sets
100% and clears stale error. Percentage alone never authorizes completion.

Root write set: core `conversion/{progress.rs,manager.rs,pipeline.rs,types.rs}`
and `conversion/manager/output_tests.rs`, plus this plan, issues, ledger and
parent summary. Subagent owns only
`frontend/src/hooks/useModelConversionWorkflow.test.ts`; no production UI edits.
Root serializes Cargo, formatting, plans and commits. No schemas, dependencies,
feature gates, native execution owners, live models or installers change.

Composed-design applicability: applicable to terminal authority. Existing
tracker owns synchronized snapshots; worker receipts own terminal outcomes;
pipeline owns metadata/index effects; scripts own execution observations.
These concerns must sequence cleanup, publication, indexing and receipt, but
must not share authority for terminal status. Callers keep existing read/start/
cancel Interfaces; no new registry or state machine is added. Completion-policy
changes stay in the tracker/receipt, while script parsing stays with execution.
Remove duplicate pipeline completion writes; deleting the tracker would spread
snapshot policy across callers, whereas deleting those writes removes accidental
authority. The composition retains one tracker and one worker owner, not an
additional supervisor. Wire shapes are unchanged; this corrects the existing
meaning of Completed/Error rather than adding compatibility behavior.

Acceptance (automated, satisfied): focused tracker checks; real controlled Linux
process tests held after complete/error and during metadata publication prove
nonterminal get/list until cleanup and receipt, then success with indexed output
identity or failure/cancellation without published output; existing all-backend
output integration stays valid.
Optional UI hook tests prove continued polling and no refresh while writing/
importing, followed by one confirmed-completion refresh. These are backend
system/integration and simulated UI-consumer claims, not a new graphical or
real-model workflow claim. Default/minimal core/RPC tests, strict lint, focused
frontend tests/type checks and plan contracts passed. See the ledger for exact
counts and evidence limits. Nonterminal NVFP4/Sherry projection is now accepted
in the subsequent admission above.

Limits: stronger containment, installer ownership and preflight remain FE-I26. No new
quantization GUI mutation is admitted. Re-plan if terminal truth needs a changed
wire contract or a new lifetime owner.

## Linux Conversion Group Custody Admission

Status: `Accepted` for bounded cooperating Linux groups; not full hostile-process
containment. See the [execution ledger](execution-ledger.md#2026-09-07--linux-cooperating-conversion-groups).
FE-I26 remains open for the prerequisites listed below.

Systemic finding: native execution lacks group cleanup; setup signals a numeric
group after `try_wait` may have reaped its leader. Retain the child without
reaping, observe exit through Linux `waitid(WNOWAIT | WNOHANG | WEXITED)`, signal
only its own still-owned group, observe group tasks stopped, then reap. The host
must retain ordinary SIGCHLD semantics and exclusive wait ownership for these
children. `ECHILD` forbids further group signals; it is not cleanup success.

Write set: group agent owns new private `conversion/linux_group.rs`,
`conversion/setup.rs` integration and their tests. Root owns
`conversion/native_process.rs`, `conversion/mod.rs`, `conversion/types.rs`
(lifecycle Rustdoc), `conversion/manager/output_tests.rs`, and frontend/parent
plan, issues and ledger. Rust paths are under `rust/crates/pumas-core/src`.
Root also owns the core Cargo manifest: review found nix 0.29 cannot represent
realtime terminating signals, so enable `process` on the existing pinned rustix
dependency for lossless non-reaping wait status. No new dependencies, generated
schemas, GUI controls, live models or installers.
Root serializes formatting, Cargo and commits; unrelated work stays untouched.

Composed design: native streaming and blocking setup keep their distinct
execution owners but share Linux non-reaping observation, group signalling and
live-task interpretation. Only lifecycle owners know the signal/observe/reap
ordering; backend parsers and argument builders remain unchanged. Kernel state
parsing and ownership checks change in one private Module. Read-only `/proc`
scans may use host blocking capacity while the async owner retains the child;
no queued blocking work may signal a PID without also owning its unreaped child.
Signal syscalls remain nonblocking and local to the retained owner. Removing
the shared Module would duplicate Linux identity and liveness policy across
setup and execution; no second registry, runtime, FFI or supervisor is added.

Binding scope: spawn native Linux commands into their own process group. On
success, error, cancellation or caught callback unwind, keep the leader
unreaped until group cleanup is observed. Account for live worker threads even
when a process leader is a zombie; inaccessible or ambiguous `/proc` state is
not proof of cleanup. Preserve failed observation as a failure and keep custody
while cleanup is unresolved. Managed shutdown retains the async operation;
unmanaged future drop still does not guarantee completed cleanup.

Acceptance claims (automated): real Linux controlled-process `system`
evidence proves non-reaping exit observation, no post-reap group signalling,
cleanup of same-group descendants on normal exit/cancel/unwind, thread-aware
observation, and setup/native integration before publication or release.
Existing pipe, progress, output-identity and retained-shutdown tests remain.
Default/minimal core/RPC suites, strict lint, formatting and all five plan
contracts passed: 66 focused conversion tests, 1,427 default and 1,387 minimal
core/RPC tests, 22 existing ignored tests in each full configuration, and strict
lint in both configurations. The zombie-thread-leader fixture requires `cc` and
pthreads. No real model tools, GPU or installer runs.

Verification-driven adjustment within setup ownership: the minimal gate found
an immediate retry reporting busy, while 30 isolated retry runs passed. The
old lease closed without explicit unlock; a duplicate-descriptor regression
reproduced this lifetime mechanism before the fix. The Linux advisory lock now
unlocks explicitly only after owned execution drains, retains unresolved
release, and preserves release errors. No timing sleeps or retry-on-busy
fallback mask the exclusion contract. Inherited fork involvement in the
original run remains an inference, not captured process evidence.

Limits: descendants that use `setsid`/`setpgid`, escape namespaces or change
credentials are not contained by this contract. No process-global subreaper,
new cgroup authority or Windows/macOS runtime claim is admitted. Non-Linux
foreground execution/setup behavior remains unchanged. Stronger containment,
quantization installer ownership, preflight and FE-I27 remain open. Re-plan if
safe identity or thread observation cannot be established in this scope.

## Foreground Conversion Process Admission

Status: `Accepted` for foreground-child execution on Linux; see the
[execution ledger](execution-ledger.md#2026-09-07--foreground-conversion-execution).
FE-I26 remains a backend prerequisite, not a new GUI admission.

Systemic finding: all six execution calls pipe both streams but consume only
one, and cancellation is checked only between lines. Quiet children ignore
cancellation and unread pipes can deadlock. Replace these split stream/wait
lifecycles together. One private native-process runner owns spawn, bounded
concurrent pipe reads, cancellation observation, direct-child termination and
reaping. Callers retain argument and progress-parsing policy. No detached drain
tasks, new runtime, dependency or generated contract is admitted.

Write set: subprocess agent owns `conversion/native_process.rs` and its
colocated tests. Root owns `conversion/mod.rs`, `pipeline.rs`, `manager.rs`,
`manager/output_tests.rs`, `llama_cpp.rs`, `nvfp4.rs`, `sherry.rs`, `types.rs`
(quantization cancellation/staging Rustdoc only), and this
plan/ledger/issues plus parent-plan summary. Rust paths are beneath
`rust/crates/pumas-core/src`. Root serializes formatting, Cargo and commits.
Existing workflow/stub deletions and recovery artifacts stay untouched.

Composed design: command policy varies by backend; process lifecycle does not.
The runner's Interface accepts a configured command, diagnostic name, token and
line callback; callers no longer own pipes, child handles or reap ordering.
The callback carries only stream identity and bounded text, keeping parsing
independently changeable. The retained manager worker still owns async
completion and admission. A cancellation/pipe-policy change touches only the
runner; a progress-format change touches only its backend callback. Removing
the runner would restore lifecycle duplication at six callers. Its necessary
machinery is bounded line framing, concurrent I/O and a single owned child;
there is no second task registry or new composition root.

Binding foreground contract: observe the existing token every 50 ms while
waiting; bound each text record to 64 KiB; malformed or oversized text fails
without retaining unbounded output. After direct-child exit, allow one second
to drain buffered output; pipes still open then produce a failure, not truncated
success. Cancellation and reader/parser failures (including unwind) await
direct-child reaping before returning. An unobservable cleanup retains the
worker rather than releasing its execution slot. Direct callers must cancel
and await; dropping an unmanaged future is not a cleanup guarantee.

Acceptance claims (automated, satisfied within the ledger's Linux and controlled
fixture boundaries): foreground process behavior is `system`
evidence using real Linux shell processes and controlled tiny payloads: quiet
cancellation, both-pipe flooding, bad records, spawn/nonzero failure, callback
panic and inherited-pipe timeout. Consumer `integration` evidence preserves
actual output metadata/index/progress through all four conversion backends.
Core/RPC default/minimal suites, strict lint, formatting and all five plan
contracts support acceptance. No real model tools or live library mutation.

Boundary and next prerequisite: this slice does not prove descendants stopped
writing. Process-tree identity/containment remains open, as do installer custody
and quantization preflight. Do not generalize setup's post-reap group-kill loop:
numeric group identity may be reused after the leader is reaped. Re-plan if
foreground ownership cannot be guaranteed without a broader lifecycle change.

## Conversion Output Publication Admission

Status: accepted for unique staging, non-replacing publication and actual output
identity; see the [execution ledger](execution-ledger.md#2026-09-07--conversion-output-publication).
FE-I26 remains open for native cleanup, installer custody and preflight.

Inspection found deterministic staging removed on every attempt, and a finalizer
that chose a versioned path but returned no identity: callers then wrote metadata
or returned the original occupied output. Replace shared staging with an
exclusively created per-attempt directory and return the actual published path.
Reuse the platform non-replacing directory move; occupied files, directories and
symlinks must never be replaced. Version selection retries only actual collision
outcomes, not other I/O failures. Failed, cancelled and abandoned staging is
retained; later cleanup requires proof that native producers have stopped. No
automatic Drop deletion may race native work. Existing staging and outputs stay
untouched.

Write set: output agent owns new private `conversion/outputs.rs` and its tests
plus `conversion/mod.rs`. Root owns `conversion/pipeline.rs`, `manager.rs`,
`llama_cpp.rs`, `nvfp4.rs`, `sherry.rs`, the filesystem helper's descriptive
comments, `conversion/manager/output_tests.rs` for simulated-executable consumer
regressions, and frontend/parent plan, issues and ledger. Rust paths above are under
`rust/crates/pumas-core/src`; no dependency, generated schema or UI changes.

Design: one output-workspace interface owns staging identity and desired
destination, and consumes itself to return the actual publication identity. The
four execution paths share it; deleting it would duplicate allocation and
non-clobbering version selection. Existing host blocking capacity isolates
filesystem operations; no runtime or generic framework is added. Parent paths
must remain stable; this does not establish hostile path-replacement protection,
process-tree cleanup, crash-atomic indexing or installer ownership.

Acceptance: temporary-root tests prove exclusive staging, old data preservation,
occupied/dangling destination behavior, independent concurrent publication,
actual returned path and surfaced non-collision errors. All execution callers
must consume that identity. Run core/RPC tests with default/minimal features,
strict lint and formatting, and plan checks. No real model conversion or
installation, live library, GUI or other-OS runtime claim.

## Conversion Worker Observation Admission

Status: accepted for Rust-worker observation and shutdown composition; see the
[execution ledger](execution-ledger.md#2026-09-07--conversion-worker-observation).

Scope: cancellation requests signal the retained conversion worker rather than
aborting it and publishing an unobserved terminal status. Shutdown observes
retained workers; dropping a shutdown waiter must not detach their join handles.
Shutdown closes admission before draining, including starts awaiting model lookup;
capacity and registration share that admission gate. Repeated cancellation
must not cancel already-terminal conversions. Worker
failure/cancellation is observed before reclaiming its retained record.

Write set: backend agent owns `rust/crates/pumas-core/src/conversion/manager.rs`
and a private conversion worker owner module with colocated tests, plus its
module declaration. Root owns `rust/crates/pumas-core/src/api/conversion.rs`,
`rust/crates/pumas-rpc/src/server.rs`, this plan, issues, ledger and parent-plan
summary to expose and compose the retained drain for standalone consumers.
Existing generated response shapes and optional frontend gates remain unchanged.
No new native installation/conversion, GUI controls or live library mutation.

Verification-driven addition: root may update the existing reopen visibility
assertion in `frontend/src/components/ModelConversionDialog.test.tsx`. The
affected contract check found it testing visibility before modal animation;
wait for the intended visible state without changing production rendering.

Composed-design review: the private worker owner replaces the manager's split
token/handle maps and progress-count authority. Its interface hides admission,
cancellation, join observation and receipt retention; the manager supplies the
operation future, while the public facade and RPC compose shutdown. Read-only
model lookup may finish after shutdown, but closed registration cannot publish
progress or start work. Existing runtime capability is reused; no executor,
dependency or generic task framework is introduced. Controlled futures test the
same owner interface. Deleting the owner would put this lifecycle machinery back
into the manager, so it is not a forwarding layer.

Acceptance: deterministic held-worker cancellation/shutdown/drop-waiter tests,
focused core tests, formatting and strict lint. This is Rust worker observation,
not process-tree custody: backend installer ownership, native child cleanup,
quantization preflight remain explicit follow-ups. GUI
quantization stays withheld until those prerequisites are accepted.

## Conversion Setup Dialog Admission

Status: accepted within the Linux simulated-installer evidence boundary; see the
[execution ledger](execution-ledger.md#2026-09-07--conversion-setup-dialog).

Operation: migrate the optional format dialog to the accepted setup observation
contract. The workflow hook owns serialized reads, admission and polling; the
backend retains installation ownership across dialog closure. Mount and reopen
only read. Explicit installation consent sends the observed terminal identity
for retry; uncertain admission must reconcile through reads, never automatic
mutation. Null status does not prove readiness or resolve an uncertain request.
Active setup blocks conversion and duplicate setup, but permits dialog closure
after admission. Terminal success still requires a readiness check. Read failure
stops polling and exposes manual refresh; stale scopes cannot publish or restart.

Write set: hook agent owns `frontend/src/hooks/useModelConversionWorkflow.ts`
and its test; root owns `frontend/src/components/ModelConversionDialog.tsx` and
its test, this plan, ledger, issues and parent-plan next-slice summary. No backend, generated contract, package,
live library, workflow or cache changes are admitted.

Verification-driven write-set addition: root also owns the idle setup fixture
in `frontend/src/components/ModelManagerIntegrityRefresh.test.tsx` and the
asynchronous focus assertion in `frontend/src/components/ui/ModalDialog.test.tsx`.
The first full suite exposed the former's absent new read response and the
latter's immediate focus assertion before deferred restoration. No modal
production behavior change is admitted.

Acceptance: hook lifecycle tests, dialog consent/status/closure tests, frontend
types/lint/full tests, both production modes and actual built Linux Chromium
interaction through compiled preload with simulated setup outcomes. No native
installation or Windows/macOS claim. FE-I25 is resolved within this scope.

## Conversion Setup Observation Admission

Status: accepted for the core/RPC/desktop contract, not dialog migration; see the
[execution ledger](execution-ledger.md#2026-09-07--conversion-setup-observation).

Operation: `continue` this canonical plan. Extend FE-I25 custody with a non-blocking
start/attach request and read-only latest setup snapshot. The backend owns an
opaque UUID and in-progress/completed/failed/cancelled state. Terminal snapshots
follow installer cleanup and lease release; idle reads do not deploy or install.
The result is scoped to one manager/process lifetime, not persisted history or
cross-process discovery. Independent setup owners still use physical exclusion.

Admission without a previous ID starts only when there is no retained operation;
otherwise it returns current work/result. An explicit retry names the observed
terminal operation. Only a matching terminal ID permits replacement after join;
repeating that request attaches to the successor, even if it has completed.
An old token on an owner without a retained record fails explicitly. Shutdown
closes admission. Existing blocking setup/check methods keep their contracts.

Write set: backend agent owns core setup/types/manager/module/facade and fixtures;
root owns RPC parsing/handlers/projection/schema fixtures, generated artifacts,
desktop registry/preload/typed API and cross-consumer evidence plus these records.
Root serializes Cargo, generation and commits. No new dependency, alternate
runtime, native installation, GUI dialog behavior, persistence or feature gate.
The separate dialog migration remains FE-I25 follow-up, not accepted by bridge
availability. GUI dependencies must not enter the core or standalone RPC crate.

Composed-design review: applicable. (1) Existing setup owns admission, identity,
result and custody; RPC owns disclosure, preload decoding and typed consumers.
(2) Identity and replacement coordinate under one owner; unrelated conversion
jobs do not. (3) Callers start/attach or inspect, never manage installer tasks.
(4) Lifecycle changes stay in setup, protocol changes in contract projection.
(5) Core remains independently usable. (6) Held executable fixtures prove timely
admission, repeat safety and cleanup-before-terminal; producer/preload fixtures
prove exact IDs, nulls, states, bounded errors and failed decoding. (7) Extend the
existing owner, not a parallel operation store. (8) Reuse UUID, schemas and the
existing generator; no persisted replay or generic task framework.

Acceptance: focused admission/retry/read/shutdown tests, default/minimal core/RPC
suites and strict lint, generated freshness and producer-to-preload/typed-consumer
conformance, frontend type/lint and affected tests, standalone HTTP status/admission
against owned fake executables. No actual tool installation or new GUI claim.

## Conversion Setup Custody Admission

Status: accepted for backend custody only; see the
[execution ledger](execution-ledger.md#2026-09-06--conversion-setup-custody).

Operation: `continue` this canonical plan. FE-I25 first establishes backend
custody without changing existing setup/check wire shapes. Same-manager callers
join one admitted setup result; independent managers/processes contend for the
same physical launcher-environment lease before script deployment or installation.
Caller cancellation drops only its wait, never process/lease ownership. Explicit
shutdown closes admission, signals the owned worker and observes terminal cleanup;
RPC shutdown drains setup even when another owner fails. Retain failed outcomes
for current waiters; a later explicit setup request may retry only after release.

The setup owner uses the application's existing runtime. Its bounded probe and
controlled command lifetime retain exclusion until child cleanup is observed.
Linux process-tree behavior requires controlled executable evidence, not just
direct-child kill assertions. Abrupt host death, hostile lockfile/root replacement,
new status/identity protocol, quantization-backend setup and actual installations
are excluded from this custody slice and must not be claimed complete. Missing
platform process-tree authority must be reported, not hidden by an unsafe fallback.

Write set: backend agent owns conversion setup/manager/module and public conversion
facade plus focused tests; root owns RPC server shutdown composition/tests and
plan/README records. Integration also admits a blocking script-deployment adapter
in the existing scripts owner: setup must work with one host blocking thread,
without waiting on nested Tokio filesystem work. Both I/O adapters use the same
embedded manifest/hash policy and have parity evidence; no second installer.
No GUI, generated contract, new dependency or alternate
runtime. Existing readiness probes and headless public setup signatures remain.

Composed-design review: applicable. (1) Setup owns lease, process and result
lifetime; manager exposes its interface; RPC owns aggregate shutdown. (2) Lease
release and child completion must coordinate, unrelated download custody must not.
(3) Callers await setup or shutdown without managing tasks/locks. (4) Native setup
changes stay in the private setup module; lifecycle composition stays in server.
(5) The backend consumes runtime/filesystem capabilities, never GUI types.
(6) Controlled child tests prove custody; server tests prove non-short-circuit
drain and repeated/cancelled waiters. (7) Replace request-owned setup rather than
layer a second installer. (8) Reuse fs2 and the current runtime; no generic task
framework, persisted operation database or download-grant repurposing.

Acceptance: controlled-child contention/cancelled-waiter/shutdown/failure tests,
cross-process Linux exclusion where applicable, all-target strict Rust lint and
default/no-default core/RPC suites. No frontend behavior change, so no new GUI
workflow claim. Next after custody is observable setup identity/state across
timeout/reopen; FE-I25 remains open until that consumer contract is accepted.

## Format Conversion Workflow Admission

Status: accepted within the scope below; see the
[execution ledger](execution-ledger.md#2026-09-06--format-conversion-workflow).

Operation: `continue` this canonical plan. Implement the first usable FE-I23
workflow for complete, current GGUF and safetensors models: show the inverse
format, disclose dequantization limits, explicitly consent to Python tool setup,
start F16 conversion, observe backend progress and request cancellation. Backend
validation remains authoritative. Quantization-specific configuration is excluded
from this admission, not advertised by its control, and remains FE-I23 follow-up.

The dialog owns one hook lifetime. Read existing conversions on open; sequential
one-second list polling runs only while relevant work is active, pauses on errors
with explicit refresh, and ends on close/unmount. Closing does not cancel backend
work. Mutation admission is synchronous and single-flight; dismissal is disabled
while a request is pending. Cancellation acceptance is not terminal cancellation.
An unconfirmed start blocks repeated start in that dialog, with an explicit
status-check/reopen instruction. Completed backend results refresh the library.

Write set: backend agent owns `useModelConversionWorkflow.ts` and its tests;
root owns `ModelConversionDialog`, row/list/manager integration, affected tests,
frontend README and this plan's records. A directly required backend correction
is also assigned to the agent: conversion manager readiness and focused tests.
Inspection found that Python executable existence was incorrectly treated as
completed dependency installation. Probe the required imports with bounded
execution and repair incomplete environments on explicit setup; preserve public
signatures and avoid a new persisted marker protocol. No generated, dependency or
feature-gate changes. Root serializes verification/builds/commits. Reuse the
existing validated desktop methods and ModalDialog focus/dismissal owner.
GUI completion evidence additionally required an optional task-owned fallback
focus ref in ModalDialog and its regression test: library refresh replaces the
original row button. When that opener is removed, close restores focus to the
named library region, preserving the existing opener/parent-modal priorities.

Composed-design review: applicable. (1) Core owns execution, preload owns wire
proof, hook owns async lifecycle and dialog owns presentation. (2) Only current
model/direction and generated outcomes cross these interfaces. (3) Rows know an
open-dialog action, not setup or polling. (4) Lifecycle changes stay in the hook;
visual changes stay in the dialog. (5) GUI depends on backend contracts, never
the reverse. (6) Hook and dialog are tested through their actual interfaces,
then the compiled preload/renderer path is exercised. (7) One workflow hook
avoids duplicated polling and replaces the withdrawn no-op with a real action.
(8) Reuse the existing modal and pull-based conversion API, with no new store,
framework, scheduler or generated contract machinery.

Acceptance: focused single-flight, failure, cancellation, completion and teardown
tests; complete-model entry/direction and partial/cached exclusion; full frontend
types/lint/tests, both builds and isolated Chromium pointer/keyboard dialog,
setup consent, start/progress/cancel and reopen evidence through compiled preload.
Real tool installation/conversion and hardware-dependent quantization are not
authorized verification effects; report fixture execution limits explicitly.

## Incomplete Conversion Control Withdrawal

Status: accepted for control withdrawal only; see the
[ledger](execution-ledger.md#2026-09-06--inert-conversion-control-withdrawn).

Operation: `continue` this canonical plan. FE-I23 permits withdrawing the inert
control; classify the GUI workflow as incomplete, not the backend as unsupported.
The only production handler logs a TODO. Remove that handler, its row/list prop
chain and conversion-only row state/icon. Keep core, RPC, generated contracts and
desktop methods intact. Full conversion interaction remains the next admission;
do not count control withdrawal as implementing start/setup/progress/cancel UI.

Write set: `useModelLibraryActions`, `ModelManager`, `LocalModelsList`,
`LocalModelRow`, `LocalModelRowActions`, `LocalModelInstalledActions`,
`LocalModelRowState`, affected component tests, frontend README and this plan's
issue/ledger plus parent next-state summary. Root owns all edits and commits.
No new state, flag, dependency or native mutation. Composed-design review is
not-applicable: delete a production no-op and its sole forwarding chain without
introducing a replacement composition or changing backend ownership.

Acceptance: complete GGUF/safetensors rows advertise no conversion control;
existing import and model rendering remain available. Component regression,
types/lint/full frontend suite and both built Chromium configurations prove that
bounded presentation claim. Native conversion execution is excluded. FE-I23
stays open until the replacement workflow has explicit setup consent, backend
capability/readiness, owned start/progress/cancel lifecycle and GUI evidence.

## Remaining Conversion Operations Admission

Accepted on 2026-09-06; evidence and exclusions are recorded in the
[execution ledger](execution-ledger.md#2026-09-06--conversion-operation-contracts-accepted).

Operation: `continue` this canonical plan under the user's continuation request.
FE-I24 owns start/cancel/environment/setup/quant-option and backend-readiness
responses. Core remains the headless operation owner. Reuse existing RPC outcome
schemas and generated validators, preserving true/false readiness and cancellation
as distinct valid outcomes, explicit nulls, backend identity, importance-matrix
metadata, and canonical public failures. Validate started identifiers and finite
nonnegative bits-per-weight at the outgoing boundary. Preserve core signatures
and mutation execution; no native tool installation or conversion is authorized
for verification.

Complete the optional desktop bridge with the already-supported backend-status
and backend-setup methods. Append optional calibration-file and force-imatrix
arguments to conversion start without changing existing positional arguments;
derive direction/backend types from the generated contract. Exact request names
and false/null values must survive preload forwarding. Native execution policy
and request admission remain in Rust, not duplicated as UI business logic.

Write set: backend agent owns conditional core schema derives, RPC response
constructors/export/fixtures and affected handlers/tests. Root owns generated
artifacts, preload, frontend conversion/bridge aliases, existing conformance,
and plan/ledger/issues. Cargo, generation and commits are root-serialized.
No new dependency, generator keyword, GUI workflow, persistence or live data.

Composed-design review: applicable. (1) Core operation policy, RPC wire proof,
preload transport and GUI interaction remain separate owners. (2) The necessary
coupling is one request/result contract, not shared execution state. (3) Callers
know generated results and optional start arguments, not native setup details.
(4) Core metadata changes regenerate consumers; transport changes stay in preload;
UI workflow remains FE-I23. (5) Stable core types feed schemas, never vice versa.
(6) Constructors/handlers and preload are independently testable, then exercised
together. (7) Generation replaces handwritten outcomes rather than layering a
new registry or schema system. (8) Retain the existing generator and bounded
constructor validation; backend capability is independent of GUI composition.

Acceptance: actual core option catalog/typed response fixtures, malformed outcome
rejection, exact bundled-preload request forwarding, typed renderer consumers,
safe isolated read/cancel RPC paths, dual RPC suites and strict lint, generated
freshness and affected frontend/Electron gates/builds. Native setup/conversion,
GUI interaction and non-Linux execution are not acceptance claims for this slice.

## Conversion Progress Contract Admission

Status: accepted within the read-only and Linux boundaries in the
[execution ledger](execution-ledger.md#2026-09-06--conversion-progress-contract-accepted).

Operation: `continue` this canonical plan under the user's continuation request.
FE-I14's existing get/list conversion-progress routes are the bounded slice.
Core owns direction/status vocabulary and progress semantics; RPC owns the
existing camelCase, explicit-null, redacted-error projection. Generate the
consumer types/decoders from that authority, reject nonfinite/out-of-range
fractions and unrepresentable counters, and decode both preload methods.
Preserve public core signatures, unknown-conversion `progress: null`, and the
empty list. No GUI or inference-plugin dependency is added to core.

Inspection found no production progress presentation: the conversion action
only logs a TODO. Do not invent a conversion screen or claim GUI acceptance for
these read contracts. Remaining setup/start/cancel/quant-option contracts and
the unfinished GUI action receive separate next-slice dispositions.

Write set: backend agent owns core conversion enum schema derives, RPC progress
constructors, export schemas/fixtures and affected handlers/tests. Root owns six
generated artifacts, preload get/list decoding, frontend conversion type aliases,
actual producer/preload/consumer conformance and plan/issue/ledger records. Root
serializes Cargo, generation, integration and commits. No downloads, conversion
tool setup, model mutation, persistence, new generator machinery or dependencies.

Composed-design review: applicable. (1) Core facts, RPC disclosure/representation,
preload proof and GUI presentation stay independently owned. (2) Only wire
projection/validation must coordinate; read contracts have no UI lifecycle state.
(3) Callers learn the generated response, including explicit nulls and all core
statuses, rather than copied field names. (4) Enum changes regenerate consumers;
numeric/disclosure changes stay in the RPC projection; UI work remains separate.
(5) Dependencies carry existing core values and generated wire types, never a GUI
dependency in Rust. (6) Headless RPC reads and preload consumers are independently
verified, then tested together. (7) Existing schema generation removes the
conflicting handwritten progress shape; no new abstraction is introduced.
(8) Necessary complexity is the existing cross-process representation, handled
by ordinary schema bounds and the existing generator/decoder, without new custom
keywords or a hypothetical conversion UI.

Acceptance: focused producer validation/redaction, all status/direction fixtures,
actual headless RPC missing/list reads, generated freshness/negative decoding,
actual bundled-preload and typed renderer-consumer preservation; affected dual
RPC tests, core compilation, strict lint and frontend/Electron suites/builds.
No native conversion execution, GUI workflow, release or non-Linux claim.

## Import-Picker Contract Admission

Status: accepted within the desktop selection and Linux fixture boundaries in
the [execution ledger](execution-ledger.md#2026-09-06--import-picker-contract-accepted).

Operation: `continue` this canonical plan under the user's continuation request.
FE-I11 owns only native selection, not import execution. Replace the coordinated,
non-persisted desktop-only picker result with `selected` (nonempty exact paths),
`cancelled`, `invalid`, or `unavailable`. Missing windows and rejected native/IPC
work are unavailable, never cancellation. Main owns native adaptation; preload
decodes the closed result; the renderer preserves selected records without path
normalization and shows failed selection with retry. No Rust/RPC import contract,
GUI feature composition, live files, or backend import rules change.

Write set: Electron picker contract module, main/preload and focused contract
tests; frontend bridge type, import-picker hook/tests, ModelManager/SearchBar
presentation and affected tests/conformance; this plan, issues, ledger and parent
next-slice pointer. No generated RPC artifact or new dependency is required.

Composed-design review: applicable. (1) Native choice and its process contract
are desktop-owned; pending/retry/dialog visibility are renderer-owned; imports
remain backend-owned. (2) A pending invocation must retain identity across await,
close and unmount, but cannot own import state after revocation. (3) Callers know
one closed outcome, not Electron's native result/options. (4) Native adaptation
changes stay in Electron; presentation changes stay in the hook/manager.
(5) A type-only frontend dependency points to the small canonical picker contract,
not Electron startup or Rust. (6) Native adaptation and UI lifecycle are separately
testable; their coordinated IPC replacement deploys atomically. (7) Removing the
contract module redistributes native outcome and validation policy across main,
preload and renderer. (8) Retain one narrow decoder and the existing hook/controls,
not a generic dialog framework, schema generator or new runner.

Acceptance: focused producer/decoder and current-invocation regressions; actual
bundled preload into renderer failure/cancellation/selection; visible named retry
and unchanged selection paths; types/lint/full affected suites; representative
built Chromium interaction with isolated native-outcome fixtures. Native OS dialog
automation and backend import execution are not claimed by those fixtures.

## Standalone Backend And Link-Health Contract Admission

Status: accepted within the Linux and registered-link read boundaries recorded
in the [execution ledger](execution-ledger.md#2026-09-06--standalone-backend-and-link-health-contract).

Operation: `continue` this canonical plan on 2026-09-06. User authority requires
the backend to remain independently usable as a Rust library or standalone RPC
process; the GUI is a separately optional consumer, not a prerequisite for backend
features. GUI selection and inference-plugin selection are independent.

Implement `PUMAS_GUI=false` in the existing launcher composition. Default desktop
behavior remains unchanged. Headless build/run/test/install omit GUI artifacts,
Node package installation and Corepack requirements. The Node launcher itself
still requires Node; direct Cargo and RPC binary entry points do not. Preserve
exact backend arguments and failures, never build implicitly on run, and reject
the GUI-only release-smoke action when GUI is disabled. No Cargo GUI feature is
needed: neither Rust crate depends on the separately built JavaScript GUI.

Migrate FE-I10's registered-link health read through the public core Interface,
canonical RPC outcome, generated decoder, preload and UI. Core link scanning
belongs to the existing public LinkRegistry Module; PumasApi and state dispatch
share it. Preserve the public result shape and signature, including the current
registry-wide (not version-filtered) semantics. A transparent validated RPC
projection owns wire-safe counts, healthy/degraded consistency and bounded
operation failure. Schema derivation reuses the core result instead of copying
its fields. The existing AJV generator gets one product refinement for count/status
correlation; ordinary schema semantics remain AJV-owned. Read failure is not a
successful empty report. UI loading/failure never masquerades as current healthy
state; provide visible retry and suppress superseded result application.

Composed-design review: `applicable`. The artifact is a standalone backend plus
optional GUI consuming one link-health read contract. (1) Core owns registered
link facts; RPC owns transport proof; preload decoding and UI presentation are
separate; launcher owns artifact selection. (2) Only request/result identity and
current UI invocation must interleave; GUI build selection cannot control core
availability. (3) Embedded callers know LinkRegistry/PumasApi and Result, RPC
callers know the existing method, desktop callers consume generated output, and
operators select GUI independently from plugins. (4) Scan changes stay in core;
wire changes regenerate consumers; presentation changes stay in UI; GUI selection
stays in launcher. (5) Stable core values cross inward dependencies; no backend
imports generated TypeScript or Electron. (6) Core and RPC build/test independently
of GUI, while wire changes coordinate generated consumers. (7) Moving the repeated
scan into its existing owner removes duplicate policy; deleting the narrow
refinement would permit contradictory reports; no new framework, registry or
runner is admitted. (8) Retained machinery is the existing launcher, core registry,
RPC exporter, AJV decoder and UI state, with ordinary focused tests.

Write ownership: backend agent owns core `model_library/link_registry.rs`,
`api/{links,state}.rs`, `models/responses.rs`, focused public core tests, RPC
`contract.rs`, `contract/export.rs` and affected read handler/tests. Frontend agent
owns `LinkHealthStatus.tsx` and tests, `LinkHealthDetails.tsx` if needed for immutable
arrays, and `types/api-links.ts`. Launcher agent owns
`scripts/launcher/{actions,contract,dependencies}.mjs` and their tests. Root alone
owns generator/refinement tests, six generated artifacts, preload, existing
producer/preload/renderer conformance, affected README guidance and these plans/
ledgers/issues. Root serializes Cargo, generation, integration and commits.

Acceptance: public Rust read with no GUI and no default features; backend-only
build and real isolated RPC read; unchanged public result behavior; valid and
invalid generated report conformance; actual producer/preload/UI visible read,
failure and retry; stale/unmount regressions; launcher delegation and dependency
exclusion with GUI/plugin combinations; affected types/lint/tests and built GUI
verification. No live model files, cleanup mutations, persistence formats,
new dependencies, complete RPC migration, or Windows/macOS runtime claims.
Stop/re-plan if the read requires broader link mutation or persistence repair.

## Download Row Association Bug Admission

Status: accepted after the bounded regression, cross-boundary, dual Rust and
two-mode built GUI checks recorded in the
[execution ledger](execution-ledger.md#2026-09-05--download-to-catalog-row-association-accepted).

User-reported regression at `aa7c483a`: starting a catalog partial adds a second
activity row. The exact-ID merge regression fails with two rows instead of one.
Current progress omits library identity and the renderer deliberately refuses
repo/name/quant guesses. Repair that missing producer-to-consumer relation.

Core projects nullable `libraryModelId` from the already-bound destination
capability's library-relative model path, not repository labels, ambient paths,
filesystem scans or a new persisted mapping. No capability means no association.
RPC list/status and pushed progress share the existing canonical progress DTO;
preload decodes both before renderer use. Generated contracts migrate together.
Recovery admission may immediately carry the exact selected catalog ID into
local activity until canonical updates arrive. Merge only one uniquely associated
activity into a current catalog row, retaining its ID/name/metadata and exact
download controls. Unassociated, ambiguous, or cached-only activity stays separate;
different repositories, artifacts and quants never merge by display similarity.

Review refinement: preserve every distinct download ID through snapshot selection,
optimistic starts and initial-list merging, even when artifact keys collide.
Only repeated observations of the same download ID may coalesce; association
must never transfer to another download ID through an artifact key. Retain the
existing priority-selected primary artifact entry for remote-search consumers.

Write ownership: Rust owner changes core `models/model.rs`,
`model_library/{download_recovery.rs,hf/download.rs}`, RPC
`{contract.rs,contract/export.rs,handlers/mod.rs}` and affected Rust fixtures.
Frontend state owner changes `hooks/{modelDownloadState,useModelDownloads,useActiveModelDownload,
useModelLibraryActions}.ts`, `types/api-models.ts`, `utils/downloadProgressProjection.ts`
and their tests. Root integrates `components/ModelManagerUtils.ts`, `types/apps.ts`,
affected row/manager tests, Electron preload and its existing event/conformance
tests, six generated desktop-contract artifacts, and producer/preload/renderer
conformance fixtures. Existing UI verification machinery and these active plans/
ledgers may record evidence. No search semantics, model files, persistence schema,
dependency, retry, root-exclusion or Pending replay changes.

Evidence: exact association survives queued/downloading/paused/error updates and
restart; one row retains live progress and pause/resume/cancel actions; absent or
ambiguous association and distinct identities remain separate. Core capability
projection, generated invalid-ID rejection, real producer/preload/renderer flow,
frontend tests/types/lint, affected dual Rust gates, generator freshness, and a
representative built GUI check are required. Do not download or delete user model
payloads to verify presentation. The Rust plan owns canonical contract changes;
root serializes generation, Cargo and commits. Full M4/M5 remain unaccepted.

**Execution ledger:** [execution-ledger.md](execution-ledger.md)

**Issues:** [issues.md](issues.md)

**Reports:** Planned report artifacts are indexed in the [execution ledger](execution-ledger.md#reports).

**Audit source:** [Frontend and UI audit](../../../audits/current-standards-2026-09-03/frontend-and-ui.md)

## Objective

Make the renderer consume proof-bearing desktop contracts, preserve fast model
startup without misrepresenting cached state, keep asynchronous installation
state current, provide consistent keyboard/focus/status/motion behavior, and
prove both supported renderer variants in a representative runtime.

This plan preserves the existing strict TypeScript, type-aware lint, snapshot
decoder, model-search stale-result guard, and build-time plugin aliases. It
changes only owners whose current behavior cannot support the audited claims.

## Baseline

- Audit code baseline: `a33c8c0efa7cd8783c7deeac9e608db205290d43`.
- Planning code baseline: `d84e2b3520ce3da3f39cc3df953301fa9d6d3d50`.
- Audit standards baseline: `52b096ded9c53afd439a3cf0efc4cc85252da570`.
- Planning standards baseline: `7bf74bb5a8cb0ffccaff3ec86550051f900fb4bb`.
- Audit evidence at its baseline: frontend ESLint, TypeScript, the then-current
  size check, and 441 Vitest tests in 102 files passed. `check:errors` reported
  31 candidates; the sibling governance plan owns that mechanism. No
  representative browser/Electron workflow or production build was run in the
  audit, so neither is baseline acceptance evidence.
- Current reconciliation standards: `1609c304`; source checkpoints:
  `2b081fba` and `2b9553a0`. Historical baselines above remain historical.
- This plan owns only its `frontend-and-ui/` directory; the program owner
  serializes shared planning changes.

## Objective Acceptance

| ID | Observable criterion | Kind | Environment | Mode | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| FE-A1 | Every desktop RPC response or event used by renderer code arrives through the accepted platform decoder Interface; invalid, unsupported, unavailable, and operation-failure outcomes remain distinct and cannot become domain values through assertions or fallback substitution. | `contract` | `simulated` (generated-decoder test Adapter and malformed payload corpus) | `automated` | `pending` | [Selected consumers accepted](execution-ledger.md#2026-09-05--selected-desktop-consumer-checkpoint-accepted); remaining operation/event inventory and migrations prevent whole-claim acceptance |
| FE-A2 | A valid saved model list renders immediately with visible cached provenance and real or explicitly unknown age; failed refresh remains visibly degraded and retryable; successful refresh replaces it with fresh authoritative state. | `user-workflow` | `representative` (built renderer in its supported desktop browser runtime) | `automated` | `pending` | [Linux cold/warm evidence accepted](execution-ledger.md#2026-09-05--selected-desktop-consumer-checkpoint-accepted); complete degradation/retry/age workflow remains pending |
| FE-A3 | Installation progress has at most one request in flight for its current owner, begins only after the backend admits a requested lifecycle, and completions superseded by app/tag changes or unmount cannot mutate current state; success, failure, and cancellation remain distinguishable. | `integration` | `simulated` (controlled deferred request Adapter and fake clock) | `automated` | `satisfied` | [M0-S1 evidence](execution-ledger.md#2026-09-03--m0-s1-installation-progress-owner-accepted) plus accepted [PRG-I12 repair evidence](execution-ledger.md#2026-09-03--m3-s3a-status-reachability-terminal-retention-and-admission-order-accepted) |
| FE-A4 | Affected modal and popup workflows expose names and state programmatically, support pointer and keyboard operation, contain and restore focus where modal, dismiss predictably, and preserve nested-dialog focus order. | `user-workflow` | `representative` (built renderer with keyboard and accessibility-tree observations) | `automated` | `satisfied` | [M2-S4 Chromium evidence](execution-ledger.md#2026-09-03--m2-s4-representative-chromium-evidence-accepted) |
| FE-A5 | Installation progress and terminal outcomes are programmatically announced without duplicate noise, and the operating-system reduced-motion preference suppresses nonessential CSS and Framer Motion movement. | `user-workflow` | `representative` (built renderer with accessibility-tree and media-preference control) | `automated` | `satisfied` | [M3-S3b real-entry evidence](execution-ledger.md#2026-09-03--m3-s3b-popover-motion-and-terminal-semantics-accepted) |
| FE-A6 | Both frontend build modes start through their real entry point; the default renderer exposes supported inference-plugin UI, while the library-only renderer omits that UI and still completes the core model-library workflow. | `user-workflow` | `representative` (built default and library-only renderers in the supported desktop browser runtime) | `automated` | `pending` | Pending Milestone 5; packaged-artifact claims remain in the platform plan |
| FE-A7 | Frontend behavior documentation describes only the accepted cache provenance, keyboard/focus, reduced-motion, and variant behavior, and routes verification policy to the governance owner. | `focused` | `not-applicable` | `manual` | `pending` | M3 documentation accepted; remaining M4/M5 behavior documentation pending |
| FE-A8 | In Electron, one launcher-root owner prevents backend-consuming renderer content from mounting before a decoded ready state, presents every startup/selection recovery outcome without path disclosure or unsafe retry, and gives both normal-ready entries a first visible model-list frame without a painted checking frame; browser mode remains explicitly not applicable. | `integration` | `representative` (production builds and preload in sandboxed Electron) | `automated` | `pending` | [Selected startup checkpoint accepted](execution-ledger.md#2026-09-05--selected-desktop-consumer-checkpoint-accepted); complete XR-S1 state/two-entry matrix still required |

## Scope

### In Scope

- Renderer consumption of the canonical decoded RPC/result Interface.
- The model-list cached projection, its provenance/freshness/degraded state,
  its presentation, and recovery behavior.
- Installation-progress polling ownership and hook lifecycle.
- Shared modal and popup interaction Modules for current affected consumers.
- Progress/status announcement and reduced-motion behavior.
- Frontend-owned representative workflow machinery for default and
  library-only built renderers.
- Frontend behavior documentation changed by those outcomes.

### Out Of Scope

- Defining Rust RPC DTOs, the canonical error taxonomy, wire schemas, or
  compatibility policy; the Rust focused plan owns them.
- Electron IPC handler validation, generated decoder implementation, preload
  exposure, or process-boundary transport tests; the platform focused plan owns
  them.
- Packaged installer contents, plugin binary inclusion, release signing, or
  packaged-artifact inspection; the platform focused plan owns them.
- Retiring `check:errors`, fixed line/count gates, changing CI schedules, or
  defining the repository verification inventory; the
  [governance and verification plan](../governance-and-verification/plan.md)
  owns F-06 and F-08.
- A general WCAG conformance claim, every screen in the application, or an
  assistive-technology certification claim.
- Opportunistic decomposition based only on file length or complexity counts.

## Constraints And Assumptions

### Constraints

- Renderer code must not duplicate canonical DTO/error schemas or accept
  asserted producer values as proof. It consumes the platform-generated
  Interface once that dependency is accepted.
- The Rust/backend model catalog is authoritative. Browser storage is a
  disposable projection used to satisfy the product requirement that the model
  list appear immediately at startup.
- Version-1 snapshots are retired, not migrated. The accepted version-2
  display cache requires the opaque launcher-root scope; mismatched or invalid
  entries are discarded. Cached paths, tickets, activity, and actions are not
  authority. A missing display scope means no cache, not an invented identity.
- The producer currently exposes no catalog revision. Capture time and source
  may be recorded locally, but must not be described as producer revision.
- Underlying IPC calls are not assumed cancellable. Superseded async work must
  still be observed and classified, while its completion is prevented from
  mutating current state.
- Popup roles must follow actual interaction. Nested action collections must
  not be mislabeled as listboxes or menus merely to obtain a familiar role.
- A test command or dependency is not selected until the bounded renderer
  harness admission proves the environment and oracle it can provide.
- Governance edits to `frontend/package.json` and CI land before Milestone 5,
  or the overlapping package-script change is rebased and reviewed explicitly.

### Assumptions

- The existing build aliases remain the production mechanism for plugin
  separation unless Milestone 5 produces contradictory runtime evidence.
- The current `VersionManagementPanel` is the sole production installation
  dialog caller and already supplies manager-owned progress. The local fallback
  has no reachable production consumer at this baseline.
- A single built-renderer harness can decide focus, semantics, cache
  provenance, motion preference, and mode visibility; Milestone 1 must disprove
  this before admitting a second permanent harness.

## Binding Decisions

| Decision | Module / Interface / Seam | Production and test Adapters | Evidence / consequence |
| --- | --- | --- | --- |
| Deepen `useInstallationManager` as the sole installation synchronization Module for the resolved app/tag identity. Its result/actions Interface owns current progress and every installation lifecycle action, including app-scoped cancellation; serialized polling and generation ownership stay hidden. | Seam between renderer lifecycle and desktop installation operations. | Production Adapter is the accepted desktop API; tests use controlled deferred operations and a fake clock through the hook Interface. | Deletes the unused dialog-local polling owner and unscoped dialog cancellation instead of coordinating lifecycle across callers. |
| Deepen `useModels` as the model-library projection Module. Its small renderer Interface adds provenance/freshness/degraded outcome and retry beside current model groups/actions. | Seam between authoritative catalog refresh, disposable local projection, and presentation. | Production Adapter is the accepted decoded models operation plus browser storage; tests use valid/legacy/malformed snapshots and controlled refresh outcomes. | One owner decides immediate display, freshness, replacement, and recovery. No second store is added. |
| Consume the platform-generated decoded desktop Interface. Keep a frontend API wrapper only when it adds renderer-domain composition or recovery; delete fallback/pass-through helpers that erase outcome distinctions. | Existing Electron/preload process boundary; this plan owns only its renderer consumer side. | Platform owns the production decoder Adapter. Frontend tests use its proof-bearing test Adapter, not hand-authored parallel schemas. | Prevents schema drift and removes `{ } as DesktopBridgeAPI`/arbitrary fallback authority from renderer paths. |
| Add one `ModalDialog` Module whose Interface owns accessible role/name, initial focus, containment, Escape/backdrop policy, nested restoration, and cleanup. Feature content owns titles, messages, and actions. | Seam between modal lifecycle policy and feature content. | No external production Adapter is needed; component tests supply focusable content, and representative tests drive the public component behavior. | Replaces partial duplicated focus hooks/frames. The Module is deeper than a styling wrapper. |
| Add one `Popover` Module whose Interface owns trigger relationship/state, outside/Escape dismissal, focus entry/return, and cleanup. Feature content selects semantics appropriate to its actual actions. | Seam between non-modal popup lifecycle and selector/search/download content. | No external production Adapter is needed; tests drive trigger/content behavior through the Interface. | Does not introduce a generic menu/listbox abstraction or hide feature semantics. |
| Implement progress/status semantics in the existing progress view and motion preference at the frontend composition root/CSS boundary. | Existing presentation owners; no shared Module until a second consumer needs the full lifecycle. | Representative harness controls progress and media preference. | Avoids a hypothetical announcement framework for a single current lifecycle. |
| Use the installed Electron runtime and Chromium DevTools Protocol behind one package command; a frontend runner owns temporary builds/process cleanup, an Electron main Adapter owns browser input/accessibility/media observation, and deterministic fixtures own only external operations. | Seam between built renderer behavior and acceptance evidence. | Real built renderer and production preload are the production subject; IPC fixtures replace only external operation providers. | [Admission evidence](reports/renderer-harness-admission.md) reached the intended modal failure without a new dependency; jsdom and release smoke cannot decide the same outcomes. |
| Deepen one `LauncherRootRecoveryProvider` at the renderer composition root as the sole owner of startup admission and library-root selection presentation. | Seam between the platform-decoded launcher-root Interface and backend-consuming renderer trees. | Production Adapter is the accepted sandboxed preload API; tests supply controlled startup/selection promises, and the representative oracle supplies exact IPC fixtures. | Browser mode renders children as not applicable. Electron withholds visibility acknowledgement through initializing, then acknowledges exactly once after ready, recovery, or unavailable presentation commits; the main process owns actual window visibility and its bounded native fallback. |

## Dependencies And Ownership

| Dependency | Provider | Consumer milestone | Ready condition |
| --- | --- | --- | --- |
| Canonical RPC DTO/error taxonomy and compatibility policy | Rust focused remediation plan | Milestone 4 | Producer contract is accepted and versioned; invalid/unsupported/unavailable outcomes are named. |
| Electron/generated response and event decoders | Platform focused remediation plan | Milestone 4 | Renderer-visible Interface is proof-bearing and has invalid-payload tests. |
| Packaged variant/artifact proof | Platform focused remediation plan | Program acceptance only | Platform plan owns installer/package inspection; FE-A6 does not claim it. |
| Count/error gate and CI schedule cleanup | [Governance and verification plan](../governance-and-verification/plan.md) | Milestone 5 | Its `frontend/package.json`/CI edits are integrated before frontend adds the selected package-local representative command. |
| Renderer harness selection | Milestone 1 of this plan | Milestones 2, 3, 4, and 5 | Admission report identifies one capable tool, cleanup protocol, duration, and independently observable oracle. |

Integration order is Milestone 0, Milestone 1, Milestones 2–3, then Milestone
4 when the Rust/platform contracts are ready, and Milestone 5 after governance
package-script changes. Milestones 2 and 3 may be reviewed independently after
Milestone 1, but their shared UI primitive/export files must be serialized.

## Evidence And Oracle Plan

| Claim | Deciding oracle | Independent authority | Deliberately unsupported by that evidence | Intended negative failure |
| --- | --- | --- | --- | --- |
| FE-A1 | Generated decoder contract tests plus renderer consumer tests that inject malformed, unsupported, unavailable, and operation-failure outcomes | Accepted Rust schema/error contract and platform decoder output | Backend implementation correctness and transport delivery | Any invalid payload reaching model or presentation state fails |
| FE-A2 | Built-renderer workflow observes immediate list content, explicit cached/unknown-age state, degradation/retry, and fresh replacement | Controlled authoritative-response Adapter and storage fixture | Cross-device cache validity or producer revision | Removing the provenance indicator or allowing a failed refresh to appear fresh fails |
| FE-A3 | Hook test with deferred operations records maximum active count and state after supersession/unmount | Invocation generation and resolved app/tag identity observed outside the hook | Cancellation of non-cancellable IPC work | A second request overlaps or an old completion mutates current state |
| FE-A4 | Built-renderer keyboard workflow and accessibility-tree/focus observations | Browser/Electron focus and accessibility behavior | Screen-reader product certification | Missing name/state, escaped modal focus, broken Escape, or failed restoration fails |
| FE-A5 | Accessibility-tree outcome observations and reduced-motion media emulation against the built renderer | Browser accessibility tree and operating-system media preference | Every decorative transition in third-party content | Silent terminal state, duplicate announcement, or nonessential motion under reduce fails |
| FE-A6 | Separate production builds launched through their real entry points and driven through a core model workflow | Build mode/entry configuration and runtime-visible controls | Installer contents or absence of strings in minified bundles | Plugin UI appears in library-only mode or the core library workflow fails in either mode |
| FE-A8 | Provider tests drive the closed startup/selection state matrix; real sandboxed Electron runs the default and library-only production entries with compositor presentation-frame capture at the native reveal boundary | Accepted platform decoder contract plus browser frame and renderer-console observations | Packaged installer execution and general startup performance | Backend content mounts before ready, a typed state gains the wrong action, startup reads overlap, the native reveal can expose checking, model-list content never reaches a presented frame, or a renderer/preload error is emitted |

For new permanent renderer verification machinery, Milestone 1 must record its
claim, reachable negative failure, independent oracle, marginal value versus
Vitest/release smoke tests, expected runtime, cleanup behavior, and retention
trigger. The package-local command is scheduled by governance rather than
silently added to every local hook.

## Development Proportionality

### Admitted Investigation: Representative Renderer Harness

- **Uncertainty:** Whether the existing Electron/Chromium launch path can be
  driven to observe accessibility roles, focus, media preference, and both
  built variants, or whether one bounded browser-driving dependency is needed.
- **Decision unlocked:** Extend the existing launch tooling or admit one
  frontend-owned runner and fixture protocol.
- **Consequence of guessing:** The project could retain a test that exercises
  jsdom rather than the real browser behavior, or acquire a costly duplicate
  runtime harness.
- **Cheapest discriminating check:** Build one mode, launch it with an isolated
  profile and deterministic desktop-operation fixture, and drive one modal
  open/focus/Escape/restore workflow while reading the browser accessibility
  representation.
- **Stopping condition:** The approach observes accessible role/name/focus and
  built-entry mode behavior, terminates all child processes deterministically,
  and has a bounded measured duration; otherwise the report records a typed
  unsupported/unavailable result and the plan is revised before tool changes.

### Deferred decisions

- The package-local command name remains integration-owned until Milestone 5;
  the admitted implementation uses existing Electron/Vite/Node dependencies
  and the three-file owner split recorded in the admission report.
- Unmigrated desktop operations remain provider-owned. The selected catalog,
  search, download, and ticket-recovery Interface is accepted; further
  operations extend that canonical generation path rather than adding a
  temporary schema.

## Systemic Finding Audit

- **Invariant families and canonical owners:** decoded desktop values are owned
  by the Rust/platform contract chain; cached model provenance by `useModels`;
  current installation progress by `useInstallationManager`; modal lifecycle
  by `ModalDialog`; non-modal popup lifecycle by `Popover`; announcement/motion
  by their presentation/composition owners; variant behavior by the frontend
  build entry and aliases.
- **Bounded population:** all files under `frontend/src/api/`; model snapshot,
  `useModels`, and its two app composition roots; installation manager/progress
  hooks and sole dialog caller; current dialog frames and feature dialogs;
  version/search/download popups; progress view; frontend entry/CSS; default
  and library-only Vite entry/alias paths.
- **Expansion facts:** expand only when a searched renderer consumer accepts the
  same undecoded value, another production owner starts the same installation
  poll, or another overlay/progress consumer shares the full audited lifecycle.
  Record the evidence and re-plan its exact files before editing.
- **Consumer dispositions:** migrate reachable consumers to the canonical
  owner; delete unused fallback owners and duplicated focus hooks; leave
  unrelated hook timers/popups with an explicit inventory disposition rather
  than silently widening the repair.
- **Alternatives considered:** event delivery before polling; existing launch
  tooling before a new test dependency; direct semantic HTML before a generic
  component; existing bridge Interface before another API facade.
- **Evidence-backed stopping condition:** every member of each bounded
  population has `migrate`, `already safe`, `delete`, `external owner`, or
  `follow-up issue` recorded in the applicable report/ledger, and every
  acceptance claim has one deciding oracle.
- **Repaired-composition comparison:** the target deletes one polling owner and
  duplicated focus machinery, adds only two reusable interaction Modules and
  one admitted renderer harness, and does not add schemas, stores, or generic
  service layers.

## Simplicity And Ownership Review

**Applicability:** `applicable`

- Independent concepts and dimensions: decoded transport values, authoritative versus cached catalog state, current async invocation, modal policy, popup policy, announcements, motion preference, and build mode can change independently and retain separate owners.
- State, identity, value, time, policy, and mechanism: model groups are values; source/freshness and installation phase are state; app/tag and request generation are identity; snapshot capture and invocation generation own time; accessibility/variant rules are policy; storage, IPC, focus management, polling, CSS, and the harness are mechanisms hidden behind their owning Interfaces.
- Caller and composition-root knowledge: app roots select the model projection and motion policy; feature dialogs/popups provide content and actions without reimplementing focus policy; renderer consumers know decoded operation outcomes but not wire validation internals.
- Representative change paths and forced owners: changing cache freshness touches the projection Module and its notice; changing modal dismissal touches `ModalDialog` and its Interface tests; adding an RPC field starts at the provider schema/decoder and reaches only consuming renderer behavior; adding a supported mode extends the build-mode matrix and representative workflow.
- Stable Interfaces versus hidden knowledge: hook results, modal/popover props, and the platform proof-bearing API are Interfaces; snapshot envelope details, request generations, focus sentinels, wire parsing, and harness process control remain hidden knowledge.
- Independent evolution, testing, failure, and replacement: each Module is tested through its Interface; production and test Adapters meet the same external Seam; platform decoders, storage fixtures, and renderer driver can be replaced without feature components learning their internals.
- Necessary complexity and containment: only external desktop/storage/runtime Seams receive Adapters; modal and popup Modules contain real repeated lifecycle policy; status/motion stay local until reuse is evidenced; no registries, pass-through facades, or speculative abstraction are admitted.
- Deletion and cumulative machinery result: delete dialog-local polling, arbitrary fallback helpers without domain meaning, and duplicated focus lifecycle code; replace them with fewer canonical owners, while retaining one renderer harness only if it supplies evidence existing Vitest and release smoke checks cannot.

## Risks

| Risk | Control |
| --- | --- |
| Immediate cache display is mistaken for current backend truth | Root-scoped display-only cache, explicit cached/degraded presentation, and successful-refresh replacement test |
| Late installation completion mutates a new app/tag | Serialized loop plus generation/current-owner checks and deferred-promise tests |
| Shared modal/popover primitive changes feature behavior | Migrate one representative consumer first, test through the Interface, then migrate only matching consumers |
| Nested dialogs restore focus to a removed element | Stack-aware restoration with connectivity fallback, exercised in representative runtime |
| Accessibility tests pass in jsdom but fail in Chromium/Electron | Representative harness is a prerequisite for acceptance, not optional corroboration |
| Remaining contract migration drifts from its provider | Coordinate each remaining operation with canonical generation; selected catalog contracts are already accepted |
| Governance and frontend both edit package scripts | Governance integrates first; Milestone 5 rebases and owns only the package-local renderer command |
| A new harness becomes slow or flaky | Admission requires measured duration, deterministic fixtures/process cleanup, unique oracle, and a retention trigger |

## Milestones

### Milestone 0: Own Installation Progress Lifecycle

**Goal:** Establish one serialized, current installation-progress owner and
remove the unused second polling loop.

**Allowed write set:**

- `frontend/src/hooks/useInstallationManager.ts`
- `frontend/src/hooks/useInstallationManager.test.ts`
- `frontend/src/hooks/useInstallationProgress.ts`
- `frontend/src/hooks/useInstallationProgress.test.ts`
- `frontend/src/components/InstallDialog.tsx`
- `frontend/src/components/InstallDialog.test.tsx`
- `frontend/src/hooks/useVersions.ts`
- `frontend/src/hooks/useVersions.test.ts`
- `frontend/src/hooks/useSelectedAppVersions.test.ts`
- `frontend/src/hooks/useVersionFetching.ts`
- `frontend/src/hooks/useVersionFetching.test.ts`
- `frontend/src/hooks/useAvailableVersionState.ts`
- `frontend/src/hooks/useAvailableVersionState.test.ts`
- `frontend/src/components/app-panels/VersionManagementPanel.tsx`
- `frontend/src/utils/appVersionState.ts`
- `reports/frontend-async-owner-inventory.md`
- this plan, ledger, and issues files

**Tasks:**

- [x] Record the bounded installation/timer consumer inventory and disposition;
  unrelated polling owners become follow-up issues unless they share the exact
  current-invocation defect.
- [x] Replace interval overlap with a serialized self-scheduling loop whose
  next request starts only after the current request settles.
- [x] Start polling a requested install only after the backend accepts that
  lifecycle; retain immediate polling only for an existing release-discovery
  lifecycle.
- [x] Give each enabled app/tag lifecycle a generation and prevent superseded
  or unmounted completions from mutating state.
- [x] Keep non-cancellable completions observed and classified; do not abandon
  rejected promises.
- [x] Remove the unreachable dialog-local polling fallback and narrow its
  presentation Interface.
- [x] Route cancellation through the manager with the resolved app identity;
  keep cancellation distinct from failure while its terminal progress is
  observed.
- [x] Add controlled overlap, app/tag change, disable/unmount, success,
  failure, and cancellation tests.

**Acceptance gate:** FE-A3 plus passing affected typecheck, lint, and focused
Vitest evidence.

**Status:** `Accepted` after PRG-I12 repair review

### Milestone 1: Admit Representative Renderer Evidence

**Goal:** Select the smallest permanent runtime harness that can decide the
renderer-only user-workflow claims.

**Allowed write set:**

- `reports/renderer-harness-admission.md`
- this plan, ledger, and issues files

**Tasks:**

- [x] Run the admitted one-workflow experiment without changing dependencies
  or permanent tooling.
- [x] Record environment, command prototype, isolated-state fixture, process
  cleanup, observed browser/accessibility surfaces, runtime, reachable negative
  failure, and comparison with existing Vitest/release smoke tests.
- [x] Select existing tooling, one new runner, or `unsupported`; if a new runner
  or different write set is selected, revise Milestone 5 before implementation.

**Acceptance gate:** Reviewed admission report meets the stopping condition and
names one deciding environment/oracle or a typed blocker.

**Status:** `Accepted`

### Milestone 2: Establish Modal And Popup Interaction Modules

**Goal:** Centralize repeated overlay lifecycle policy and migrate the audited
modal and popup consumers without changing feature-domain content.

**Allowed write set:**

- `frontend/src/components/ui/ModalDialog.tsx`
- `frontend/src/components/ui/ModalDialog.test.tsx`
- `frontend/src/components/ui/OverlayEscapeStack.ts`
- `frontend/src/components/ui/Popover.tsx`
- `frontend/src/components/ui/Popover.test.tsx`
- `frontend/src/components/ui/index.ts`
- `frontend/src/components/ConfirmationDialog.tsx`
- `frontend/src/components/ConfirmationDialog.test.tsx`
- `frontend/src/components/InstallDialogFrame.tsx`
- `frontend/src/components/InstallDialogFrame.test.tsx`
- `frontend/src/components/ModelMetadataModalFrame.tsx`
- `frontend/src/components/ModelMetadataModalFrame.test.tsx`
- `frontend/src/components/ModelServeDialog.tsx`
- `frontend/src/components/ModelServeDialog.test.tsx`
- `frontend/src/components/model-serve/ModelServeDialogContent.tsx`
- `frontend/src/components/model-serve/useDialogFocusTrap.ts`
- `frontend/src/components/ModelImportDialog.tsx`
- `frontend/src/components/ModelImportDialog.test.tsx`
- `frontend/src/components/HuggingFaceAuthDialog.tsx`
- `frontend/src/components/HuggingFaceAuthDialog.test.tsx`
- `frontend/src/components/VersionSelector.tsx`
- `frontend/src/components/VersionSelector.test.tsx`
- `frontend/src/components/VersionSelectorTrigger.tsx`
- `frontend/src/components/VersionSelectorTrigger.test.tsx`
- `frontend/src/components/VersionSelectorDropdown.tsx`
- `frontend/src/components/VersionSelectorDropdown.test.tsx`
- `frontend/src/components/ModelSearchBar.tsx`
- `frontend/src/components/ModelSearchBar.test.tsx`
- `frontend/src/components/RemoteModelDownloadMenu.tsx`
- `frontend/src/components/RemoteModelDownloadMenu.test.tsx`
- `frontend/src/components/RemoteModelListItemActions.tsx`
- `frontend/src/components/RemoteModelListItemActions.test.tsx`
- `reports/frontend-overlay-consumer-inventory.md`
- this plan, ledger, and issues files

**Tasks:**

- [x] Record the bounded overlay consumer matrix, actual interaction semantics,
  and migrate/already-safe/delete/follow-up disposition.
- [x] Implement `ModalDialog` and migrate one representative nested-capable
  consumer; prove name, entry, containment, Escape/backdrop, cleanup, and focus
  restoration through the Interface before migrating matching dialogs.
- [x] Implement `Popover`, select feature-correct semantics, and prove trigger
  relationship/state, keyboard/pointer dismissal, focus entry/return, and
  cleanup before migrating matching popups.
- [x] Delete superseded focus lifecycle machinery after its last consumer
  migrates; do not retain compatibility wrappers without a consumer.
- [x] Run representative focus/accessibility workflows selected in Milestone 1.

**Acceptance gate:** FE-A4 with focused component tests and representative
runtime evidence; every inventoried consumer has a disposition.

**Status:** `Accepted`

### Milestone 3: Make Progress, Outcomes, And Motion Perceivable

**Goal:** Expose dynamic state programmatically and make the documented motion
preference true without adding an unsupported general accessibility claim.

**Allowed write set:**

- `frontend/src/components/ProgressDetailsView.tsx`
- `frontend/src/components/ProgressDetailsView.test.tsx`
- `frontend/src/components/ui/Popover.tsx`
- `frontend/src/components/ui/Popover.test.tsx`
- `frontend/src/components/ModelMetadataModal.test.tsx`
- `frontend/src/components/InstallDialog.tsx`
- `frontend/src/components/InstallDialog.test.tsx`
- `frontend/src/hooks/useInstallationManager.ts`
- `frontend/src/hooks/useInstallationManager.test.ts`
- `frontend/src/hooks/useInstallationState.ts`
- `frontend/src/hooks/useInstallationState.test.ts`
- `frontend/src/index.tsx`
- `frontend/src/index.css`
- `frontend/README.md`
- this plan, ledger, and issues files

**Slice sequence:**

- M3-S1: `ProgressDetailsView.tsx` and its focused test only.
- M3-S2: composition-root and CSS reduced-motion policy only.
- M3-S3a: program-approved FE-I13 repair through only the existing
  installation manager, state, and dialog source/tests, including the PRG-I12
  requested-install admission-order regression that reopened FE-A3.
- M3-S3b: representative normal/reduced runtime evidence and affected README
  behavior claims, after M3-S3a acceptance and serialized documentation
  ownership confirmation.

**M3-S3b re-plan gate:** Real Chromium proved the CSS policy but found that
`MotionConfig reducedMotion="user"` still lets the Popover's initial
`translateY(-6px)` persist into nonzero-opacity frames before snapping to its
resting position. Program review admitted only `Popover.tsx` and its colocated
test: use the operating-system preference to select zero entry and exit
translation under reduce while retaining `-6px` in normal mode and preserving
opacity behavior. The deciding rerun must record positive entry and dismissal
samples in all four default/library-only × normal/reduced scenarios and reject
any visible reduced transform. The root also admitted a test-only M2 follow-up
in `ModelMetadataModal.test.tsx`: replace its deleted backdrop-label query with
the actual backdrop/document interactions and await initial async state without
act warnings. The full frontend suite must be green before README or FE-A5
acceptance.

**M3-S3 re-plan gate:** Runtime admission found that no production caller sets
the progress view to `details`, successful terminal progress is cleared by the
manager, and the dialog clears its presentation tag on any terminal payload.
Resolve `FE-I13` through the existing installation owner chain with the exact
M3-S3a files and focused behavior evidence before representative runtime
resumes. Integration review additionally found PRG-I12: a requested lifecycle
could poll before backend admission and consume a prior terminal payload.
M3-S3a included the deferred-admission regression and the accepted repair that
re-satisfied FE-A3/M0. The library-only entry has no installation feature by
design; installation-status
evidence is not applicable there, while motion-policy evidence still covers
both real entries.

**Tasks:**

- [x] Give determinate progress a programmatic name/value and announce terminal
  success/failure at the appropriate politeness without duplicate updates.
- [x] Apply the operating-system reduced-motion preference to Framer Motion at
  the composition root and to nonessential CSS animation/transition behavior.
- [x] Verify normal and reduced modes in the representative runtime.
- [x] Update only the affected behavior claims in the frontend README; leave
  gate/count policy cleanup to governance.

**Acceptance gate:** FE-A5 and the applicable portion of FE-A7.

**Status:** `Accepted`

### Cross-Plan Slice XR-S1: Gate Renderer On Launcher-Root Recovery

**Goal:** Integrate the accepted desktop launcher-root startup and selection
contract without mounting backend consumers against an unresolved root or
making the normal ready path feel slower.

**Dependency gate:** The platform owner accepted and froze the closed,
path-free startup/selection unions, main-process single-flight policy, and
sandbox-compatible preload decoder before this consumer was integrated.

**Allowed write set:**

- `frontend/src/types/api-window.ts`
- `frontend/src/types/api-bridge-utilities.ts`
- `frontend/src/hooks/useLauncherRootRecovery.tsx`
- `frontend/src/hooks/useLauncherRootRecovery.test.tsx`
- `frontend/src/hooks/useAppWindowActions.ts`
- `frontend/src/hooks/useAppWindowActions.test.ts`
- `frontend/src/components/LauncherRootRecoveryView.tsx`
- `frontend/src/components/LauncherRootRecoveryView.test.tsx`
- `frontend/src/index.tsx`
- `reports/launcher-root-recovery-consumer-evidence.md`
- this plan, ledger, and issues files

This source boundary was integrated in `2b081fba`, including the synchronous
decoded bootstrap and root-scoped display cache. The former uncommitted-source
hold is superseded. Full XR-S1 acceptance still requires its complete composed
startup/selection and two-entry first-visible-frame evidence.

**Tasks:**

- [x] Replace the legacy optional-field selection response with the exact
  startup and selection unions exposed by the decoded preload Interface.
- [x] Add one composition-root provider that treats browser mode as not
  applicable, gates Electron children until ready, serializes startup/selection
  attempts, keeps initializing polling sequential, and cleans up timers and
  late outcomes; the main process owns the only visibility deadline.
- [x] Present checking, explicit-authority guidance, retryable unchanged,
  ambiguous/published terminal, restarting, and bridge-unavailable states with
  accessible status/actions and frameless window controls.
- [x] Delegate the existing Change Library action to the provider and remove
  the legacy direct bridge/logging path.
- [x] Prove StrictMode ownership, every state transition, action availability,
  cancellation restoration, focus, path non-disclosure, both renderer aliases,
  and absence of legacy response fields through public seams.
- [x] Run both production entries with the actual sandboxed preload and prove
  that a fast ready response can reach a model-list content frame; this sample
  exposed that renderer-only timing cannot prove the first visible frame.
- [x] Add the renderer half of the construction-safe visibility handshake:
  never acknowledge initializing, terminal-latch the platform watchdog, and
  acknowledge ready, recovery, or unavailable exactly once from the terminal
  layout commit. This is semantic notification only; it makes no compositor-
  presentation claim.
- [ ] Prove all closed producer startup and selection values through the actual
  compiled preload and frontend semantics, reject malformed/extra payloads,
  and run the complete first-visible ready/recovery/unavailable/no-preload
  matrix through the accepted main-owned marker barrier. Accepted Linux
  cold/warm library runs and sandbox preload conformance corroborate this
  claim but do not alone cover every state in both entries.

**Acceptance gate:** FE-A8, the accepted platform producer/decoder evidence,
focused and full frontend gates, both production builds, and the representative
two-entry frame oracle.

**Status:** `Verifying`

### Milestone 4: Adopt Decoded Contracts And Honest Model Projection

**Goal:** Consume the provider-owned decoded operation Interface and retain
instant model display with explicit provenance, degradation, and recovery.

**Dependency gate:** Selected catalog/search/download/ticket contracts and
their generated decoder are accepted in `2b081fba`. Remaining operations require
their own coordinated provider handoff, not a whole-milestone source hold.

**Remaining-consumer gate:** Classify the link-health refresh rejection
(`FE-I10`) and model-import picker rejection (`FE-I11`) against this
milestone's decoded bridge/projection Seam. When an outcome shares that owner,
add its exact consumers and behavior evidence to this milestone before source
edits. Otherwise add one exact focused slice with its own write set and gate;
neither finding may be deferred outside this program.

**Allowed write set:**

- `frontend/src/api/adapter.ts`
- `frontend/src/api/adapter.test.ts`
- `frontend/src/api/import.ts`
- `frontend/src/api/models.ts`
- `frontend/src/api/versions.ts`
- `frontend/src/utils/modelLibrarySnapshot.ts`
- `frontend/src/utils/modelLibrarySnapshot.test.ts`
- `frontend/src/hooks/useModels.ts`
- `frontend/src/hooks/useModels.test.ts`
- `frontend/src/App.tsx`
- `frontend/src/components/LibraryOnlyApp.tsx`
- `frontend/src/components/AppShellState.ts`
- `frontend/src/components/AppShellState.test.ts`
- `frontend/src/components/ModelManager.tsx`
- `frontend/src/components/ModelLibraryProjectionNotice.tsx`
- `frontend/src/components/ModelLibraryProjectionNotice.test.tsx`
- `frontend/src/components/ModelManagerIntegrityRefresh.test.tsx`
- `reports/renderer-contract-consumer-inventory.md`
- `frontend/README.md`
- this plan, ledger, and issues files

**Tasks:**

- [ ] Inventory every renderer API consumer and record decoded/already-safe,
  migrate, delete, or external-owner disposition against the accepted Interface.
- [ ] Remove assertions or fallback substitution that let invalid/unavailable
  results masquerade as domain values; retain wrappers only where the deletion
  test shows real renderer-domain composition or recovery.
- [x] Replace version-1 storage with a closed version-2 root-scoped display
  projection; retain honest age semantics and omit all action authority.
- [x] Expose one typed projection outcome from `useModels` and render cached,
  degraded, retrying, and fresh state without hiding the cached list.
- [x] On successful authoritative refresh, atomically replace the projection;
  on failure, preserve visible cached data and expose recovery.
- [x] Test valid/retired/malformed storage, unavailable/invalid/failure results,
  stale completion, retry, and fresh replacement through the Module Interface.
- [ ] Run the representative immediate-startup and degradation workflow.

**Acceptance gate:** FE-A1, FE-A2, and the applicable portion of FE-A7. Link
provider contract/decoder evidence without claiming ownership of it.

**Status:** `Active`; selected consumer checkpoint accepted, remaining operation
inventory/migration and complete FE-A1/FE-A2/FE-A7 evidence pending. See the
[accepted checkpoint](execution-ledger.md#2026-09-05--selected-desktop-consumer-checkpoint-accepted).

### Milestone 5: Prove Default And Library-Only Renderer Behavior

**Goal:** Add the admitted package-local representative command and prove both
supported built-renderer workflows without claiming packaged release contents.

**Dependency gate:** Milestone 1 harness decision and integrated governance
changes to shared package scripts.

**Allowed write set:**

- `frontend/package.json`
- `frontend/tests/renderer/run.mjs`
- `frontend/tests/renderer/electron-main.cjs`
- `frontend/tests/renderer/fixtures.cjs`
- `frontend/README.md`
- this plan, ledger, and issues files

**Tasks:**

- [ ] Revise this exact write set first if the admission report selects a
  different bounded location or no capable tool.
- [ ] Add one package-local representative command with deterministic external
  operation fixtures, isolated profile/state, and process cleanup.
- [ ] Build and launch the default and library-only modes through their real
  entry points.
- [ ] Prove plugin UI presence in default mode, its absence in library-only
  mode, and a core model-library workflow in both.
- [ ] Run FE-A2, FE-A4, and FE-A5 workflows in the same harness and document its
  unique claim and schedule for governance consumption.
- [ ] Keep bundle/package inspection in the platform plan; minified string
  absence is not the renderer behavior oracle.

**Acceptance gate:** FE-A2, FE-A4, FE-A5, FE-A6, and FE-A7 with a passing
representative command in both build modes.

**Status:** `Planned`

### Milestone 6: Frontend Acceptance Review

**Goal:** Reconcile all renderer claims, reports, and evidence, then hand off
remaining producer, packaged-artifact, and governance evidence without
overclaiming frontend acceptance.

**Allowed write set:**

- this plan, ledger, issues, and reports

**Tasks:**

- [ ] Run affected frontend lint, typecheck, focused unit/integration tests,
  both production builds, and the admitted representative command.
- [ ] Review each objective row against linked evidence and its stated
  environment; lower-fidelity results remain corroboration only.
- [ ] Confirm each systemic consumer inventory has a disposition and each
  retained permanent test has a unique claim and schedule owner.
- [ ] Link unresolved producer, packaged-artifact, CI, platform, or broader
  accessibility claims to their sibling owner rather than broadening this plan.

**Acceptance gate:** FE-A1 through FE-A7 are satisfied in their stated
environments, or the plan remains non-accepted with explicit issues.

**Status:** `Planned`

## Blockers

- No source blocker remains for the accepted catalog/recovery/cache/startup
  boundary. Remaining link-health/import/conversion contracts require the
  bounded provider/consumer handoff described above.
- Complete XR-S1/M5 and whole M4 evidence remains outstanding; existing
  Electron/CDP admission is available for the selected Linux workflow.
- Windows/macOS runtime evidence is unavailable locally and remains with the
  platform owner. It does not block independent Linux source work or imply
  cross-platform acceptance.

## Re-Plan Triggers

- The harness experiment cannot observe the required browser accessibility,
  focus, media, or build-mode behavior with deterministic cleanup.
- Contract providers expose a materially different consumer Interface or omit
  an audited invalid/unavailable outcome.
- Consumer inventories find another active owner with the same invariant or a
  required file outside a milestone's exact write set.
- A popup's real interaction requires a different semantic/lifecycle Module
  than the proposed `Popover` Interface.
- Snapshot compatibility or product policy requires a producer revision or
  retention period not available in current contracts.
- Governance or platform integration changes shared package/build files before
  the dependent milestone starts.
- A proposed Module fails the deletion test, merely forwards calls, or forces
  unrelated callers to learn hidden transport/storage/focus details.
- Representative runtime evidence contradicts jsdom/unit behavior.

## Final Acceptance

- Acceptance status: `partial`
- Deferred follow-ups: Rust schema/error production, Electron/generated decoder
  production, packaged-artifact verification, CI scheduling, and any broader
  accessibility conformance remain with their named sibling owners.
- Final status: `Active`
