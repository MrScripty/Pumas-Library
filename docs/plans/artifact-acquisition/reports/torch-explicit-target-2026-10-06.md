# Explicit wheel target prerequisite — 2026-10-06

The smallest production-facing slice is a strict versioned target data owner,
optional actual-wheel preflight/native-consumer validation, and materialization
of that trusted helper. The offline experiment uses the same contract throughout
markers, Requires-Python, ABI/platform compatibility and selected closure. It
cannot fill incomplete targets from the inspection host or treat unknown OSes as
Linux. The experimental resolver is not adopted into production; automatic/
preview P1, catalog authority and all AQ gates remain open.

## Exact source and executor continuity

Branch `feat/torch-explicit-target-cdbeabe6` starts at
`cdbeabe638ca82d6706fc8618c119ff3079fe589`, tree
`f56826efc9d781bf5776a71983c10631109707b5`. Tested source milestone:
`6a6881009da788356269f76a1b0497fa70e7a926`, tree
`7cf43f459e343e90e349b88e8584ca6b65ca8dcc`.

The parent reported an executor disconnection during the Rust command. This same
executor continued to answer tools; its original session 84437 joined successfully.
No active test was restarted, environment switched, old seed restored, or
authentication/security setting changed. Current dependency graphs passed 33
before the first build. Cargo ran offline/locked, one job, under the shared
serialized lock with an isolated /tmp target. No shared cache eviction was needed.

The frozen 9eda and independently accepted bounded cdbe experiments remain on
their original branches, with original evidence unchanged. Main 5e114f6d,
composition a1da98, finite 6308, production resolver/catalog source, dependencies,
locks, recipes/pins and native/shared cleanup remain unchanged. No PR/main merge,
external reviewer contact, provider/account credential or paid service operation.
The only Rust edit materializes the new trusted Python helper beside the existing
scripts. No applicable filesystem AGENTS/skills were found.

## Reused contracts and production-facing interfaces

Contract section 11 already separates accepted package resolution, shared payload
acquisition and exact local consumption. The existing `native_target` restricts
Torch to Linux/x86_64, Windows/x86_64 and macOS/arm64 and requires CPython;
these platform boundaries are reused. Existing public standalone packaging
tooling, Requires-Python checks, complete marker/extras closure and actual
METADATA/WHEEL identity remain the semantic owners. Qualified recipe selection
and lock bytes do not change.

The new [wheel target contract](../../../contracts/wheel-target.md) and
[`wheel_target.py`](../../../../torch-server/wheel_target.py) implement
`pumas.wheel-target.v1`. An explicit target requires full stable CPython patch
version, normal GIL ABI, known OS/architecture, Linux libc or macOS deployment
policy, explicit native-Linux-tag policy and **all eleven** runtime marker
environment values. Identity/OS/Python contradictions, missing/extra fields,
unknown libc/architecture, minor-only Python, debug/free-threaded ABI and
unsupported policy refuse with bounded diagnostics.

Construction consumes only supplied data. Public tag generation receives
explicit Python/ABI/interpreter/platform parameters, avoiding host ABI/debug/GIL
and platform defaults. Standard glibc/musl floor and alias policy plus explicit
public macOS tag generation produce complete label compatibility for this stated
scope. Complete supplied marker values override every packaging runtime default.
`capture_native()` is separate and must run in the **selected interpreter**; it
reuses that interpreter's public markers/native tags and refuses an unsupported
or ambiguous native observation. Tests show the normal Linux native tag set is
unchanged, including legacy/native tags.

Additive Python APIs:

- `WheelTarget(document)`, `to_dict`, `marker_environment`, `tags`, `supports`,
  `allows_python`, `require_native_consumer`, and explicit `capture_native`.
- `local_requirements(..., wheel_target=document)` applies the same complete
  target to acquired filename/WHEEL tags, Python requirements and actual metadata
  dependency closure.
- `install(..., wheel_target=document)` additionally revalidates complete native
  markers/ABI and the compatible tag set before stage creation or pip.
- Optional CLI resolution field `wheel_target` activates these checks; an
  explicitly present null/incomplete target fails. Absent fields retain existing
  selected-native consumption and finite-recipe behavior.

No automatic/preview caller is switched to a new resolver or mandatory target
policy. No public Rust/RPC DTO, source identity, retry/verification or durable
acquisition schema changes. Future production integration still needs an admitted
target observation/provenance handoff from the actual selected interpreter.

## Shared experimental target and explicit unsupported projections

