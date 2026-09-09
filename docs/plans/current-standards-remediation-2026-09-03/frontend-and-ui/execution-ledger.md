# Execution Ledger: Frontend and UI Standards Remediation

## 2026-09-08 — Native Setup Source And Build Coherence

Accepted NBUILD. Read-only inspection found that llama.cpp setup pulled source
before deciding whether to build, but skipped CMake when both old binaries were
usable. The setup recipe now owns configure/clean-build of both targets for
every admitted operation, followed by artifact and import verification. It
explicitly selects CUDA ON/OFF to avoid inheriting a prior cached ON selection.
The previous healthy-output skip policy from NATIVE is superseded; guarded
output cleaning, retained command custody and managed root exclusion remain.

Root and root_capability bounded the decision before implementation. A HEAD
receipt would omit working-tree edits, configuration and toolchain facts and
would add persistence/read/execution obligations without adequate cache
authority. The codebase-design skill therefore kept build policy in the existing
recipe, with no new module, cache, runtime or public interface. Rebuilding is
owned by explicit setup admission, not every status read or retained receipt
observation. It can cost more than the old shortcut but cannot treat an old
usable binary as evidence of rebuilding updated source.

This is a setup-success guarantee under stable paths and cooperating tools,
not persistent provenance after failed/cancelled setup or external changes.
Normal unsuccessful optional pull still warns and builds local source; no latest
upstream, pinned revision or reproducibility claim. FE-I26 retains interrupted
setup/external mutation invalidation, independent probe/direct-use coordination
and real tool/hardware prerequisites before new GUI mutations.

root_diagnostics owns recipe and public setup fixtures; root owns docs, serial
Cargo/formatting and acceptance. root_capability supplied read-only design and
production/docs review with no blockers. Three new public setup tests cover
updated/unchanged source with usable outputs, configure/build failure followed
by retry, and dropped build/shutdown waiters with observed child/group cleanup
before Cancelled. The existing artifact matrix now requires rebuilding healthy
outputs too; the occupied-symlink case refuses cleaning even when the other tool
is healthy. The shared fixture writer uses an awaited child to avoid writable
executable descriptors inherited by parallel test forks. It does not introduce
production spawn retries.

Focused conversion tests passed 113/113 using `cargo test --offline --locked
-p pumas-library --no-default-features conversion::` from `rust/`.
Full core/RPC suites passed: 1,479 default and 1,439 minimal tests, each with
22 ignored and no failures. Both strict lint configurations, formatting and all
five canonical plan checks passed. The existing unrelated FE-I28 download timeout
remains open; these passing runs do not establish its cause or fix.
Full tests: `cargo test --offline --locked -p pumas-library -p pumas-rpc`, with
and without `--no-default-features`. Strict lint: `cargo clippy --offline --locked
-p pumas-library -p pumas-rpc --all-targets` with `--all-features` or
`--no-default-features`, each followed by `-- -D warnings`.
Formatting: `cargo fmt --all --check`.
Logs: `/tmp/pumas-native-build-*.log`. No actual package installation, upstream clone/build,
model/GPU, live library, graphical workflow or Windows/macOS evidence is claimed.
Program acceptance remains partial. Next slice: native setup failure/interruption
invalidation before managed use, while later external mutation provenance remains
separate outstanding work.

## 2026-09-08 — Retained Base-Format Readiness

Accepted BPROBE. Base Python readiness now uses the same retained ProbeOwner
mechanism as quantization readiness. Overlapping reads share active work, later
reads refresh and aggregate setup shutdown closes/drains the base probe along
with every setup/quantization probe owner before runtime shutdown. The raw child
poll/kill loop and per-request detached blocking path were removed. The existing
synchronous boolean remains caller-owned and conservative on error.

The unchanged base import specification is owned once in readiness. Base setup
uses it through the existing synchronous import runner inside its retained
blocking installer, with setup cancellation and its own named five-second
budget. It does not enter the public ProbeOwner or occupy a nested blocking-pool
slot. Shared runner cleanup now applies during setup probes as well as public
reads; cancellation cannot proceed to pip after a held import check.

Missing-command/ENOENT and normal nonzero import exits remain false. Signal,
deadline, other spawn and cleanup failures now remain `ConversionFailed`, rather
than false or an internal I/O error; closed/cancelled async reads return
`ConversionCancelled`. The existing RPC envelope projects redacted operation
failure (`-32003`) or cancellation (`-32004`), without changing method names,
schemas or successful boolean payloads. This is an intentional classification
correction, not compatibility fallback. Reads still do not install or acquire
setup exclusion; independent setup/read coordination remains caller-owned.
Cleanup can outlive the command budget. No new GUI or runtime dependency.

The codebase-design skill kept lifecycle knowledge in the existing core owners.
root_diagnostics implemented core; root_capability added the RPC dispatch
regression; root integrated public docs, named the unchanged base setup budget,
reviewed consumers and serialized formatting/Cargo. Independent core review by
root_capability found no blockers in retained ownership, close-before-await
shutdown, direct setup probing or failure classification. Three new public manager
fixtures cover overlap/dropped readers and fresh imports; base read/setup held
imports with dropped shutdown observation and all-owner closure on a one-thread
blocking pool; and signal-failure retention. Existing timeout/spawn assertions
now require explicit errors; Unix timeout coverage is retained. Linux fixtures
also check foreground reaping, group quiescence and absence of pip continuation.
Controlled shell interpreters do not execute real package imports or installs.

Focused conversion tests passed 110/110, and actual RPC dispatch passed missing,
normal nonzero, invalid executable, directory, signal and closed cases with
exact correlated/redacted envelopes and no setup effects. Read-only inspection
confirmed the optional conversion workflow catches readiness rejection and
blocks mutation while its error is present; its existing hook/dialog tests
passed 21/21. No GUI source or generated artifact changed, and no graphical
workflow acceptance is inferred from these tests. Full core/RPC suites passed:
1,476 default and 1,436 minimal tests, with 22 ignored in each configuration.
Strict all-feature and minimal lint, workspace formatting and all five canonical
plan checks passed. The existing unrelated FE-I28 download timeout remains open;
these successful runs do not establish its cause or fix.

Commands from `rust/`: focused `cargo test --offline --locked -p pumas-library
--no-default-features conversion::`, RPC `cargo test --offline --locked
-p pumas-rpc --no-default-features base_readiness_rpc`. Full tests use
`cargo test --offline --locked -p pumas-library -p pumas-rpc` with and without
`--no-default-features`. Strict `cargo clippy --offline --locked -p pumas-library
-p pumas-rpc --all-targets` uses `--all-features` and `--no-default-features`,
each with `-- -D warnings`; formatting uses `cargo fmt --all --check`.
UI supporting check: `pnpm --dir frontend exec vitest run
src/hooks/useModelConversionWorkflow.test.ts src/components/ModelConversionDialog.test.tsx`.
Logs: `/tmp/pumas-base-readiness-*.log`. No real model/package/GPU, live library,
release-artifact or Windows/macOS execution claim. Program acceptance stays partial.
Next slice: native source/build revision coherence, before new quantization GUI
mutations; independent probe/setup coordination remains outside this acceptance.

## 2026-09-08 — Managed Setup And Conversion Exclusion

Accepted EXCL. Every managed direction now enters through WorkerOwner's
environment wrapper, acquiring the existing exclusive physical-root setup lease
before execution and retaining it through cleanup, output publication and
indexing. The raw worker spawn is private, so production manager dispatch cannot
bypass the wrapper. Setup and conversion retain their existing distinct
operation identities, cancellation and terminal owners. Contention fails the
admitted operation explicitly, with no queue or automatic retry. Other managed
conversions at the same root are excluded too because base conversions deploy
shared scripts; independent roots remain independent.

The codebase-design skill kept coordination inside core. The helper uses brief
retained blocking acquisition/release calls and carries only an owned File
during asynchronous execution. It never parks a blocking-pool worker for the
whole conversion or runs the setup lease's retrying destructor on the async
thread. Work failure/panic and release failure remain observable before terminal
progress. Caller/runtime shutdown obligations are unchanged. Independent probes,
direct backend execution, external tools, hostile root/lock replacement and
abrupt runtime/process destruction are excluded. The controlled panic test has
no child process; it is not proof of arbitrary native-panic quiescence.

root_diagnostics implemented the bounded conversion source/tests; root reviewed,
integrated facade/README documentation and serialized formatting/Cargo.
root_capability independently reviewed the frozen composition and found no
blockers within its documented custody, failure and exclusion boundaries. Five
new regressions plus the extended existing quiet-native fixture cover all six
public manager directions under contention, unchanged source/index/output state,
both contention directions, independent roots, symlink aliases and independent
processes, cancellation with dropped shutdown observation, observed release for
success/error/no-child panic/cancellation, and async filesystem work on a
current-thread runtime with only one blocking thread. The actual quiet-native
fixture also checks setup exclusion while the child is alive and reacquisition
after child cleanup/drain. No real conversion or installation tools are invoked.

Initial focused evidence found two fixture problems. The new no-output test
incorrectly assumed one library entry; it now compares the exact pre/post entry
set, preserving source bytes, index identity and no-script/output assertions.
An existing manager readiness timeout fixture failed before execution with
`ETXTBSY`; the same binary passed that test in isolation. Its helper opened the
executable for writing inside the parallel test process, permitting another
test's fork to inherit the writer. Root reused the existing awaited shell-writer
pattern, keeping the writable descriptor outside that process. The original
transient fork was not traced; this removes that inheritance path by construction,
not by widening deadlines, retrying spawn or serializing the suite. Production
readiness behavior is unchanged. Corrected focused conversion evidence passed
107/107. Default core/RPC evidence passed 1,472 tests, with 22 ignored.
The first minimal run hit the unchanged download fixture's one-second admitted-ID
wait in `pause_after_a_worker_check_is_settled_by_the_same_generation`. The same
binary passed that exact test in isolation in 0.15 seconds; default also passed
it. This is FE-I28, not an environment-exclusion failure or a diagnosed download
fix. Preserve the original `/tmp/pumas-managed-exclusion-minimal.log`; the normal
parallel minimal recheck passed 1,432 tests with 22 ignored. Strict all-target
all-feature and no-default clippy, formatting, whitespace and all five plan
contracts passed. No download source, deadline or test parallelism was changed.
The passing recheck does not resolve FE-I28 or prove its timeout cause.

Commands from `rust/`: focused `cargo test --offline --locked -p pumas-library
--no-default-features conversion::`; full `cargo test --offline --locked
-p pumas-library -p pumas-rpc` with and without `--no-default-features`.
Strict `cargo clippy --offline --locked -p pumas-library -p pumas-rpc
--all-targets` uses `--all-features` and `--no-default-features`, each with
`-- -D warnings`; formatting uses `cargo fmt --all --check`.
Evidence logs: `/tmp/pumas-managed-exclusion-*.log`. No GUI, real package/model/GPU,
release-artifact or Windows/macOS claim; core remains standalone and GUI-optional.
Full-program acceptance remains partial; base-format readiness probing is next.

## 2026-09-08 — Backend Setup RPC And Desktop Projection

Accepted BRPC. Added `start_backend_setup` and `get_backend_setup` to the
closed Rust command decoder/dispatch and optional desktop bridge. Request
schemas derive exact snake_case backend variants from QuantBackend and reuse
the canonical UUID constraint; unknown fields, aliases and malformed values
fail before dispatch. The accepted core owner retains setup, retry CAS and
shutdown; these adapters do not create state, retry automatically or fall back
to blocking setup. Existing base commands are unchanged and the new commands
remain available without inference plugins. Rust library use needs no GUI/RPC.

Responses reuse the validated/redacted setup started/status constructors.
`success: true` acknowledges the observation, not successful installation.
Request correlation carries backend selection; callers keep it alongside the
latest-only snapshot. Core/wire/desktop documentation preserves caller-owned
setup/execution exclusion, readable records after shutdown, and no readiness
inference from idle or completed setup.

The codebase-design skill kept lifecycle semantics in core and reused the
existing generated adapters. Rust params/schema/dispatch/test changes were
delegated to root_diagnostics; root integrated the IPC/preload/typed consumer
surface, regenerated all six contract artifacts, reviewed and serialized Cargo.
Review confirmed main IPC forwards the generated decoder's copied value rather
than a validation flag plus the original renderer object. Main and preload
share the same request schemas; no independent enum/token validator or generator
vocabulary was introduced. Build trust/dialect remain the existing offline,
locked Rust exporter and AJV Draft7 pipeline. Generated outputs were regenerated
and checked for freshness, never edited independently.

Evidence: three focused Rust tests passed. The actual handle_rpc fixture
exercises NVFP4 start, terminal read, None attachment, identity-bound successor,
stale-token attachment and shutdown using a probe-only shell interpreter; no
package installation/import occurs. All four backends also have dispatch idle,
invalid/obsolete-token and closed-admission checks. The real Rust parser produces
37 request probes consumed by generated decoder conformance. Producer snapshots
for every state cross the bundled preload to typed frontend callers for all four
backends. Main IPC and compiled preload reject malformed requests/responses;
unknown-method rejection never falls back or retries.

Generator tests passed 6/6, producer/decoder conformance 13/13, typed renderer
contract tests 13/13, and full desktop tests 135 passed with one environment-gated
real-Electron oracle skipped. Desktop/frontend lint and types passed, as did both
frontend builds; the default output was rebuilt after library-only verification.
No graphical workflow or OS-sandbox/runtime acceptance is inferred from VM-based
preload or jsdom contract tests. No real tools/models/GPU, release, live library
mutation, network installation or Windows/macOS execution claim.

Final core/RPC suites passed 1,467 default and 1,427 no-default tests (22 ignored
in each), with strict all-target/all-feature and all-target/no-default clippy,
cargo fmt, whitespace and all five canonical plan contracts passing. Logs use
`/tmp/pumas-backend-rpc-*.log`; focused Cargo uses `-p pumas-rpc
--no-default-features backend_setup`, full suites use `-p pumas-library
-p pumas-rpc`, all offline/locked. Full-program acceptance remains partial;
managed setup-versus-conversion exclusion is the next backend prerequisite.

