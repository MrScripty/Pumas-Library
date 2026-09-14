# M3b: local durable desired-state integration

Status: accepted within the local Linux evidence and explicit compatibility limits below.

## Native contract

The existing primary instance composes one `IntentService` over its library,
index, download client, and finite task owner. The public intent facade exposes
`ensure_model`, `release_model`, `get_ensure_status`, and `list_declarations`.
Requirements and outcomes remain transport-independent domain values.

An accepted ensure has committed its consumer-scoped declaration. Its observed
state can still be missing, unavailable, acquiring, or blocked. Duplicate
requests reuse the declaration; release/re-ensure uses a new generation. Release
never deletes bytes or cancels a download. Status remains a read-only observation.

Initial durable selector/policy pairs are local references with LocalOnly and
upstream repositories with AllowUpstream. First-time upstream binding requires
provider planning even when a related local artifact exists; an already-bound
available declaration can be observed offline. No source/provenance is guessed.

## Ordering and ownership

The store commit precedes upstream planning. Planning shares the canonical HF
metadata, selected-artifact identity, and destination calculation without
preparing an artifact directory. Immutable binding precedes download admission.
Generation reread, binding, and admission share a gate with release. Replay checks
the stored commit, artifact, and canonical target; policy changes that would
relocate the target block rather than repin it. Existing live or dormant download
custody is observed before admitting another attempt. Cold recovery resumes the
exact retained download through the existing HF owner only when durable state
proves it was queued or downloading before interruption. Deliberate pause,
pausing, error, and failed-integrity states do not authorize automatic resume.

Finite operations are registered before effects start. Requester cancellation
leaves the admitted operation owned. The shared runtime owner observes outer and
nested blocking joins and retains exclusion through their settlement. Domain
refusals are result values; actual storage/task failures remain owner failures.
`shutdown_intent` drains local effects and then downloads through a shared,
waiter-independent shutdown sequence. It does not stop inference runtimes.

## Reconciliation

Startup inspects durable declarations before entering the existing coordinator;
empty legacy stores do not force a new full-library pass. Ensure completion,
existing filesystem events, and canonical reconciliation feed the same owner.
Dirty events arriving during an active snapshot receive a coalesced follow-up.
Transient work has three automatic delayed wakes per unresolved episode:
30 seconds, 120 seconds, and 600 seconds. Explicit ensure, later events, or owner
restart can trigger a further observation after that automatic budget ends.
There is no additional scheduler, daemon, or persistent retry database.

## Destructive operations and compatibility limits

Primary composition shares the same existing download persistence with library
mutation guards even when HF is disabled. Native root grants exclude download
execution/admission while strict inventory checks cover dormant and hidden
custody. SQLite claims atomically exclude declarations. Claims remain after a
partially failed mutation; they do not expire or disappear through Drop.
Automatic recovery of unfinished deletion claims is not provided; retained
claims require explicit repair before affected paths can be mutated again.

Deletion and supported relocation use held directory capabilities. Metadata
publication reuses the existing atomic JSON publisher. Exact source/destination
identity checks and no-overwrite rename protect against path substitution.

- Standalone libraries without composed mutation authority refuse destructive
  work. Path-only merge refuses before opening/migrating its source.
- Composed relocation supports held, same-filesystem Linux rename. Unsupported
  platforms and cross-filesystem relocation refuse rather than copy/delete.
- Partial duplicate transfer and split-file migration preserve source data and
  report an unresolved/refused operation until capability transfer is supported.
- Constructor partial-file promotion is deferred to the configured download
  owner. Standalone library construction does not publish a pending artifact.
- A retained target that would require moving an older unknown partial can remain
  blocked by relocation retention. No acquisition-specific retention bypass is
  introduced.
- Historical binaries ignore declaration tables. Downgrade with live declarations
  or deletion claims remains unsupported.

Nodes, fleets, remote discovery, new listeners, MCP, and new networking features
remain deferred. Existing local transport projections remain M4.

## Verification

Final relevant verification passed **877 tests**: model-library suites 682,
intent 11, runtime task ownership 11, reconciliation 28, index 74, HF API 26,
public native intent 9, public desired state 5, public local crash 1, and relevant
operational API 30. Six ignored subprocess helpers are invoked by their parent
tests. Six unrelated runtime API cases remain excluded for the recorded I7
baseline failures; this is not a full runtime-suite claim.

The evidence includes real process exit after declaration commit, upstream
admission, and canonical publication; orderly restart; two-consumer retention;
manual-pause preservation; generation ABA rejection; same-pin repair after the
branch moves; root/path substitution; and absence of a duplicate payload writer.
Tests use real on-disk SQLite/filesystem state and controlled local HTTP; the
publication reopen gate works with its HTTP server stopped. No power-loss,
all-filesystem, all-platform, or general historical recovery claim is made.

Core library/tests/examples Clippy with warnings denied passed. Existing RPC
without default features and UniFFI Cargo checks passed. Exact-file Rustfmt and
whitespace checks passed. Review/test-driven fixes restored completed-pin
local-first resolution, closed the shutdown sequencing and missed-wake gaps,
restricted cold resume to interrupted active custody, and normalized the
existing importer review-reason producer so repair preserves canonical metadata.