The experiment's `context` now requires a validated target document. Its actual
wheel eligibility, full marker environment, closure and independent selected
packet all use that same declared target. The CLI receives the **full** patch
version and exact qualified platform/libc projection. Synthetic fixtures declare
3.12.7 and 3.12.14 explicitly; these are test data, not inferred patch defaults.
Direct-root artifact bindings and original source URL/hash declarations are
unchanged.

Projection schema advances to `pumas.experiment.offline-input.v2`, carrying the
whole target and canonical SHA-256. The experiment-decision receipt payload also
retains that target/hash with the selected original identities. This does not
create a runtime installation receipt or authorize source access.

The pinned uv CLI cannot express every complete target policy. The experiment
therefore refuses musl and macOS deployment projections, unlisted exact glibc
floors, and exclusion of native Linux tags instead of substituting generic
platforms. Public normalized marker tokens containing platform_release or
platform_version are refused even in inactive branches: those values cannot be
overridden by the current CLI. Dependency-group marker contexts also refuse.
Module-level metadata/preflight supports declared musl and macOS policies; that
does not imply a qualified resolver projection or actual foreign installation.
Unknown/incomplete targets refuse, never become Linux. The fixture harness still
acquires its explicit authorized object set before actual-byte preflight; future
catalog admission must validate/bind target authority before source enumeration.

## Measured tests

| Evidence | Result |
| --- | --- |
| New target/consumer tests | 11 pass: host-poisoned markers/ABI probes, complete/invalid targets, ABI3 and ABI rejection, libc floors/aliases, macOS deployment/universal2, explicit preflight, foreign/null CLI refusal, actual native local install and observed native parity |
| Existing local consumer | 18 pass, including actual local report/RECORD and ambient config isolation |
| Existing finite qualified catalog | 13 pass; selection, exact original lock/direct roots and no source fallback unchanged |
| Actual shared Rust acquisition/local-consumer/publication handoff | 1 pass; verified files, scripts, lease/receipt and settlement preserved |
| Feature graphs | 33 pass before builds |
| Expanded public offline uv controls | 36 cases / 341 assertions pass |
| Exact direct-root controls | 6 controls / 50 assertions pass against the updated target context |
| Scoped checks | Strict app-manager Clippy with test-support/tests, Ruff, Rust format, Python syntax and diff pass |

The two patch-version cases select the corresponding Python-full-version and
implementation-version leaves; a >=3.12.8 candidate is excluded at 3.12.7 and
included at 3.12.14. glibc 2.17 selects the 2.17 wheel while 2.28 admits the 2.28 wheel.
Existing Windows/Linux markers and CPython/platform wheel selection pass, with
no inspection-host fallback. Invalid target, missing markers, debug ABI,
unprojectable libc/deployment and kernel/build marker cases refuse before uv.
All URL/VCS, source substitution, config/cache, ambiguity, no-fallback and receipt
regressions remain passing. Both admitted same-name/version direct-root wheels
retain their original identities; copied lock substitution remains rejected.

Authoritative CLI/checker runs total 42 cases and 391 assertions, zero failures.
Their sixteen traced public compiles, including cache priming, observe no IP
socket/operation or package-tool/build-hook/installer invocation. Package installs
in the separate production-consumer tests use only existing generated tiny local
fixtures; no real Torch/GPU/provider or backend is installed/executed. Syscall
observation is not an OS network-denial claim. Windows/macOS/musl vectors qualify
contract/tag behavior only, not foreign runtime/native-platform acceptance.

[Target evidence](torch-explicit-target-2026-10-06/target-evidence.json),
[direct-root evidence](torch-explicit-target-2026-10-06/direct-root-evidence.json),
[source/tool provenance](torch-explicit-target-2026-10-06/provenance.json) and
[raw archive](torch-explicit-target-2026-10-06/raw-evidence.tar.gz) retain exact
commands, declared targets, actual bytes, projection/target digests, public locks,
source/syscall records and receipts. Logs include the original joined Rust test,
strict Clippy, Python groups and final CLI controls. Archive: 1730 members,
382737 bytes, SHA-256
`9519813d3557c865c2146ef39b024f255063196da9f50484a8b3d22d175e33cf`.

## Remaining upstream decision and next existing-plan work

Parent still owns the upstream catalog-authority decision: which index/object
universe and redirects are approved, how complete bounded enumeration is proven,
what candidate-byte/time budgets mean, and how omissions/unavailability refuse
without online/source fallback. A declared target is not catalog authority.
Automatic/preview adoption requires a separately admitted target-bound accepted
packet/consumer integration, complete public-tool projection qualification and
actual supported-platform/runtime evidence. Existing pip source-preparation P1
is not closed by this contract prerequisite. No new production uv dependency,
private hook or finite-recipe replacement is introduced.