## 2026-09-08 — Backend-Specific Rust Setup Observation

Accepted BSETUP. Standalone PumasApi/ConversionManager callers can select
all four built-in setup environments for start, memory-only status and guarded
retry. Selection uses the existing owner's backend identity; PythonConversion
selects the existing base owner. Setup snapshots, CAS retry admission, ensure
semantics, retained worker custody and aggregate shutdown are unchanged. No
backend trait expansion, dependency, wire shape, GUI or second lifecycle owner.
Rustdoc and the core README own the caller contract, including caller-owned
setup/execution exclusion and process-local observation rather than readiness.
The README's obsolete exclusion of quantization installers from shutdown was
corrected, and transport claims now distinguish base setup from Rust-only
backend observation. RPC/desktop projection remains the next consumer slice.

The codebase-design skill kept state and retry semantics in SetupOwner instead
of adding per-backend observation machinery. Root implemented/wired the public
surface and PumasApi contract check; root_diagnostics added the controlled
manager regression and reviewed production identity/lifecycle/docs. Tests prove
all-backend selection, read/rejected-token non-effects, base alias identity,
attachment to held ensure work, dropped waiter custody, None not retrying,
cross-backend/stale tokens not retrying, duplicate matching retries sharing one
successor, exact controlled install counts and post-shutdown reads/rejection.

Verification deviation: the first full minimal suite failed an existing held
installer test during llama.cpp venv creation with `Observing setup process-group
cleanup: No such process (os error 3)`. The same isolated test passed 20/20;
repetition alone was not accepted as a fix. The diagnosing-bugs skill guided a
deterministic Linux regression: open a live child's procfs stat inode, kill/reap
the test-owned child, reopen the retained inode through `/proc/self/fd`, and
observe raw ESRCH. The real read_stat helper failed its expected-absence assertion
before correction. This reproduces an actual reader failure consistent with the
suite symptom; the original transient process/path was not traced.

The bounded write set expanded to linux_group.rs because this cleanup failure
invalidated the setup acceptance gate. Its stat reader now recognizes ESRCH,
as well as ENOENT, as a disappeared task. It does not ignore permission, parser,
task-enumeration or child-identity errors, weaken group checks, or relax lease
custody. The held-inode regression uses real Linux procfs rather than fabricated
errno; all installer programs remain controlled substitutes. No debug logging
or new runtime owner was added. This is not stronger containment or proof for
escaped processes, unsupported procfs visibility or Windows/macOS.
The independently reviewable cleanup correction is commit `b89d403a`.

Final evidence: 102 focused conversion tests passed, including the deterministic
procfs regression and previously failing installer case. Full default core/RPC
passed 1,464 tests; no-default passed 1,424 tests (22 ignored in each). Strict
all-target/all-feature and all-target/no-default clippy passed with warnings
denied; cargo fmt, whitespace and all five canonical plan checks passed.
Both new backend-observation tests also passed in the initial focused run.
Final runs use `/tmp/pumas-backend-observation-*-final.log`; the deterministic
red is `/tmp/pumas-proc-stat-red.log`. Commands use offline/locked Cargo with
`-p pumas-library -p pumas-rpc`, or `--lib conversion::` for focused evidence.
Linux controlled installer evidence is not real package/native build/GPU
compatibility. No real library, release, GUI, network or installation action.

## 2026-09-08 — Native llama.cpp Setup Artifacts

Accepted NATIVE, the native artifact portion of FE-I26 setup repair.
After git clone/pull, setup rejects an absent, empty or non-file converter before
building or installing Python dependencies. Both llama-quantize and llama-imatrix
must satisfy the existing readiness metadata predicate. An unusable tool triggers
configuration and a clean rebuild of managed CMake outputs; a healthy pair skips
CMake. Both outputs are checked again before Python setup, so a successful build
exit without usable tools cannot produce a completed setup receipt.

The codebase-design skill kept these decisions in the existing retained recipe
and reused the readiness predicate. Agent updated fixtures and public setup
regressions; root implemented the checks, repair decision and public Rustdoc.
The build workflow guided the explicit generated-output scope and invalidation
reason: an empty/nonexecutable artifact can otherwise appear up-to-date. Local
CMake help confirmed the clean-first option; no actual model build was run.

Review identified that cleaning an occupied output might remove unrelated
contents. Before any CMake invocation, both output entries must therefore be
missing or regular files according to symlink_metadata. A directory, symlink or
other unexpected entry refuses repair without being removed. A healthy symlink
may still be used when no rebuild is necessary. Both initial metadata checks
run before deciding to rebuild; inspection errors cannot become missing output.
This assumes stable paths, not protection against hostile concurrent replacement.

Setup keeps the existing checkout/venv and does not reset, reclone or delete an
invalid converter to disguise failure. Generated CMake outputs are disposable
within their managed build directory. The existing checkout/configuration and
compiler discovery remain inputs; source-revision coherence and build-cache
validity are not established by this metadata check. Public setup docs require
caller-owned exclusion of conversions/external tool use; enforcing that exclusion
remains FE-I26. Existing setup lease, cancellation, command cleanup and explicit
retry remain authoritative, with no new owner, public/wire type or GUI change.

Evidence: 100 focused conversion tests and strict lint in both feature
configurations passed. Four new Linux test functions cover both native tools
healthy/missing/empty/nonexecutable, clean-first argv, zero-exit build with invalid
outputs failing before pip/import probes, explicit retry reaching Completed,
bad converter preservation, and occupied native directories/symlinks preserved
before CMake. Retained failures survive repeated composed shutdown. Fake git and
CMake now create meaningful artifacts so existing installer tests remain valid.

Limits: controlled tools prove recipe decisions and receipts, not real CMake
cleanup behavior, native builds, packages, GPUs, ABI or Windows/macOS execution.
No live checkout, model or library was changed. Supporting gates passed: 1,461
default and 1,421 no-default-feature core/RPC tests, with 22 existing ignored tests
each; formatting, whitespace and all five plan contracts. No gate bypass.
Logs: `/tmp/pumas-native-setup-{focused,focused-final,clippy-default,clippy-minimal,default,minimal}.log`.

## 2026-09-08 — Retained Quantization Readiness Probes

Accepted PROBE, the public quantization import-readiness portion of FE-I26.
Built-in summaries now check regular nonempty artifacts (Unix execute bits for
executables) and the same isolated imports as setup. NVFP4/Sherry use embedded
script imports; llama.cpp checks locally declared modules, not every requirement
of an evolving upstream converter. Missing artifacts or normal failed imports
return false. Metadata/spawn/signal/timeout/cleanup failures remain contextual
ConversionFailed errors on async reads; closed/cancelled probes return
ConversionCancelled. Synchronous boolean summaries conservatively return false.

One private ProbeOwner per concrete backend retains a bounded blocking operation.
Concurrent async callers join it; later reads first observe the old worker then
atomically admit a fresh probe, rather than reuse a cached terminal answer.
The worker captures paths/imports only, never its owning backend. Dropped callers
do not release joins or native cleanup. Shutdown closes all setup/probe owners
before its first await, cancels probes, observes all receipts even after errors,
and retains results across interrupted/repeated drain. Missing artifacts still
use the retained inspection worker but do not spawn an interpreter. Probe reads
do not acquire install leases, produce setup snapshots or install dependencies.

The codebase-design skill kept this read lifecycle distinct from installer
mutation identity, while reusing the existing command runner and import recipe
authority. Agent implemented probe ownership/tests and reviewed integration;
root wired concrete backends, manager/API shutdown, caller fixtures and manager
contract tests. Manager async status/catalog no longer spawn unretained wrapper
tasks. The additive trait is_ready_async default explicitly reports unavailable;
downstream implementations remain source-compatible without a blocking fallback.
Built-ins override it. Python-dependent execution awaits it before staging;
GGUF-only llama.cpp skips Python. Public/wire data shapes and features are unchanged.

Each interactive import command has a five-second execution budget; a sequential
status/catalog read may probe three backends. Fail-closed cleanup is not bounded
by that budget. Setup keeps its separate thirty-second import budget. Conversion
cancellation is checked before/after the shared read, not used to cancel other
readers' work; probe execution/cleanup errors still propagate. Blocking summaries
are caller-owned, need no runtime, and must finish before shutdown. Async owner
capacity does not govern independent synchronous invocations. No stronger sync
shutdown or cancellation guarantee is claimed.

Evidence: 96 focused conversion tests passed.
Controlled Linux fixtures prove fresh import results, shared active work after
waiter drop, runtime responsiveness, held-probe shutdown/closure/reaping,
five-second timeout plus repeated failure observation, and synthetic lost/join
panic receipt retention. A downstream-style trait implementation proves the new
default does not invoke synchronous readiness. Actual manager tests prove catalog
and status follow each backend's imports without setup records, and aggregate
shutdown closes all matching setup/probe owners while three probes are active.
Existing publication/progress fixtures now distinguish probes from model steps.

Limits: no real packages, model tools, GPU/ABI compatibility, GUI workflow or
Windows/macOS execution. Base-format probe modernization, setup-versus-conversion
exclusion, native setup repair, hardware, calibration/source-file custody and
backend-specific observable setup remain open. Supporting gates passed: 1,457
default and 1,417 no-default-feature core/RPC tests, with 22 existing ignored tests
each; strict all-target clippy in both configurations; formatting, whitespace
and all five plan contracts. No gate bypass or live-library mutation occurred.
Logs: `/tmp/pumas-public-probes-{focused,focused-final,clippy-default,clippy-minimal,default,minimal}.log`.

## 2026-09-08 — Direction-Specific llama.cpp Artifacts

Accepted ART, a bounded FE-I26 artifact prerequisite. llama.cpp execution
now checks tools for the route it actually selects before staging or spawning:
all routes need llama-quantize, safetensors-only needs converter and Python,
and IQ/forced imatrix needs llama-imatrix. Existing mixed-source GGUF preference
is unchanged. GGUF requantization no longer requires unrelated Python artifacts.
Aggregate is_ready remains advisory for the basic safetensors route and now
includes Python; has_imatrix checks the optional tool independently.

Required artifacts must be regular, nonempty files. Unix executable artifacts
must have execute bits; the converter script need not. These are metadata checks,
not effective-user/ACL/noexec, loader/ABI, import or hardware proofs. Execution
uses async metadata, preserves inspection I/O errors, and rejects known missing
or invalid artifacts as QuantizationEnvNotReady. The existing boolean summaries
remain conservative on inspection failure. Symlinks are followed, not retained;
callers must keep artifacts stable through execution. No new probe subprocess,
installer, cache, lifetime owner, public type, feature or wire change is added.

The codebase-design skill kept the common metadata rule private to llama.cpp,
with route requirements beside pipeline selection rather than in API/UI callers.
Agent supplied public-backend fixtures and review; root implemented the source,
added inspection-error evidence and integrated verification. Public Rustdoc now
distinguishes advisory readiness from route-specific execution requirements.
Setup completion does not promise native artifact or future readiness validity.

Evidence: 87 focused conversion tests passed.
Four new controlled Linux tests prove GGUF/mixed-source execution without Python,
missing converter/Python/imatrix rejection before staging/spawn, a successful
safetensors-plus-imatrix route, invalid aggregate artifacts, and a symlink-loop
inspection error retained as I/O failure before staging. The existing manager
publication fixture also runs GGUF without dummy Python/converter artifacts.
Tiny shell executables produce fixture bytes, not real model conversions.

Verification incident: the first focused run passed 85 tests but failed the
existing import-probe timeout assertion. After improving its diagnostic, isolated
and focused reruns passed, but the full default suite reproduced a spawn failure:
`Text file busy (os error 26)`, before timeout behavior was exercised. The
diagnosing-bugs skill separated this fixture failure from artifact correctness.
The fixture now writes/chmods its script in an awaited single-threaded shell,
passing path/content as positional arguments. Concurrent Rust test forks cannot
inherit its writable script descriptor. The originating fork was not traced;
this removes that fixture-lifetime hazard, not a production ETXTBSY recovery
policy. No production retry, timeout, cleanup or assertion meaning is weakened.
Logs: `/tmp/pumas-route-artifacts-{focused,probe-isolated,focused-rerun,default,focused-final}.log`.

Limits: public import-probe lifetime/readiness, native setup repair, hardware,
calibration/source-file validation and artifact custody remain FE-I26. No GUI,
live installations, real dependency compatibility or Windows/macOS execution
claim is made. Supporting gates passed: 1,448 default and 1,408 minimal-feature
core/RPC tests, with 22 existing ignored tests each; strict all-target clippy in
both configurations; formatting, whitespace and all five plan contracts. The
corrected timeout fixture passed in both complete runs. Final gate logs:
`/tmp/pumas-route-artifacts-{default-final,minimal,clippy-default,clippy-minimal}.log`.

## 2026-09-08 — Quantization Setup Import Verification

Accepted IMP, the setup dependency-verification/repair portion of FE-I26.
All three retained quantization recipes probe required imports before skipping
pip and after installation. Existing interpreters are reused, never deleted or
recreated as repair. A healthy environment skips dependency installation;
pip success alone cannot complete setup. NVFP4/Sherry probe embedded-script
imports; llama.cpp probes locally declared dependencies, not the full evolving
requirements of an unvendored upstream converter.

The codebase-design skill kept this sequence in the shared private recipe helper
and existing retained command runner, with no new owner or public configuration.
Probes use isolated Python with bytecode writes disabled and a thirty-second
setup-only budget. Normal nonzero exit or missing interpreter permits repair;
cancellation, signal termination, timeout, spawn/observation and cleanup failure
cannot silently become missing dependencies. A review caught signal termination
being classified as repairable; the corrected code and regression passed before
acceptance. Public is_ready checks remain unchanged and require a separate
nonblocking design; setup probes are not interactive readiness checks.

Evidence: 83 focused conversion tests passed. Controlled Linux fixtures traverse
all three actual recipes with existing healthy and incomplete environments,
prove no interpreter recreation, dependency-loss repair and healthy retry
without another pip invocation. A successful pip fixture with failed imports
retains Failed; explicit repair/retry reaches Completed. Colocated probe tests
check actual isolated argv, missing interpreter, nonzero exit, signal termination,
permission failure and child reaping before timeout/cancellation returns.
No real Python packages, model code, installers or GPU work ran.

Supporting gates passed: 1,444 default and 1,404 no-default-feature core/RPC
tests, with 22 existing ignored tests each; strict all-target clippy in both
feature configurations; formatting, whitespace and all five plan contracts.
Logs: `/tmp/pumas-setup-imports-{focused,default,minimal,clippy-default,clippy-minimal}.log`.

Limits: no public/wire types, feature gates or GUI changes, and no Windows/macOS
execution or real dependency compatibility claim. Public direction-specific
readiness, native artifacts/hardware, backend setup observation/retry,
setup-versus-conversion exclusion and the remaining FE-I26 prerequisites stay
open before quantization GUI configuration.

## 2026-09-08 — Quantization Installer Custody

Accepted INST, the built-in installer-lifetime portion of FE-I26. llama.cpp,
NVFP4 and Sherry each retain an existing SetupOwner configured with a private
recipe. Their public trait ensure methods delegate to that owner; dropped
request waiters no longer release installer lifetime or exclusion. The manager
captures the exact same owner Arcs from its concrete backends and closes all
four owners before its first drain await. Repeated/interrupted shutdown retains
and observes the same workers and failures. Concrete backends expose additive
shutdown_setup methods for direct embedded consumers; no trait signatures change.
Trait-object-only users must retain a concrete handle for explicit shutdown.
PumasApi shutdown_conversion_setup now drains base and quantization setup; RPC
and local IPC managed ensure calls use the same ownership path.

The codebase-design skill guided extending the existing owner and command runner,
not adding another supervisor. Agent moved installer recipes to one private
Module; root wired backend ownership, aggregate shutdown and controlled tests.
Recipes capture only root/program values, never their backend owner. The existing
launcher-data lease serializes all setup kinds across root aliases/processes;
another owner receives contention, not a path-based join. One instance joins its
active operation. The existing blocking worker retains every child through
cleanup before lease release and receipt. Linux cooperating-group and other-OS
foreground limits remain unchanged; lock files and directories must stay stable.

All installer commands now use the retained null-stream runner, including CUDA
compiler detection, avoiding unbounded captured installer output. Each command
uses the existing fifteen-minute setup timeout. Step labels and exit status
remain diagnostic context, rather than captured pip stderr. Cancellation and
runner/cleanup failures cannot become optional success. Only previously optional
git-pull/pip-upgrade unsuccessful exit statuses warn and continue. A private
CommandNotFound outcome allows absent nvcc to select the existing CPU-build path;
other probe failures fail. Existing observed-nvcc-exit detection semantics,
dependency lists, paths, build arguments and venv-present shortcuts are preserved.
Owner failures surface as ConversionFailed and cancellation as InstallationCancelled;
shutdown treats successfully cleaned cancellation as success and retains failures.

Evidence: 79 focused conversion tests passed. Controlled Linux fake executables
traverse all three actual recipes, stopping inside dependency installation.
Nine backend/outcome cases drop the first caller, prove the child remains owned,
reject a competing setup owner, then prove same-owner joining and success/error
or cancellation cleanup. Terminal receipts have reaped the direct child; failure
survives repeated shutdown and closed owners reject setup. A manager fixture
polls then drops aggregate shutdown, proves base and all backend admissions are
closed, resumes/idempotently repeats drain, and observes no unrelated backend
directories created. Existing base setup alias, process-lock, group cleanup,
panic and single-blocking-thread tests continue to pass. Private Programs paths
select isolated fixture tools without changing global PATH. This exercises real
process/owner/recipe boundaries, not real dependency installation or GPU behavior.

Supporting gates passed: 1,440 default and 1,400 no-default-feature core/RPC
tests, with 22 existing ignored tests each; strict all-target clippy in both
feature configurations; formatting, whitespace and all five plan contracts.
Logs: `/tmp/pumas-installer-custody-{focused,default,minimal,clippy-default,clippy-minimal}.log`.

Limits: GUI and wire types/feature gates remain unchanged; no live installers,
models, release build or Windows/macOS execution. Readiness still uses existing
interpreter/binary presence shortcuts, so incomplete-install repair is next.
Backend-specific setup start/observation/retry, setup-versus-conversion exclusion,
hardware/content preflight and stronger containment remain FE-I26 before new
quantization GUI mutations. Custody acceptance does not establish full readiness.

## 2026-09-08 — Managed Quantization Request Admission

Accepted QREQ, the managed-input portion of FE-I26. All four managed
quantization directions validate their exact target against the selected
backend's catalog, including backend identity, before WorkerOwner admission.
Omitted targets retain Q4_K_M, NVFP4 and Sherry-1.25bit defaults. No case,
whitespace, path or foreign-backend aliases are accepted. IQ calibration remains
required even with force=false; force=true requires llama.cpp and calibration.
Supplied paths must resolve at inspection to nonempty regular files. Missing
paths return InvalidParams, while other metadata-inspection errors retain I/O
failure context. NVFP4/Sherry missing optional calibration remains allowed;
their existing sample-data behavior is not changed or newly endorsed.

The codebase-design skill kept the checks in existing managed request
preparation rather than introducing another public validator or owner. Core
manager, PumasApi and RPC managed requests reach this same preparation. Backend
catalogs remain target authority; the worker still owns atomic capacity and
lifetime. Subagent reviewed catalogs/callers and the implementation read-only;
root implemented, verified and integrated. Rustdoc now correctly states that
false cannot bypass IQ requirements and scopes supplied-file checks to managed
quantization starts.

Evidence: 77 focused conversion tests passed. Public-manager contract fixtures
assert exact InvalidParams diagnostics for wrong-backend/unknown/empty/case/
whitespace/path/option-like targets, missing IQ or forced calibration, unsupported
force flags, and supplied missing/empty/directory paths. Rejection preserves
the pre-call library entries and source payload, creates no progress or setup
directory, and leaves shutdown clean. Preparation fixtures accept all current
backend catalog entries and defaults, preserve supplied paths with spaces,
and preserve optional absence where allowed. Existing controlled-process
conversion tests exercise valid requests through real managed worker paths.
Initial new assertions incorrectly assumed the model directory was the only
library entry; the index also owns files there. Corrected to compare the actual
pre-call entry set, retaining the no-staging/no-output claim without a guessed
count. No production correction was needed for that fixture failure.

Both complete core/RPC suites passed: 1,438 default and 1,398 no-default-feature
tests, with 22 existing ignored tests each. Strict all-target clippy passed
with all features and without defaults; formatting, whitespace and all five
plan contracts passed. Logs:
`/tmp/pumas-request-admission-{focused,default,minimal,clippy-default,clippy-minimal}.log`.

Limits: evidence uses isolated local library/file fixtures and controlled
processes, not live models, installers, GPUs or a graphical workflow. No wire
types, dependencies or feature gates change. This is input rejection, not
complete preflight: inspection proves neither readable/valid calibration text
nor retained custody against replacement; callers must keep inputs stable and
readable through execution. Direct QuantizationBackend calls and base Python
format conversion are outside this managed-quantization admission guarantee.
FE-I26 remains open for installer custody, direction-specific tools/hardware,
calibration content and stronger containment before new quantization GUI work.

## 2026-09-08 — Python Quantization Progress

Accepted the remaining FE-I27 script-progress projection. NVFP4 and Sherry now
share base conversion's private stdout JSON decoder and deferred-error policy.
Setup/loading, calibration, training, quantization and export are observable
through existing manager get/list snapshots. Unknown durations are indeterminate.
Sherry announces an epoch before executing it, so training progress measures
`(epoch - 1) / epochs_total`. Missing pairs remain indeterminate; invalid provided
pairs retain failure until cleanup. Epoch transport fields remain private.
Complete still means Writing at 95%, not terminal success. The first script
error survives subsequent records and exit zero; nonzero exit retains both
contexts, and cancellation remains distinct. Publication follows successful
cleanup and protocol outcome; retained worker receipts still own terminal state.

The codebase-design skill guided one private script-protocol module shared by
three callers, replacing the base-conversion local parser. Native cleanup,
worker custody and backend argument/publication ownership are unchanged. The
subagent implemented the private runner and tests; root integrated the three
callers, phase tracker and public-manager fixtures, then serialized verification.

Evidence: 73 focused conversion tests passed. Controlled Linux script tests
cover mixed diagnostic stdout, stderr excluded from protocol, stage projection,
missing/invalid epochs, first-error retention on zero/nonzero exit, and
cancellation after an observed error. Actual NVFP4/Sherry backend fixtures hold
scripts while public manager get/list show their phase and no output identity.
Both successful and error-reporting zero-exit cases prove indexed completion
only after release, or failure without publication. Existing four-backend
versioned-output and base-conversion terminal-authority tests also pass. The
initial focused run caught a mistaken test expectation: quantization deliberately
clears prior tensor counters when entering finalization. The corrected assertion
preserves that contract; held-phase fixtures verify progress before it is cleared.
Tiny isolated fixture payloads exercise real process execution and backend
integration, not actual model tools, GPUs or live library data.

Supporting gates passed: 1,434 default and 1,394 no-default-feature core/RPC
tests, with 22 existing ignored tests each; strict all-target clippy in
all-feature and no-default configurations; formatting, whitespace and five
plan-contract checks. The first minimal run had 1,200 passing tests and one
failure in existing download admission
`incumbent_disappearance_cannot_return_a_stale_id_or_drop_overlap_files`:
its one-second second-ID wait timed out. That test passed unchanged in isolation,
then the complete minimal suite passed unchanged. Timing sensitivity is a
possible cause, not an established diagnosis or a fixed download defect. Preserve
this signal for verification follow-up if it recurs; no timeout was relaxed.
Logs: `/tmp/pumas-quant-progress-{focused,default,minimal,minimal-recheck,download-recheck,clippy-default,clippy-minimal}.log`.

Limits: public/wire shapes and feature gates are unchanged; backend operation
remains standalone and GUI-independent. Prior UI terminal-consumer evidence is
unchanged, not newly rerun graphical verification. FE-I27 is resolved; FE-I26
installer custody, preflight and stronger containment remain open before new
quantization GUI mutation (FE-I23). No release build or live library mutation.

## 2026-09-08 — Conversion Terminal Progress Authority

Accepted the terminal-authority portion of FE-I27. Python complete/error
records previously became public terminal status while their process could
still run and before output publication/indexing. Script completion now means
Writing at 95%; failure text stays local until native cleanup, then fails the
operation even on exit zero. Nonzero exit preserves both native and script
context; cancellation retains its distinct outcome. Late script records cannot
rewrite terminal snapshots. Duplicate completion writes in conversion and
quantized metadata publication were removed: the existing retained worker
receipt alone publishes terminal status. Success sets 100% and clears stale
error in the same tracker critical section; cancellation clears stale error.
Output identity is recorded after indexing, before the worker returns.

The codebase-design skill localized snapshot semantics in the existing tracker
and terminal authority in the existing worker owner, without adding a registry,
state machine or public Interface. Root implemented and verified the backend;
the subagent reviewed sibling producers and added the optional UI consumer
regression. No production frontend changes were needed.

Evidence: 68 focused conversion tests passed. A real controlled Linux process
emits complete, remains held, then reaches a separately held metadata write.
Public manager get/list remain Writing before child exit and Importing before
indexing; only the worker receipt exposes Completed with the indexed identity.
Six cases cover success, nonzero exit, reported failure with exit zero,
cancellation, reported failure plus nonzero exit, and reported failure then
cancellation. Existing four-backend output integration also proves pipeline
publication supplies identity without declaring terminal status. Fixtures use
tiny payloads, bounded holds and isolated roots, not model tools or live models.

Both full core/RPC suites passed: 1,429 default and 1,389 no-default-feature
tests, with 22 existing ignored tests each. Strict all-target clippy passed in
all-feature and no-default configurations; formatting and whitespace passed.
The 21 selected UI hook/dialog tests, focused frontend lint and full frontend
type check passed. The new hook test keeps polling and cancellation through
writing/importing, refuses duplicate start, and refreshes exactly once after
backend completion. This is simulated consumer evidence, not a new graphical
workflow or real conversion claim. All five plan-contract checks passed.
Rust logs: `/tmp/pumas-terminal-progress-{focused,default,minimal,clippy-default,clippy-minimal}.log`.

Limits: wire shapes and feature gates are unchanged; backend use remains
standalone and GUI-independent. NVFP4/Sherry stdout remains log-only pending
nonterminal projection under this contract. FE-I27 therefore remains open;
stronger containment, installer custody and preflight remain FE-I26. No new
quantization GUI capability, release build or live library mutation is accepted.

## 2026-09-07 — Linux Cooperating Conversion Groups

Accepted the bounded cooperating Linux group-cleanup portion of FE-I26.
Native conversion and blocking setup now share private Linux identity and task
observation. Both retain the direct child without reaping it until its own
group has been signalled and no live group tasks remain. Setup no longer uses
`try_wait` before numeric group signalling. Native execution also stops surviving
group members on successful leader exit, before returning to publication.
Read-only `/proc` scans use blocking capacity for the async runner; no queued
task can signal a PID after its owner drops the child. Observation failures
retain custody and remain failures after eventual cleanup.

The subagent implemented the shared helper and setup integration; root wired
native execution and reviewed the composed lifecycle. The codebase-design skill
kept Linux identity/liveness rules behind one private module, without changing
backend argument builders or progress parsers. A zombie process leader is not
sufficient evidence: group scans inspect worker tasks and fail closed when
task visibility is incomplete. Controlled tests cover exited leaders with live
descendants, success/cancel/observer panic, and an actual pthread worker whose
main thread has exited. Test processes have explicit cleanup guards or bounded
lifetimes; the tiny thread fixture requires `cc` and pthreads, not model tools.

Review counterevidence: the first 64 focused tests passed, but nix 0.29's
`WaitStatus` rejects realtime terminating signals. Treating that conversion
failure as lost ownership would retain a zombie indefinitely. The full gate was
interrupted before acceptance. The correction uses raw-capable wait status from
the existing pinned rustix dependency, enabling its `process` feature; a real
realtime-signal termination fixture guards this path. No new dependency or Rust
unsafe code is introduced.

Second gate counterevidence: the default suite passed 1,426 tests, but the
minimal suite failed immediate setup retry with `already running`. The
diagnosing-bugs skill narrowed this to lease release: 30 isolated retry runs
passed, completion is published after execution returns, and each fixture root
is unique. A retained duplicate of the acquired descriptor deterministically
reproduced busy after dropping the original lease. Linux
[`flock` ownership](https://man7.org/linux/man-pages/man2/flock.2.html) is shared
by duplicated/inherited descriptors; closing only the parent's descriptor is
not explicit release. A new lease guard unlocks after child cleanup, retains
custody while unlock is unresolved, and preserves eventual release errors.
The regression also checks active-owner exclusion and that dropping the old
duplicate cannot unlock the successor. This establishes the descriptor-lifetime
defect; an unrelated fork in the original full-suite failure is an inference,
not captured process evidence. No sleeps, busy retries or serialization were
added to hide the original symptom.

Final evidence on Linux 7.0.0-28-generic: 66 focused conversion tests passed.
Full core/RPC suites passed with 1,427 default-feature and 1,387 minimal-feature
tests, each with 22 existing ignored tests. Strict all-target clippy passed
with all features and with no default features. Four extraction-only needless
borrows were corrected, then both full suites passed again on final source.
Formatting, whitespace and all five plan-contract checks passed. Logs:
`/tmp/pumas-linux-group-{focused,default,minimal,clippy-default,clippy-minimal}.log`;
the deterministic pre-fix lock failure is `/tmp/pumas-setup-lease-red.log`.

The identity ordering relies on Linux's documented non-reaping
[`WNOWAIT` contract](https://man7.org/linux/man-pages/man2/waitpid.2.html).
Kernel fork publication and group signalling serialize under `tasklist_lock`,
with fatal-signal checking before publication; inspected primary sources are
[fork.c](https://raw.githubusercontent.com/torvalds/linux/v6.12/kernel/fork.c)
and [signal.c](https://raw.githubusercontent.com/torvalds/linux/v6.12/kernel/signal.c).
This supports the cooperating-group scope, not arbitrary hostile containment.

Limits: the host must preserve exclusive wait ownership and ordinary SIGCHLD
semantics. `ECHILD` cannot authorize signalling or cleanup success. Descendants
escaping groups, namespaces or credentials remain outside the contract.
Direct embedded callers must cancel and await; unmanaged future drop has no
observed cleanup guarantee. Non-Linux behavior remains foreground-only and was
not runtime-tested. Stronger containment, quantization-installer ownership,
preflight, retained-staging cleanup policy and FE-I27 progress authority remain
open. No GUI, wire shapes, live library data or feature-gating contract changed.

## 2026-09-07 — Foreground Conversion Execution

Accepted the bounded foreground portion of FE-I26. Review found all six
execution calls consumed only one of two piped
streams and checked cancellation only between lines. A quiet child could block
cancellation; an unread pipe could deadlock execution. The private native
process runner replaces the shared stderr/wait helpers and the separate Python
and llama.cpp stream loops. The runner owns null stdin, both output pipes,
bounded record framing, child completion, cancellation and direct-child reaping.
Backend callbacks retain argument and progress-format policy. No additional
runtime, task registry, dependency or detached pipe/cleanup tasks were added.

Records are limited to 64 KiB, preserve final unterminated lines and trim CRLF.
Malformed UTF-8 and oversized records fail explicitly. Cancellation is checked
on each read-loop turn and a 50 ms wake covers silent children. After foreground
exit, a one-second drain allowance precedes explicit failure for still-open
pipes; it never silently truncates to success. Reader/parser errors and caught
observer unwind keep the child outside the unwind scope until cleanup observes
reaping. Failed cleanup observation retains the worker; an eventual cleanup
failure preserves its cause alongside the original operation failure.

The subprocess subagent implemented the runner and seven focused tests, then
reviewed all six caller migrations. Root integrated Python format conversion,
llama.cpp F16 conversion/imatrix/quantization, NVFP4 and Sherry; strengthened the
existing actual-output/index fixture with stdout JSON and stderr tensor progress;
and added real quiet-child execution through the retained worker owner. Dropping
one shutdown waiter does not prevent a later waiter from observing cancellation
and foreground reaping; no destination is published and staging remains.
The codebase-design skill kept parsing separate from the shared lifecycle
Interface, so a pipe or cancellation policy change no longer requires changes
in every backend. Quantization Rustdoc now documents cancel-and-await and
retained staging instead of incorrectly promising cleanup on every failure.

Evidence: all 58 focused conversion tests passed. Linux system tests use real
shell/exec processes and tiny controlled payloads: both pipes exceed pipe
capacity, final records arrive, quiet cancellation reaps, observer panic reaps,
pre-cancelled calls do not spawn, and bad output/nonzero/spawn failures surface.
The inherited-pipe case injects a separately owned real pipe holder at the
private drain seam and explicitly stops/reaps it; this proves the bounded
foreground-output policy, not actual descendant containment. Review corrected
an undersized oversized-record fixture before execution. Full core/RPC suites
passed: 1,419 default-feature tests and 1,379 minimal-feature tests, each with
22 existing ignored tests. Strict all-target clippy passed with all features
and with no default features. Formatting, whitespace and all five plan-contract
checks passed.
Logs: `/tmp/pumas-foreground-process-{focused,default,minimal,clippy-default,clippy-minimal}.log`.

Boundaries: foreground receipt does not establish descendant cleanup or prevent
descendant writes after publication. Direct embedded callers must cancel via
token and await; dropping a future invokes only the existing kill-on-drop
fallback, not an observed cleanup contract. Setup's numeric group-kill loop is
not generalized because post-reap group identity can be reused. Process-tree
containment and installer ownership remain FE-I26 prerequisites. NVFP4/Sherry
stdout is now drained/logged but their JSON progress is not newly projected;
raw script terminal status remains distinct from actual worker/publication
completion and needs the separate FE-I27 progress-authority repair. Quantization
preflight and FE-I23 GUI controls remain deferred. No actual installer/model
tools, live library data, GUI, generated API shapes or feature gates changed.
No Windows/macOS runtime or release acceptance is claimed. Unrelated deletions
and private recovery artifacts remain untouched.

## 2026-09-07 — Conversion Output Publication

Accepted a bounded FE-I26 destination-safety prerequisite. Deterministic
`.converting`/`.quantizing` staging previously deleted another attempt's files.
The finalizer also discarded its chosen versioned destination, so callers could
write metadata or report an ID for the original occupied output instead.

One private `OutputWorkspace` exclusively allocates staging beside the desired
output and consumes itself to return the actual published path. Publication
reuses the platform non-replacing move and retries only `AlreadyExists` with a
version suffix. Files, directories, dangling symlinks and non-UTF8 leaf names
retain their identities; other I/O errors surface without replacement or a
copy/delete fallback. Python conversion, llama.cpp, NVFP4 and Sherry all use the
returned identity for metadata, indexing and progress. Existing deterministic
staging is untouched; failed, cancelled and abandoned attempts retain their
unique staging. Review removed Python error-path deletion because an ignored
leader-kill error does not prove that native producers have stopped.

The output subagent implemented the workspace and seven temporary-root tests;
root integrated the four consumers and a simulated-executable test covering
their real metadata/index/progress paths. The codebase-design skill placed
allocation, collision policy and returned identity behind one shared interface.
Fixtures use tiny shell-produced payloads, not actual model tools. The first
consumer run exposed an incomplete simulated llama.cpp installation; correcting
its unused conversion-script fixture made all 50 focused conversion tests pass.

Verification: complete default core/RPC suite passed 1,411 tests; minimal-feature
suite passed 1,371. Each retained 22 existing ignored tests. Strict all-target
clippy passed with all features and with no default features. Rust formatting,
diff whitespace and all five plan contracts passed.
Logs: `/tmp/pumas-conversion-outputs-{focused,default,minimal,clippy-default,clippy-minimal}.log`.

Boundary: parent paths must remain stable. This is not hostile-filesystem path
pinning, native process-tree cleanup, crash-atomic metadata/index publication,
installer custody or cross-process conversion admission. A dropped direct
publication waiter can still allow its blocking move to finish; supported
manager execution retains that future through the existing worker owner.
Retained staging cleanup/recovery requires a later admitted policy and proof
that producers have stopped. No live model data, installer, GUI, dependency,
generated API shape or feature gate changed. No real conversion, release or
Windows/macOS runtime acceptance is claimed. Unrelated workflow/stub deletions
and private recovery artifacts remain untouched.

Next: native process cleanup, quantization installer ownership and preflight;
FE-I26 remains open and FE-I23 quantization GUI configuration remains deferred.

## 2026-09-07 — Conversion Worker Observation

Quantization admission review found that base Python setup custody does not
cover quantization installers or execution. Accepted a bounded backend
prerequisite instead of enabling new GUI mutations: the private worker owner
replaces split token/handle maps and progress-count admission. It atomically
checks manager-local capacity/closure, inserts initial progress and registers
the task only after read-only model lookup and backend preparation. Rejected
admission has no ghost progress or polled worker. Script status is not execution
slot or cancellation authority.

Cancellation now signals the worker without aborting it or immediately marking
it cancelled. Joined success, operation failure, cancellation and panic determine
the retained outcome. Completed workers are observed before pruning; the first
failure remains available to repeated shutdown callers and individual failures
remain in progress records. A borrowed join handle survives a dropped shutdown
waiter, and closed admission prevents new workers while draining. Cancellation
also remains valid while a shutdown observer holds the completion lock.

`ConversionManager::shutdown` now returns a fallible result, and embedded users
can call the additive `PumasApi::shutdown_conversions` before stopping their
runtime. RPC shutdown composes this drain with downloads, catalog and base setup,
observing all owners even after another fails. No new runtime, dependency,
generated wire shape, GUI requirement or feature-gate change was introduced.

The subagent inspected backend prerequisites and implemented worker ownership;
root implemented public/RPC composition, reviewed the contended-cancellation
case, and ran gates. The codebase-design skill concentrated lifecycle machinery
behind one private owner rather than duplicating it across manager branches.
Four controlled-future tests cover held cancellation, interrupted/repeated
shutdown, closure/rejected work, premature script completion, panic/error
retention and two-thread competing admission. RPC adds a delayed-worker drain
test with a dropped waiter and another failed owner.

Verification: complete default core/RPC suite 1,403 passed; minimal-feature suite
1,363 passed; each retained 22 existing ignored tests. Strict all-target clippy
passed with all features and with no default features. Rust formatting and all
five plan contracts passed. The 20 affected frontend hook/dialog contract tests,
frontend types and lint passed. Their first run exposed an existing reopen
visibility assertion racing modal animation; it now waits for visibility, with
no production renderer change. An initial sandboxed core run could not create
required local socket fixtures; the complete permitted run is the accepted
evidence, not those permission failures.

Logs: `/tmp/pumas-conversion-workers-{default,minimal,clippy-default,clippy-minimal,ui-contract,ui-types,ui-lint}.log`.
No real installer/conversion/model data was used, no caches were deleted, and
unrelated workflow/stub deletions and recovery artifacts remain untouched. No
new GUI, release, Windows/macOS or process-tree-cleanup acceptance is claimed.
Next: FE-I26 native process/destination custody, installer ownership and
quantization preflight; FE-I23 GUI configuration remains deferred behind them.

## 2026-09-07 — Conversion Setup Dialog

Accepted FE-I25's optional dialog migration to `start_conversion_setup` and
`get_conversion_setup`. Mount/reopen only read. The existing workflow hook owns
one serialized queue and poll timer; backend installation remains independently
owned. Admission releases the busy dismissal gate before follow-up reads.
Active setup blocks conversion without a guessed percentage. Terminal status
does not imply readiness; failed/cancelled setup offers explicit retry with the
observed operation ID, while a completed but unready environment offers repair.
Null or an unchanged terminal ID cannot resolve uncertain admission; refresh
can attach to current backend work without automatically repeating installation.
Read failure stops automatic polling and exposes bounded manual refresh.

The hook agent owned lifecycle implementation/tests; root owned dialog,
integration and modal-test adjustments, review and verification. The
codebase-design skill kept setup lifecycle complexity behind the existing hook
interface rather than distributing it across controls. Full-suite counterevidence
required an idle setup-read fixture in ModelManager tests and waiting for modal
visibility/focus restoration instead of asserting during animation teardown.
No modal production code, backend, generated contract or feature gate changed.

Evidence: 577 frontend tests in 115 files, strict frontend typecheck/lint,
library-only and default production builds, and all five canonical plan checks.
Actual built Linux Chromium through the compiled sandboxed preload exercised
explicit consent, simulated lost admission response, read-only reconciliation,
active close/reopen, read failure stopping polls, identity-bound failed retry,
completion/readiness, conversion progress/cancellation/completion refresh, native
Escape and focus restoration in both modes. Visible screenshots were inspected
for the active setup message, usable Close and blocked Start conversion. No
renderer warning/error occurred; main-process IPC errors were deliberately
injected fixture failures. Default build was restored last.

Temporary evidence: `/tmp/pumas-setup-dialog.5S7QJn/main.cjs`, producer fixtures
exported by the existing core/RPC binary, screenshots in that directory, and
`/tmp/pumas-setup-dialog-{types,lint,tests,build-library,build-default,gui-library,gui-default}.log`.
These are simulated installation/conversion outcomes, not native execution,
real 60-second transport timing, restart persistence, release packaging or
Windows/macOS evidence. Existing backend custody evidence remains separate.

Environment repair restored absent lockfile-pinned JavaScript dependencies and
Electron without modifying manifests/lockfile. Offline restore was unavailable;
online restore needed a lower-concurrency retry. Partial-install checks are not
acceptance evidence. No model files/build caches/recovery artifacts were removed;
unrelated workflow/stub deletions remain unstaged. Next: FE-I23 quantization
configuration admission; whole M4/M5 and program acceptance remain open.

## 2026-09-07 — Conversion Setup Observation

Accepted the FE-I25 core/RPC/desktop observation contract, not dialog migration.
`start_conversion_setup` promptly starts or attaches to the retained operation;
`get_conversion_setup` reads its UUID and in-progress/completed/failed/cancelled
state without filesystem work. Idle is explicit null, not readiness. Terminal
state follows cleanup and lease release. Only a matching observed terminal ID
permits explicit retry; duplicate/stale retry requests return current work even
after completion. A valid old token on a fresh owner fails without installing.
The existing blocking setup method retains its behavior on the same owner.

The backend agent owned core types, admission, snapshots and facade tests. Root
owned RPC parsing/disclosure/schema fixtures, generated output, preload/registry,
typed consumer contracts and gates. The codebase-design skill kept lifecycle
behind the existing backend interface. Identity generation precedes worker spawn;
replacement is atomic, including calls through the existing blocking method.
RPC preserves canonical UUIDs and state/error correlation but replaces private
failure text with a bounded public message. No new runtime, dependency, GUI
requirement, persistence or inference-plugin gate was introduced.

Acceptance evidence:

- Core/RPC: 1,398 default and 1,358 no-default-features tests pass; 22 existing
  ignored tests each. The focused conversion suite passes 38 tests. Fixtures
  cover read-only idle, concurrent admission/retry, retained identity after
  dropped waiters, failed/cancelled snapshots, invalid/obsolete tokens and
  cleanup-before-terminal. RPC tests cover closed requests and redacted outcomes.
- Strict all-target Clippy passes with all features and no defaults. Rustfmt,
  generated freshness and all five plan contracts pass.
- Actual producer/decoder and compiled-preload/typed-consumer conformance each
  pass 12 tests, covering all states, explicit null, UUIDs, exact retry forwarding,
  malformed payloads and contradictory errors. Generator tests pass 6;
  compiled-preload/registry tests pass 14 with one existing GUI-oracle skip.
  Electron build and both projects' lint
  pass. Frontend types, all 569 tests in 115 files and both production builds pass;
  default renderer build output is restored.
- Standalone localhost HTTP uses the current no-default-features, contract-export
  binary with an isolated library and owned fake interpreter. It proves idle,
  rejected obsolete-token admission, timely start while an installer is held,
  retained ID/status, observed completion/reaping and repeat-safe explicit retry.
  No real packages, models or live application state were changed.

Environment limit: an additional bare minimal-binary link crashed in rust-lld
with the repository drive nearly full (about 420 MB free). HTTP evidence instead
uses the successfully built current headless contract-export binary; the failed
link is not claimed successful. Rust incremental caches occupy about 203 GB.
No caches or recovery artifacts were removed; free build space before further
large builds. This is not Windows/macOS or release-artifact acceptance.

Identity remains latest-only and owner/process-lifetime, not durable history,
restart recovery or another manager's operation discovery. FE-I25 stays open for
the optional dialog's start/observe, timeout/reopen and consent behavior, followed
by remaining FE-I23 quantization configuration. No new GUI interaction claim,
full M4/M5 acceptance or Pending cleanup replay acceptance is made here.

## 2026-09-06 — Conversion Setup Custody

Accepted the backend custody prerequisite of FE-I25, not its public operation
identity/state contract. One private setup owner retains the worker, result,
installer children and physical environment lock. Same-manager callers join
active work; independent managers/processes fail contention before deploying
scripts. Dropped request waiters cannot abandon installation or release its lease.
An explicit new request can retry completed failure after the old worker is
observed. Setup shutdown closes admission, cancels and drains; interrupted or
repeated shutdown waiters retain the outcome. RPC shutdown observes setup,
downloads and catalog even when listener or another owner's drain fails.

The backend agent implemented the setup owner and facade integration. Root
integrated RPC shutdown and hardened retained join failures, non-UTF-8 Linux
process-name parsing and single-blocking-thread runtime support. The
codebase-design skill kept custody behind one private backend interface rather
than adding UI-owned installation or a second lifecycle framework. Script
deployment retains one manifest/hash policy with tested async/blocking I/O
adapters, avoiding nested blocking-pool waits inside setup. No new dependency,
runtime, feature gate or setup/check wire shape was introduced.

Acceptance evidence:

- Core/RPC: 1,391 default and 1,351 no-default-features tests pass, with 22 existing
  ignored tests each. The focused conversion suite passes 34 tests.
- Controlled Linux executables prove same-owner joining, dropped callers,
  cross-process and alias exclusion, preserved execution through symlinked
  launcher-data, cancellation and interrupted shutdown, deadline/unwind cleanup,
  child reaping and no live installer process-group members before release.
  Additional fixtures prove explicit failure retry, repeatable worker failure,
  script-adapter parity and a host runtime with only one blocking thread.
- RPC composition proves all four failures remain observable despite an
  interrupted shutdown waiter. Strict all-target Clippy passes with all features
  and no defaults; rustfmt, generated freshness and all five plan contracts pass.

Limits: advisory exclusion assumes stable launcher-data/lockfile identity and
installers staying in their process group. Linux cleanup retains custody if it
cannot establish quiescence. Other platforms naturally drain the foreground
installer; their process trees are not verified here. Abrupt host death,
quantization-backend setup, conversion-job shutdown and actual installation or
conversion are outside this slice. No GUI source changed or new GUI verification
claim is made. Existing recovery artifacts remain untouched.

Next: expose observable setup identity/state to embedded/RPC/desktop consumers
across timeout and reopen (FE-I25 remains open), before remaining FE-I23
quantization configuration. Full M4/M5 and Pending cleanup replay remain open.

## 2026-09-06 — Format Conversion Workflow

Accepted the bounded FE-I23 GGUF/safetensors format workflow, replacing the
withdrawn no-op with an actual dialog. Complete current models without integrity
warnings expose an accurately named format action; partial, cached and unsupported
formats do not. The dialog discloses F16 output and dequantization limits, requires
explicit installation consent, starts through the generated bridge, displays
backend progress and requests cancellation without claiming terminal success.
Reopen reads existing work; close stops UI polling without cancelling backend work.

The backend agent owned the workflow hook/tests and the directly required Rust
readiness fix. Root owned presentation, integration, modal focus, gates and records.
The codebase-design skill guided the split: one hook owns serialized operations,
polling, stale results and uncertain outcomes; rows only open the dialog. The
backend remains independent of GUI composition and inference-plugin features.

Integration findings changed two directly affected owners:

- Readiness formerly meant only that the venv interpreter existed, so failed or
  ongoing dependency installation could appear ready. Core now probes required
  imports using isolated Python with bytecode writes disabled and a five-second
  bound. Async inspection stays off the async worker. Explicit setup repairs an
  existing incomplete environment and verifies imports before success. Missing
  imports/timeouts mean not ready; async spawn/observation I/O errors propagate.
- Completion refresh replaces row controls. Modal close now uses the task-owned
  library-region focus destination if its original opener disappeared. An existing
  nested teardown test also exposed a cleared-ref race; cleanup now retains the
  owned DOM node when transferring focus restoration to closing descendants.

Acceptance evidence:

- Frontend: 569 tests in 115 files pass; types/lint and both builds pass. Focused
  hook tests cover duplicate prevention, accepted-but-not-listed work, uncertainty,
  sequential polling, read errors/retry, cancellation and teardown/supersession.
  Dialog/manager tests prove consent, exact source/direction, progress, reopen and
  eligibility; modal tests prove the refresh fallback and nested teardown.
- Core/RPC: 1,381 default and 1,341 no-default-features tests pass, with 22 existing
  ignored tests each. Three new Unix readiness tests use owned fake executables
  to prove import failure, repair/retry, timeout/reaping and I/O error semantics.
  No real package installation or model conversion occurs in those fixtures.
- Strict all-target core/RPC Clippy passes with all features and no defaults;
  rustfmt and generated freshness pass. Producer/decoder and compiled-preload/
  typed-consumer conformance each pass 10 tests. Standalone HTTP conversion reads,
  unknown cancellation, quant metadata and environment/backend readiness pass in
  a fresh temporary library without the GUI.
- Both built GUI modes are exercised in hidden Chromium windows with compiled
  preload and producer-derived fixtures: pointer setup consent, exact start,
  determinate progress, close/reopen without duplicate start, native Escape,
  cancellation, completion-driven library refresh and focus restoration. Linux
  display `:0.0`, isolated temporary profile, context isolation/renderer sandbox
  enabled, bounded 30-second process watchdog; no live application/model access.
  Screenshots were inspected. Native installation/conversion is simulated, not
  validated by this GUI evidence. Default build output is restored.

Remaining: FE-I25 owns backend setup lifetime/identity across the existing
60-second desktop timeout and independent callers. The UI retains unconfirmed
outcomes and does not automatically repeat setup/start; that does not establish
backend operation ownership across reopen or multiple apps. Resolve that contract
before quantization-specific GUI configuration (remaining FE-I23). Real native
execution, non-Linux evidence, full M4/M5 and Pending cleanup remain unaccepted.
Existing recovery artifacts are untouched.

## 2026-09-06 — Inert Conversion Control Withdrawn

Accepted only the FE-I23 control-withdrawal alternative, not the conversion
workflow. Inspection confirmed the sole GUI action logged a TODO; no dialog or
progress consumer existed. Removed that callback, forwarding props, icon and
conversion-only row state. Core, RPC, generated/preload contracts and backend
conversion remain unchanged. The frontend README records the incomplete GUI
lifecycle and re-enable criteria; FE-I23 remains open as the next workflow slice.
This explicitly narrows the initial workflow implementation intention to remove
a production no-op while preserving that unfulfilled objective.

Verification: 553 frontend tests in 113 files pass, including complete GGUF and
safetensors rows with no conversion action and import still enabled. Types/lint
and both production builds pass. A hidden Electron/Chromium window using the
compiled preload and built renderer verifies both configurations: both formats
render, import remains present and no button advertises conversion. No renderer
warnings/errors were observed; screenshot inspected. Linux display `:0.0`,
context isolation and renderer sandbox enabled, private temporary profile and
fixture responses, bounded 30-second watchdog; no live backend/model access.
This proves presentation removal, not native conversion or a replacement user
workflow. Default build restored. Existing recovery artifacts untouched.

## 2026-09-06 — Conversion Operation Contracts Accepted

FE-I24 is resolved. A bounded backend agent supplied canonical outcome schemas,
projection validation and producer evidence; root integrated generated contracts,
preload routes, typed consumers and verification. Start/cancel/environment/setup,
quant options and backend readiness now cross validated desktop boundaries.
Quant options preserve core camelCase, explicit nullable backend and importance-
matrix metadata. The start bridge retains its existing positional arguments and
adds optional calibration-file and force-imatrix inputs, preserving false/null.
Existing backend status and backend setup RPC routes are now exposed to desktop
callers. Invalid started IDs and nonfinite/negative quantization evidence fail
projection rather than becoming misleading success data.

Core public APIs and native mutation execution remain unchanged. Schema derives
are conditional; neither embedded Rust nor standalone RPC requires the GUI.
The codebase-design skill guided this boundary: core owns conversion policy,
RPC projects outcomes, preload validates transport and the renderer consumes
generated types. No GUI conversion workflow or native setup was added.

Acceptance evidence (Linux):

- RPC suites: 123 default and 83 no-default-features tests pass; 10 existing
  ignored tests each. Isolated handler evidence verifies unknown cancellation,
  actual available quant metadata and all three backend readiness states.
- Strict all-target core/RPC Clippy passes with all features and without
  defaults; rustfmt and generated-contract freshness pass.
- Producer/decoder conformance: 10 tests pass, covering real catalogue fixtures,
  nullable backend, false/true outcomes, Unicode/UTF-8 ID boundaries and malformed
  metadata rejection. Bundled-preload/typed renderer conformance: 10 tests pass,
  including exact start arguments, omission/null/false and backend setup routing.
- Full frontend: 551 tests across 113 files pass. Electron: 11 test files pass.
  Type checks, both linters and both production GUI builds pass; default output
  restored. One fixture unsafe-return lint finding was corrected before acceptance.
- A standalone RPC process using a fresh temporary root passes HTTP progress/list,
  unknown cancellation, environment, quant metadata and backend readiness checks.
  No conversion, installation, download or live-model mutation was performed.

Start/setup success fixtures validate transport, not actual native execution.
No GUI interaction or non-Linux runtime verification is claimed. FE-I23, full
M4/M5 and Pending cleanup replay remain open; existing recovery artifacts are
untouched. Next: a separate GUI conversion capability/workflow admission.

## 2026-09-06 — Conversion Progress Contract Accepted

FE-I14 is resolved for the existing get/list progress routes. A bounded backend
agent implemented canonical projection validation, enum schema derives and RPC
fixtures; root integrated generated consumers, preload decoding and verification.
The frontend no longer claims snake_case or omitted optional fields for a
camelCase, explicit-null response. Generated aliases include all six directions,
fourteen statuses and pipeline fields. Read/list wire errors are null or the
existing fixed redacted message. Invalid fractions and unsafe byte counters fail
the canonical constructor and decoder instead of producing misleading progress.

Core public read signatures and persisted conversion metadata are unchanged.
Conditional enum schema derives do not add GUI dependencies. Start, cancellation,
tool installation and quant-option contracts are deliberately not migrated by
this read-only slice. Inspection also found the GUI conversion action is still a
logging TODO; FE-I23 tracks that unfinished workflow and FE-I24 owns the remaining
API migration. No working conversion screen is claimed.

Acceptance evidence:

- RPC default suite: 121 passed; no-default-features suite: 81 passed, with 10
  existing ignored tests each. Actual handler tests preserve missing progress
  and empty lists; constructor tests reject nonfinite/out-of-range progress and
  either byte counter above JavaScript's safe integer limit while preserving
  explicit nulls and redaction.
- Strict all-target core/RPC Clippy passes with all features and without defaults;
  rustfmt passes. Public core compilation remains GUI-independent.
- Actual producer fixtures cover 84 direction/status combinations. Generated
  freshness passes; producer/decoder conformance passes 8 tests, including old
  field names, extra fields, missing explicit nulls, malformed numeric evidence,
  unsupported vocabulary and unredacted error rejection. Comparisons assert
  wire fields independently of the decoder's intentional null-prototype objects.
- Producer/bundled-preload/typed renderer-consumer conformance passes 9 tests,
  including all conversion fixtures, exact read identifiers, missing progress,
  list reads and malformed response rejection. There is no progress screen to
  exercise; this proves the current callable consumer contract, not GUI behavior.
- Full frontend: 551 tests in 113 files pass; Electron: 11 test files pass.
  Types/lint and both production GUI builds pass; default build restored.
- A fresh standalone RPC process on localhost returns `progress: null` for an
  unknown conversion and an empty conversion list, and rejects invalid parameters.
  Its root is a private temporary library; no conversion, download, setup, model
  mutation or live application interaction occurs.

Evidence is Linux-only. Full M4/M5, actual native conversion execution, the GUI
workflow and Pending cleanup replay remain open. Existing recovery artifacts are
untouched. Next: admit FE-I24 without making backend functionality GUI-dependent.

## 2026-09-06 — Import-Picker Contract Accepted

FE-I11 is resolved. The Electron-owned picker contract distinguishes selected
paths, cancellation, invalid data and unavailable native/IPC work. Missing or
destroyed windows no longer masquerade as cancellation. The canonical decoder
rejects contradictory/extra fields and retains no mutable path-array alias;
selection preserves spelling, Unicode, order and duplicates. This is an atomic,
non-persisted desktop contract replacement, not a backend import change.

The renderer exposes failure and named retry, shows pending state and disables
duplicate selection, preserves an existing selection on failure/cancellation,
and classifies late work after close/unmount as superseded. Native completion
remains observed because this bridge cannot cancel the OS dialog. Backend import
validation and execution remain independent of GUI selection and are unchanged.

Acceptance evidence (Linux, automated unless noted):

- Focused hook/presentation tests: 19 passed, including duplicate admission,
  close/unmount, failure/retry, exact paths, native-button keyboard activation
  and disabled/pending semantics. Full frontend: 551 tests in 113 files passed.
- Electron: all 11 test files passed, including native-outcome projection,
  closed decoder and actual bundled-preload contract cases. Native rejection
  does not expose private diagnostic strings. Type checks and lint pass.
- Actual producer/bundled-preload/renderer conformance: 8 tests passed. Picker
  fixtures exercise failure, cancellation and selection through the production
  native adapter and preload into the real hook, without a Rust picker contract.
- Default and library-only production builds pass. Isolated real Chromium with
  the compiled preload and native-result fixtures passes pointer import,
  visible failure, retry/cancel, exact paths reaching import classification,
  import-dialog opening and close. No backend import mutation is invoked.
  Hidden-window capture needed a separate compositor observation; no production
  timing or startup behavior was changed. Default GUI build is restored.

The GUI fixture uses a private temporary profile, disabled sandbox/GPU for the
verification environment and bounded process lifetime; it does not drive the
real OS picker, prove native Windows/macOS behavior, or claim backend import
execution. No live model files or user configuration changed. Existing recovery
artifact directories remain untouched. Conversion contracts remain next; full
M4/M5 and Pending cleanup replay are not accepted by this slice.

## 2026-09-06 — Standalone Backend And Link-Health Contract

Status: accepted for this bounded slice; FE-I10 is resolved.

The user prioritized API/UI contracts with the GUI as an optional consumer.
Three scoped agents implemented launcher selection, public backend projection,
and UI invocation ownership; root integrated the generator, preload, conformance,
documentation and composed verification. The existing registry now owns the
shared read, avoiding duplicate policy or a GUI-dependent backend service.

- `PUMAS_GUI=false` omits desktop build/dependency/test paths independently of
  inference plugins; direct Cargo and the RPC binary remain Node-independent.
  Run executes a selected existing artifact without implicit build. GUI-only
  release-smoke refuses headless use explicitly.
- Public `LinkRegistry::health`, the owning facade and local dispatch share the
  read-only implementation. The existing result signature and registry-wide,
  version-independent behavior are preserved. Orphan discovery and link cleanup
  semantics are not part of this slice.
- The canonical RPC projection validates success, status and wire-safe counts;
  generated immutable decoders replace the handwritten renderer response.
  Failed and superseded reads cannot leave an apparently current healthy report.
  Initial failure, refresh failure, retry, version replacement, same-scope
  supersession and unmount are covered by 12 component tests.
- Full affected Rust suites pass: 1,374 tests with defaults and 1,334 without
  defaults, with 22 existing ignored tests in each combined core/RPC run.
  The public registry tests and actual RPC handler exercise real filesystem
  reads, preserve files, and reject inspection failures without leaking paths.
  Strict all-target Clippy passes with all features and without defaults.
- Frontend: 545 tests, TypeScript and lint pass. Electron: 10 test files,
  compilation and lint pass. Launcher: 49 tests pass, including fake-executable
  GUI/plugin/debug/release delegation and exact argument/error propagation.
  Generator tests pass 6/6; freshness passes; actual producer/decoder conformance
  passes 6/6 and producer/bundled-preload/renderer conformance passes 7/7.
- Both production GUI builds pass. Isolated real Chromium runs with the actual
  bundled preload reject a contradictory producer report, display unavailable,
  and recover through native pointer retry in both GUI modes. Screenshots exposed
  low-contrast new-state text; explicit theme colors corrected it before final
  verification. Native keyboard delivery to the hidden fixture window did not
  execute; keyboard accessibility evidence is the component interaction test,
  not a claimed Chromium keyboard pass. Default build is restored.
- A real standalone binary served healthy and version-independent JSON-RPC
  reads and rejected unknown parameters over localhost using a fresh temporary
  library. No live app, model payload, user configuration or library was changed.
  The real `PUMAS_GUI=false PUMAS_INFERENCE_PLUGINS=false` debug build also passed,
  followed by launcher argument forwarding and the same standalone HTTP check
  against that plugin-disabled binary. All five canonical plan checks passed.

Evidence remains Linux-only and slice-specific. Full API migration, frontend
M4/M5, packaged platform verification and Pending cleanup replay remain open.

## Baseline

- Plan status: `Active` after the Rust-first source gate was released.
- Planning code baseline: `d84e2b3520ce3da3f39cc3df953301fa9d6d3d50`.
- Planning standards baseline: `7bf74bb5a8cb0ffccaff3ec86550051f900fb4bb`.
- Milestones 0 through 3 are accepted; FE-A3, FE-A4, and FE-A5 are satisfied.
  The selected desktop consumer checkpoint is accepted; M4 remains `Active`
  for remaining operations and complete claim evidence. XR-S1 remains
  `Verifying` for its full composed matrix. See the current checkpoint below;
  earlier dated entries preserve the evidence and rejected alternatives at
  their historical boundaries.

## 2026-09-05 — Download-to-catalog row association accepted

- The user-visible two-row regression failed before the fix (`shows one catalog
  row`: expected one, received two). Download progress now carries the exact
  destination-derived library ID through canonical list/status/push DTOs and
  the real preload. Recovery admission supplies the selected current catalog ID
  immediately. One uniquely associated activity overlays that catalog row;
  absent, cached-only or ambiguous associations remain separate.
- Review caught artifact-key association inheritance and activity collapse
  before the ambiguity guard. Both are corrected: distinct download IDs survive
  snapshot selection, optimistic admission and initial restore; same-ID updates
  retain exact controls. Repository/name/quant similarity grants no association.
- `pnpm --dir frontend test:run`: 113 files, 535 tests pass. Frontend types/lint,
  Electron build/tests (10), lint, producer/preload conformance (5), renderer
  conformance (5), and generated desktop-contract freshness pass. The renderer
  conformance uses actual Rust fixtures and bundled preload, including invalid
  pushed identity rejection and exact resume parameters.
- Both real Vite entry modes pass an isolated Electron 39.8.6/Chromium workflow:
  initial partial, recovery admission, paused push and reload each retain one
  catalog row without a separate activity label; resumed IPC names the exact
  producer download ID. A 50% partial is visible after push and restore. Native
  mouse input and captured pixels supplement DOM assertions. No renderer
  warnings/errors remained after correcting ancillary test fixtures.
- Temporary fixture window/profile and scripts are isolated under
  `/tmp/pumas-row-gui.L9RInV`; logs are `/tmp/pumas-row-gui{,-default}.log` and
  `/tmp/pumas-row-*.log`. No backend or user model files participate in that GUI
  test. The running operator app was not restarted. Permanent regression owners
  are the existing component/hook and producer/preload suites, not a new runner.
- Optimized backend and default frontend/Electron builds pass. This accepts
  only the admitted duplicate-row correction, not full M4/M5, packaged-platform
  claims, or the complete remediation program.

## Reports

- `reports/frontend-async-owner-inventory.md` — completed for M0-S1.
- `reports/renderer-harness-admission.md` — accepted for M1-S1.
- `reports/frontend-overlay-consumer-inventory.md` — accepted for M2-S1.
- `reports/launcher-root-recovery-consumer-evidence.md` — verifying XR-S1.
- `reports/renderer-contract-consumer-inventory.md` — pending Milestone 4.

## Slice Ledger

| Slice | State | Evidence | Notes |
| --- | --- | --- | --- |
| M0-S1 | `accepted` | [original evidence](#2026-09-03--m0-s1-installation-progress-owner-accepted) and [PRG-I12 repair](#2026-09-03--m3-s3a-status-reachability-terminal-retention-and-admission-order-accepted) | Requested-install polling now begins after backend admission; existing-install discovery remains immediate. |
| M1-S1 | `accepted` | [renderer-harness admission](reports/renderer-harness-admission.md) | Existing Electron/CDP selected; no dependency or permanent-tooling change admitted. |
| M2-S1 | `accepted` | [overlay consumer inventory](reports/frontend-overlay-consumer-inventory.md) | Bounded modal/popup population, non-members, owner policies, and two write-set discoveries recorded. |
| M2-S2 | `accepted` | [modal verification record](#2026-09-03--m2-s2-modal-module-and-consumer-migration-accepted) | Shared stack-aware lifecycle, six modal branch dispositions, page preservation, and old-hook deletion. |
| M2-S3 | `accepted` | [verification record](#2026-09-03--m2-s3-popover-module-and-consumer-migration-accepted-after-replan) | Consumer migration and shared cross-Module Escape arbitration accepted after red/green composition repair. |
| M2-S4 | `accepted` | [runtime record](#2026-09-03--m2-s4-representative-chromium-evidence-accepted) | Deciding Chromium accessibility/focus and outside-pointer evidence accepted. |
| M2-S2-F1 | `accepted` | [caller-test correction](#2026-09-03--m2-s2-f1-install-dialog-caller-test-corrected) | Program-approved test-only correction; no product-source change. |
| M3-S1 | `accepted` | [focused verification record](#2026-09-03--m3-s1-progress-and-terminal-outcome-semantics-accepted) | Named determinate progress and one atomic terminal outcome; caller-level M2/M3 set is green. |
| M3-S2 | `accepted` | [focused verification record](#2026-09-03--m3-s2-central-reduced-motion-policy-accepted) | One composition-root and CSS reduced-motion policy covers both variants. |
| M3-S3a | `accepted` | [repair evidence](#2026-09-03--m3-s3a-status-reachability-terminal-retention-and-admission-order-accepted) | Red→green manager/state/dialog public seams plus requested-install admission order accepted. |
| M3-S3b | `accepted` | [repair and runtime evidence](#2026-09-03--m3-s3b-popover-motion-and-terminal-semantics-accepted) | Exact Popover entry/exit repair, stale M2 caller correction, both real entries, full suite, and behavior docs accepted. |
| XR-S1 | `verifying` | [selected checkpoint](#2026-09-05--selected-desktop-consumer-checkpoint-accepted) and [historical recovery evidence](reports/launcher-root-recovery-consumer-evidence.md) | Source and Linux cold/warm GUI accepted; full startup/selection and two-entry first-visible matrix remains pending. |
| M4 selected consumers | `accepted` | [selected checkpoint](#2026-09-05--selected-desktop-consumer-checkpoint-accepted) | Catalog/FTS, ticket recovery, scoped display cache, and exact activity presentation; not all desktop consumers. |

## 2026-09-05 — Selected Desktop Consumer Checkpoint Accepted

- Reconciliation operation: `continue` this focused plan at Standards
  `1609c304`. Commit `2b081fba` integrates the selected desktop contracts and
  consumers; `2b9553a0` independently corrects backend artifact reclassification.
  This entry supersedes current provider/preload/uncommitted-source holds,
  not earlier dated observations or remaining milestone gates.
- Accepted boundary: generated Rust-owned catalog and FTS projections, exact
  ticket recovery, selected download responses, bundled sandbox preload,
  synchronous decoded startup bootstrap, and version-2 root-scoped display
  cache. Retired version-1 entries are discarded. Cached rows grant no model
  action or recovery authority; fresh related-model controls are retained.
  Scan/import records remain a distinct unmigrated contract.
- Activity presentation preserves unassociated download rows and their exact
  IDs; no repo/name/quant join is inferred. The deciding regression reproduced
  an inflated model count, then proved catalog-only counting and explicit
  download-activity status while retaining exact-ID resume controls.
- Accepted automated evidence: frontend 520 tests, types, lint, both builds;
  real-producer renderer conformance 2/2; producer-decoder conformance 5/5;
  generator 5/5; Electron 129 tests plus the separately enabled real sandbox
  preload oracle. The mandatory fixture-wrapper command is
  `pnpm --dir frontend run test:desktop-contract`; it uses actual Rust
  constructor output rather than consumer-authored authority.
- Deciding Linux/X11 built-app evidence: 83 catalog models, two labelled paused
  activities, nine partial badges with percentages, no Ready-to-finish state,
  no renderer errors, scrolling, and normal shutdown. Warm native window
  capture shows cached rows during refresh without stale controls. The
  collision correction retains 83 models and clean repeat startup.
- The [program verification record](../execution-ledger.md#2026-09-05--coordinated-desktop-contract-verification)
  owns the combined commands, results, private capture location, and data
  preservation evidence. The [collision record](../execution-ledger.md#2026-09-05--collision-correction-accepted-on-linux)
  owns the later backend-only correction; neither is a whole M4/XR-S1/M5
  acceptance claim.
- Remaining next frontend slice: classify and coordinate link-health,
  import-picker, and conversion consumers with canonical providers; preserve
  raw import records and extend existing generation rather than handwritten
  mirrors. Whole FE-A1, full cache degradation/retry runtime evidence, and
  complete startup/two-entry matrices stay pending. Windows/macOS runtime
  evidence is platform-owned and does not block independent Linux source work.

## 2026-09-03 — M0-S1 Installation Progress Owner Accepted

- Operation: continued the active focused plan after the program owner released
  frontend source work on RUST-A1 acceptance.
- Behavior: `useInstallationManager` is the sole installation synchronization
  Module. It self-schedules only after a request settles, serializes across
  superseded app/tag generations, ignores late disabled/unmounted completions,
  derives resumed work from release hints, and owns app-scoped cancellation.
- Deleted machinery: dialog-local desktop polling, caller-managed progress
  refresh, and the available-version installation-tag callback chain.
- User outcome: cancellation rejection now remains distinct from successful
  cancellation and is visible in the affected install row.
- Design review: the dialog no longer knows the desktop progress/cancellation
  mechanism. The manager Interface gained the necessary cancellation action
  while losing the scheduling action; serialization, generation, polling, and
  normalization remain hidden. Removing the manager would redistribute those
  policies across callers, so the Module passes the deletion test.
- Focused integration evidence: `npm run test:run --
  src/hooks/useInstallationManager.test.ts
  src/hooks/useInstallationProgress.test.ts src/hooks/useVersions.test.ts
  src/hooks/useVersionFetching.test.ts
  src/hooks/useAvailableVersionState.test.ts
  src/hooks/useSelectedAppVersions.test.ts
  src/components/InstallDialog.test.tsx` passed 35 tests in 7 files.
- Supporting gates: full `npm run check:types` passed; full `npm run lint`
  passed before the final two test additions, and focused ESLint over every M0
  source/test file passed after them; `git diff --check` passed.
- Independent corroboration: governance ran the then-current full frontend
  suite successfully (102 files, 446 tests) before the last two focused
  regression cases were added; it does not replace the deciding focused
  deferred-request/fake-clock oracle.
- Historical acceptance: FE-A3 and M0 were accepted on this evidence. PRG-I12
  subsequently superseded that claim by exposing a missing admission-order
  case; the accepted M3-S3a repair re-satisfied the claim.

## 2026-09-03 — M1-S1 Renderer Harness Admission Accepted

- Operation: completed the planned bounded experiment, accepted its report,
  revised the later exact harness write set, and explicitly started M2-S1 as a
  report-only consumer inventory slice.
- Subject: a production library-only Vite bundle and compiled production
  preload running in Electron 39.8.6 / Chromium 142 with isolated state and
  deterministic desktop-operation fixtures.
- Deciding surfaces: Chromium accessibility tree, actual DOM focus, browser
  input dispatch, and CDP media emulation.
- Reachable failure: the current import overlay exposed no dialog role/modal
  state, did not receive focus or dismiss on Escape, and did not restore focus.
- Runtime: build 8.29 seconds; corrected workflow 1.554 seconds; no surviving
  Electron process after exit.
- Decision: existing Electron/Vite/Node tooling plus one small custom runner;
  no dependency. See the [admission report](reports/renderer-harness-admission.md)
  for cleanup, limits, owner split, and comparison to Vitest/release smoke.
- Acceptance: the Milestone 1 stopping condition and gate are satisfied.

## 2026-09-03 — M2-S1 Overlay Consumer Inventory Accepted

- Operation: completed and accepted the report-only inventory slice; stopped
  before shared overlay source mutation pending program release.
- Population: six modal families (including page/dialog branches and nested
  confirmations), three popup families, and searched nearby non-members.
- Semantics: popups are named non-modal action dialogs, not listboxes or menus;
  feature content/actions remain outside the shared lifecycle Modules.
- Re-plan: added `model-serve/ModelServeDialogContent.tsx` so stale dialog-ref
  plumbing can be deleted with the old focus hook, and added the missing
  `RemoteModelListItemActions.test.tsx` opener-contract evidence file.
- Follow-up: in-flow link-health/report disclosures do not share overlay
  lifecycle and are routed to FE-I12 rather than widening M2.
- Evidence: [overlay consumer inventory](reports/frontend-overlay-consumer-inventory.md).

## 2026-09-03 — M2-S2 Modal Module And Consumer Migration Accepted

- Operation: implemented the admitted modal-only source slice, accepted its
  focused evidence, and stopped before Popover work.
- Module Interface: `ModalDialog` owns the portal, dialog/alertdialog state,
  initial focus, Tab containment, topmost Escape, backdrop policy,
  dismissal-disabled state, cleanup, and stack-aware restoration. Feature
  consumers retain their titles, content, actions, and backdrop choice.
- Nested behavior: the real install-frame plus confirmation test proves that
  Escape closes only the confirmation, restores its install-dialog opener,
  then closes the parent and restores the original page opener. The primitive
  also rewires restoration past a parent and opener removed with the nested
  hierarchy.
- Migrated consumers: confirmation, modal install branch, model metadata,
  modal serving branch, model import, and HuggingFace authentication. Install
  and serving page branches remain non-modal and no longer receive modal
  autofocus.
- Deleted machinery: `model-serve/useDialogFocusTrap.ts` and its stale
  `dialogRef` plumbing in `ModelServeDialogContent`.
- Focused tests: seven files and 27 tests passed, covering the Module and every
  migrated family. Focused ESLint passed for all changed M2-S2 files, full
  TypeScript checking passed, the deleted-hook/ref sentinel passed, and
  `git diff --check` passed.
- Unsupported here: Chromium acceptance remains M2-S4 after the separate
  popup slice; FE-A4 is not yet satisfied by jsdom evidence.

## 2026-09-03 — M2-S3 Popover Module And Consumer Migration Accepted After Re-Plan

- Operation: implemented the program-released popup-only source slice and
  stopped before the M2-S4 representative Chromium workflow.
- Module Interface: controlled `Popover` owns the named non-modal dialog,
  trigger `aria-controls`/expanded/has-popup relationship, focus entry and
  return, topmost Escape handling, pointer-outside dismissal, and lifecycle
  listener cleanup. Feature consumers retain domain actions and state.
- Semantics: the three mixed-action collections remain dialog popups. No
  menu, listbox, or selection abstraction was introduced.
- Migrated consumers: version actions focus the active actionable version;
  model filters focus the selected filter; remote download options use either
  the primary action or the queue-another action as the truthful opener while
  cancellation remains a direct action.
- Preserved behavior: version management/default actions remain separate;
  filter selection still closes the controlled popup; remote detail hydration
  occurs only on opening, and grouped/quant/all-file download behavior remains
  owned by `RemoteModelDownloadMenu`.
- Deleted machinery: version-selector document pointer listeners, container
  ref, local dropdown animation shell, toggle-only trigger contract, and the
  download trigger's misleading pressed-state attribute.
- Focused integration evidence: ten files and 27 tests passed across the
  `Popover` Interface, all three migrated families, remote list-item/list
  callers, and remote summary behavior. The smaller direct Interface and
  consumer set passed 17 tests in seven files.
- Supporting gates: focused ESLint passed over every M2-S3 source/test file;
  full `npm run check:types` passed; old-lifecycle deletion sentinels and
  `git diff --check` passed.
- Review contradiction: independent modal and popup document-capture listeners
  each know only their local stack. When a Popover is opened inside a modal,
  the older modal listener receives Escape first and stops propagation, closing
  the parent before the newer popup.
- Re-plan: add only `ui/OverlayEscapeStack.ts` as the private cross-Module
  arbitration owner. First demonstrate the contradiction with a composed
  `ModalDialog` + `Popover` test, then route both Modules through the shared
  topmost policy and prove two-stage close/restoration. No feature consumer or
  M2-S4 runtime scope is added.
- Red evidence: the composed regression failed one of four Popover tests before
  repair because the first Escape removed `Composed dialog` along with its
  child popup.
- Repair: `OverlayEscapeStack` owns one document Escape listener and dispatches
  only to the most recently registered modal or popup layer. Modal Tab/focus
  containment and popup pointer-outside behavior remain inside their respective
  Modules.
- Green evidence: both Module suites passed eight tests in two files, then the
  complete migrated modal/popup and caller set passed 55 tests in 17 files.
  Focused ESLint over the shared owner and both Modules/tests, full TypeScript
  checking, and `git diff --check` also passed.
- Review: the program accepted the shared private Escape layer as the correct
  cross-Module Seam; containment and pointer-outside policies remain local.
- Acceptance boundary: this record does not satisfy FE-A4. The separately
  released M2-S4 Chromium oracle remains.

## 2026-09-03 — M2-S4 Representative Chromium Evidence Accepted

- Operation: built the production library-only renderer and a temporary
  composition fixture containing the actual `ModalDialog` and `Popover`
  Modules, then ran both in Electron 39.8.6 / Chromium 142 through the real
  compiled preload. No repository source or permanent tooling changed.
- Production modal oracle: the real model-import workflow exposed a DOM and AX
  dialog named `Import Models` with modal state true. Focus entered the close
  action, an attempted background focus was contained, Tab from the last
  actionable element wrapped to the first, and Escape closed the dialog and
  restored `Import models`.
- Production popup oracle: the model-filter trigger reported expanded true,
  `aria-haspopup="dialog"`, and `aria-controls` exactly matching the named
  `Filter by category` popup id. The AX node was a non-modal dialog, focus
  entered `All Categories`, and Escape closed and restored the trigger.
- Outside-pointer observation: reopening the filter and clicking the search
  input closed the popup and left final browser focus on `Search 0 models`.
  This is the intended policy: lifecycle cleanup may restore the opener during
  unmount, then Chromium's pointer default transfers focus to the user's target.
  A subsequent Escape left that focus unchanged, corroborating listener cleanup.
- Cross-Module oracle: sequential opening matches the current production
  consumer invariant. The actual Modules exposed named modal/popup AX dialog
  nodes; first Escape closed only the popup and restored `Open runtime popup`,
  while second Escape closed the modal and restored `Open composed workflow`.
- Result: the clean Electron workflow exited 0 in 6.74 seconds after a 7.38
  second production build and 3.50 second fixture build. Ten deterministic IPC
  calls completed, and renderer console output contained one expected info log
  with no warning/error.
- Cleanup: the debugger detached, BrowserWindow was destroyed, no matching
  Electron process remained, and `/tmp/pumas-m2-acceptance` (bundles, fixture,
  diagnostics, and isolated profiles) was deleted.
- Review: the program accepted the named modal/popup AX state, focus lifecycle,
  sequential nested ordering, browser-native outside-pointer focus outcome,
  and cleanup evidence. Milestone 2 and FE-A4 are satisfied.

## 2026-09-03 — M2-S2-F1 Install Dialog Caller Test Corrected

- Operation: used the program-approved test-only follow-up in
  `InstallDialog.test.tsx`; no product source or M3 write set was broadened.
- Discovery: the M3-S1 caller-level suite reached a stale assertion for the
  pre-M2 `Dismiss install dialog` control. The accepted M2 focused evidence
  exercised the migrated `InstallDialogFrame` Interface and modal families but
  did not include this higher-level orchestration test, whose M0 assertions
  predated the frame migration.
- Correction: the caller test now drives the public M2 behavior through the
  dialog backdrop and document Escape path, matching the accepted Module and
  representative Chromium behavior.
- Evidence: the combined `ProgressDetailsView`, `InstallDialogContent`, and
  `InstallDialog` suite passed 10 tests in three files. Focused ESLint over the
  corrected caller test and M3-S1 files passed, as did full TypeScript checking.

## 2026-09-03 — M3-S1 Progress And Terminal Outcome Semantics Accepted

- Operation: implemented only the admitted progress-view source/test slice,
  then stopped before composition-root and CSS motion work.
- Behavior: overall and current-stage progress expose named, clamped
  determinate progressbar values. Terminal installation state is projected to
  exactly one outcome: failure is assertive, while cancellation and success
  are polite. Incremental progress remains outside the live region.
- Red evidence: before the source change, both focused tests failed because no
  named progressbar or terminal status/alert role was present.
- Green evidence: `ProgressDetailsView.test.tsx` passed two tests, including
  failure, cancellation, success, and an identical-success rerender that keeps
  one stable status node. The caller-level M2/M3 suite passed 10 tests in three
  files after the separately recorded test-only correction.
- Supporting gates: focused ESLint over the M3-S1 source/test and corrected
  caller test passed; full `npm run check:types` passed.
- Review: the program accepted the clamped stable progress values and the
  single atomic terminal region with outcome-appropriate politeness.
- Acceptance boundary: representative accessibility-tree announcements and
  normal/reduced motion remain M3-S3, so FE-A5 is not yet satisfied.

## 2026-09-03 — M3-S2 Central Reduced-Motion Policy Accepted

- Operation: changed only the admitted composition-root and CSS policy files;
  stopped before representative runtime and documentation work.
- Framer Motion policy: one `MotionConfig` at the renderer composition root
  uses `reducedMotion="user"`, so both default and library-only application
  trees consume the operating-system preference without teaching feature
  components about media queries.
- CSS policy: one `prefers-reduced-motion: reduce` rule makes animation and
  transition duration effectively immediate, prevents repeated animation, and
  disables smooth scrolling across elements and pseudo-elements. The short
  duration preserves lifecycle completion events while suppressing visible
  repeated motion.
- Focused gates: ESLint passed for the composition root and M3 status source;
  full TypeScript checking and the two ProgressDetailsView tests passed.
- Build evidence: default and library-only production Vite builds passed in
  6.19 and 5.97 seconds respectively.
- Review: the program accepted the two central policy boundaries and the
  near-zero-duration/one-iteration lifecycle behavior.
- Acceptance boundary: browser media emulation must still demonstrate the
  computed normal/reduced difference and terminal accessibility tree in M3-S3.

## 2026-09-03 — M3-S3 Runtime Admission Stopped For Unreachable Status View

- Operation: inspected the real default and library-only entry paths before
  creating a temporary harness; stopped without runtime or README mutation.
- Contradiction: `useInstallationState` initializes and resets `viewMode` to
  `list`, while the complete production source population contains no call
  that sets it to `details`. `ProgressDetailsView` therefore has no production
  entry transition even though direct component tests can render it.
- Terminal loss: `useInstallationManager` retains unsuccessful terminal
  progress but clears successful completion; `InstallDialog` clears its local
  presentation tag for any terminal payload. Adding a view transition alone
  would still leave success unreachable and terminal failure/cancellation too
  short-lived for the existing presentation timer to govern.
- Oracle consequence: a temporary component fixture would prove Chromium can
  render the component, not that a user of either real entry can receive the
  status. It is rejected as manufactured evidence.
- Mode classification: installation UI is intentionally compiled out of the
  library-only entry, so its progress/outcome portion is not applicable. The
  central reduced-motion policy remains applicable to both real entries.
- Re-plan request: keep ownership in the current manager/dialog/state chain,
  retain all terminal outcomes long enough for one deterministic presentation,
  and provide a real transition into that presentation. Exact source/tests
  were accepted as `useInstallationManager`, `useInstallationState`, and
  `InstallDialog` with their colocated tests. Their public output, presentation
  state, and rendered dialog are the confirmed TDD seams; only desktop
  operations and time may be mocked.

## 2026-09-03 — M3-S3a Status Reachability, Terminal Retention, And Admission Order Accepted

- Operation: implemented only the accepted `useInstallationManager`,
  `useInstallationState`, and `InstallDialog` source/test slice and stopped
  before Chromium or README work.
- Manager behavior: the current lifecycle now retains normalized success just
  as it retains failure/cancellation, while success still refreshes version
  state and exposes idle network status. The existing superseded-app test now
  supplies a late successful terminal payload and confirms it cannot mutate the
  new lifecycle.
- PRG-I12 counterevidence: integration review found `installVersion` could
  start its progress read before `install_version` acknowledged the new
  backend lifecycle. A fast read of a prior terminal payload could invalidate
  the new generation and strand the subsequently accepted install. This
  reopens the earlier FE-A3/M0 claim until the correction is reviewed.
- Admission-order red evidence: with `install_version` held by a controlled
  deferred Adapter, the public hook called `get_installation_progress` once
  before admission when the expected count was zero.
- Admission-order repair: requested-install polling starts only after a
  successful backend response. Existing-release discovery still begins
  polling immediately because that lifecycle already exists. The focused
  regression observes no pre-admission read, then exactly one current-lifecycle
  read after success.
- Presentation behavior: a new installation identity enters details once when
  progress first appears; Back remains on the list through later active updates;
  the active-to-terminal transition enters details once more, and a later Back
  remains respected. Identity consists only of the installation tag/start time
  and active/terminal transition, hidden inside the state owner.
- Dialog behavior: auto-presented progress exposes both determinate bars. On a
  terminal payload the local tag remains until the existing outcome timer
  expires, allowing the single terminal live region to include the correct
  version before the view returns to the list.
- Red evidence: the manager regression received `null` instead of successful
  terminal progress; the two state regressions received `list` instead of
  initial/terminal `details`; and the rendered dialog could not find the
  terminal `status` after completion.
- Green evidence: the manager/state/dialog seams and their direct progress and
  content callers passed 31 tests in six files. This includes the corrected
  admission-order regression and the existing maximum-one-request,
  supersession, terminal-retention, presentation, and outcome-timer cases.
- Supporting gates: focused ESLint over all M3/FE-I13/PRG-I12 source/tests and
  full TypeScript checking passed. Sequential default and library-only
  production Vite builds passed in 2.76 and 2.58 seconds. `git diff --check`
  passed.
- Independent review: root reran 21 direct tests plus focused lint, types, and
  diff checking, confirmed the repair preserved immediate discovery of an
  existing lifecycle, and accepted FE-A3/M0 and M3-S3a.
- Acceptance boundary: real-entry Chromium and README claims remain M3-S3b;
  FE-A5 is not yet satisfied.

## 2026-09-03 — M3-S3b Runtime Stopped For Visible Reduced-Motion Displacement

- Operation: built both production Vite modes into isolated `/tmp` directories
  and ran their real entries with the compiled production preload in Electron
  39.8.6 / Chromium 142. Media emulation was applied from a neutral page before
  the production bundle's first evaluation.
- Accepted sub-evidence, not an aggregate acceptance: the default entry exposed
  exactly two DOM and AX progressbars named `Overall installation progress`
  and `Downloading progress`, with ranges 0–100 and values 37 and 64. Its
  terminal success exposed exactly one DOM `status` with polite live behavior,
  atomic true, and the retained `v0.22.1` tag; the AX status reported live
  polite and atomic true.
- CSS comparison in both entries: normal mode exposed a 0.15-second transition
  and a 1.2-second/infinite scan animation. Reduced mode exposed 0.01-millisecond
  duration, zero delay, and one animation iteration. Each comparison had a
  positive DOM observation.
- Framer counterevidence: timer-based computed-style sampling recorded 40
  samples per scenario. Normal default mode recorded visible translated entry
  frames. Reduced default mode still recorded `translateY(-6px)` at nonzero
  opacity before snapping to rest, even when the preference was applied before
  module evaluation. The earlier hidden-window result with zero reduced
  samples was rejected as vacuous rather than accepted.
- Consequence: central CSS policy is proven, but FE-A5's Framer movement claim
  is false for the representative Popover. README mutation and M3 acceptance
  remain unavailable pending a bounded source/test write-set re-plan and a
  green four-scenario rerun.
- Accepted re-plan: change only shared `Popover.tsx` and its colocated test so
  the operating-system reduced-motion preference selects zero entry and exit
  translation while normal mode retains `-6px`; require non-vacuous open and
  dismiss sampling. Root also admitted `ModelMetadataModal.test.tsx` only to
  replace its stale deleted-backdrop-label query and await initial async work,
  because that unchanged full-suite caller failed after the accepted M2
  migration.
- Repository boundary: no package, permanent harness, README, or motion source
  changed during the experiment. Temporary bundles, fixture, and profile state
  remain under `/tmp/pumas-m3-s3b` until the deciding repair rerun, after which
  they must be removed. Renderer console output had no application/module
  errors; Electron emitted its development-only CSP warning when the unpackaged
  `file://` bundle was used.

## 2026-09-03 — M3-S3b Popover Motion And Terminal Semantics Accepted

- Operation: implemented the admitted Popover-only motion repair and the exact
  stale metadata-modal caller correction, rebuilt both production entries,
  reran the strengthened real Chromium oracle, updated only accepted behavior
  claims in `frontend/README.md`, and stopped before M4.
- Focused red→green: under the reduced preference, the Popover test first
  received entry and exit `y=-6` where zero was required. `Popover` now uses
  Framer's operating-system preference hook to select `y=0` for both reduced
  entry and dismissal while retaining opacity and normal-mode `y=-6`.
  Popover plus metadata-modal caller tests pass eight tests in two files.
- Stale caller: the metadata-modal test now awaits initial metadata state,
  drives the aria-hidden `data-modal-backdrop` through `mousedown`, and sends
  Escape to `document`. This matches the accepted ModalDialog Interface and
  removes the prior async act warning; no product source changed for FE-I16.
- Motion runtime: all four default/library-only × normal/reduced production
  entry scenarios ran in Electron 39.8.6 / Chromium 142 with media emulation
  established before bundle evaluation. Every entry scenario recorded 40
  positive open samples and 17–18 positive dismiss samples. Normal mode showed
  14–16 visible translated entry frames and 13–15 visible translated dismiss
  frames. Reduced mode showed zero translated entry or dismiss frames.
- CSS runtime: both entries reported 0.15-second transitions and
  1.2-second/infinite scan animation in normal mode; reduced mode reported
  0.01-millisecond duration, zero delay, and one iteration.
- Status runtime: the default entry exposed two DOM and AX progressbars named
  `Overall installation progress` and `Downloading progress`, with ranges
  0–100 and values 37 and 64. Terminal success exposed exactly one DOM status
  with polite/atomic semantics and the retained `v0.22.1` tag; Chromium AX
  reported live polite and atomic true. Installation is not applicable to the
  library-only entry.
- Clean run: the deciding renderer workflow exited zero in 9.6 seconds with no
  renderer application or module console warning/error. The expected
  unpackaged Electron CSP diagnostic was separated from application output.
- Verification: full frontend lint and TypeScript checking pass; the full
  frontend suite passes 109 files and 473 tests. Fresh production builds used
  by the deciding run passed in 3.20 seconds default and 2.41 seconds
  library-only. `git diff --check` passed after documentation reconciliation.
- Cleanup: the debugger detached and every BrowserWindow was destroyed; no
  matching Electron process remained. The exact `/tmp/pumas-m3-s3b` harness,
  bundles, and isolated profile state were removed after the deciding run.
- Acceptance: root accepted FE-A5, M3-S3b, Milestone 3, FE-I15, and FE-I16 on
  the non-vacuous bidirectional four-scenario evidence. No M4 source or
  permanent runner/package change started.

## 2026-09-03 — XR-S1 Launcher-Root Recovery Consumer Verifying

- Operation: consumed only the accepted launcher-root startup/selection
  Interface in the exact frontend write set and stopped before M4 catalog or
  permanent renderer-runner work.
- TDD progression: public provider tests first failed on the missing owner,
  premature Electron child mount, absent persisted/explicit outcomes,
  StrictMode duplicate reads, missing sequential polling cleanup, incorrect retry/back
  actions, unobserved invocation rejection, overlapping selection, focus, and
  legacy direct selection. Each vertical slice was made green before the next.
- Module result: one composition-root provider owns the immediate startup read,
  sequential initializing polls, selection single-flight, cancellation
  restoration, typed terminal projection, and late completion cleanup. Browser
  mode is not applicable and renders children; Electron children do not mount
  until the decoded state is ready.
- Presentation result: one lightweight view supplies an accessible named
  region, polite atomic status, deterministic heading focus, exact action
  availability, and existing frameless minimize/close controls. Expected typed
  selection outcomes do not enter the console-error path.
- Platform counterevidence: the first composed run found the production
  sandboxed preload could not load a new relative CommonJS decoder import. The
  platform owner corrected that producer boundary, removed the duplicate
  decoder authority, and independently passed its real OS-sandbox oracle and
  full 77-test Electron gate before refreezing.
- First-frame counterevidence: after the preload correction, the hardened
  default run recorded one checking frame before content (22.8 milliseconds),
  while library-only recorded zero. Program review admitted only a provider-
  local atomic terminal commit; no timing threshold, App, index, platform, or
  manifest expansion was accepted.
- No-bridge counterevidence and repair: a real Electron renderer with a missing
  preload first mounted valid-looking backend content. A public red test then
  drove the provider to distinguish Electron identity from intentional browser
  mode. The repaired real renderer showed only focused `Desktop bridge
  unavailable`, made no root-state request, hid unsupported Minimize, retained
  working Close, and mounted no model-list content.
- Rejected timing claim: a provider-local `flushSync` experiment then produced
  zero checking frames in one default and one library-only sample, but review
  correctly rejected this as construction proof because an async IPC reply can
  lose the first paint.
- Handshake red/green: five of 23 provider tests failed before the renderer had
  commit notification, timeout subscription, and terminal-latch behavior. The
  initial provider repair passed 24/24, derives the closed signal type from the existing
  startup projection, never acknowledges initializing, acknowledges ready,
  recovery, or unavailable once from a layout effect, removes renderer-owned
  deadline/attempt policy, and ignores a late reply after the main watchdog.
  Focused lint and full TypeScript checking passed at that boundary.
- Compositor counterevidence: a real delayed-default Electron 39.8.6 run used
  the compiled sandboxed preload and `beginFrameSubscription` NativeImage
  presentation callbacks. It captured hidden Checking, but the latest frame
  synchronously available when the production owner called `showWindow` was
  still Checking after the renderer's proposed two-rAF delay. The harness
  rejected that run, and a deliberately early acknowledgement was also
  rejected. Waiting for an invalidated next hidden frame produced no callback
  under the production preferences. Renderer timing therefore cannot own the
  actual-presentation decision.
- Corrected renderer boundary: 3 of 27 provider tests failed after replacing
  the disproved rAF contract with direct terminal layout-commit expectations.
  Removing the delay made all 27 pass while preserving one-shot StrictMode
  notification, timeout replacement of resolved or in-flight ready state, and
  suppression of late results after unmount. Main now owns the separate in-
  frame compositor challenge before native reveal.
- Frozen frontend gates: 34 tests across the provider, recovery view, and
  window-action seams pass; the full frontend suite passes 111 files and 504
  tests; lint and TypeScript checking pass; fresh default and library-only
  production builds pass in 3.40 and 2.48 seconds. Composed runtime evidence
  remains pending the corrected platform owner.
- Composed verification gate: after the platform handshake freezes, the real
  oracle must cover delayed ready in both modes, watchdog unavailable,
  recovery, missing preload, and acknowledgement failure without a hidden
  hang. It must also pass all nine startup and nine selection values through
  the canonical producer, compiled preload decoder, and frontend semantics,
  while malformed/extra payloads fail before presentation.
- Claim boundary: neither prior timing samples nor the rejected two-rAF repair
  decide FE-A8. M5 still owns the
  admitted reusable renderer runner, packaged-target execution remains with
  the platform plan, and XR-S1 remains `Verifying` pending the composed gate.
