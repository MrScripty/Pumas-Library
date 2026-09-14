# M3a: Authoritative declaration-store prerequisite

Status: accepted for the private storage prerequisite on 2026-09-12; full M3 remains unaccepted.

## Implemented boundary

The existing ModelIndex connection now installs a component-versioned private
intent schema in one immediate SQLite transaction. Writer connections use WAL
with FULL synchronization. Read-only opens do not migrate. Incompatible,
partial, or malformed authoritative state is rejected before ordinary index
initialization; no database replacement/rebuild is used as recovery.

Private declarations store a consumer-scoped canonical requirement ID, creation
UUID, original requirement, and optional typed bound target. Binding is immutable
and uses the creation UUID as its compare-and-swap token. Exact duplicates retain
their UUID; release and re-ensure create a new lifetime, preventing stale work
from binding or releasing a replacement declaration.

Local model declarations immediately retain their model ID. Upstream declarations
can remain unresolved until an immutable single-file recipe is bound. Deletion
claims and target retention decisions share immediate transactions; claims use
exact UUID tokens and survive restart without automatic expiry. Declarations
and claims have no foreign-key cascade from the searchable model catalog.

Every read uses one SQLite snapshot and strict schema/row validation. Canonical
JSON/IDs, model projections, UUIDs, policy/selector combinations, filename rules,
artifact identity, and declaration/claim contradictions are checked rather than
silently repaired or ignored.

This is a private storage foundation. It adds no public ensure/release method,
background reconciliation, acquisition admission, filesystem deletion protection,
or transport operation. Its supported private inputs are LocalModel/LocalOnly
and UpstreamRepository/AllowUpstream. Repository-based local-only retention still
needs a provenance-binding contract in M3b.

## Verification

Final verification used the compiled core test binary after:

```sh
cargo test --offline --manifest-path rust/Cargo.toml -p pumas-library --lib --test intent_api_tests --test api_tests --no-run
```

| Suite/filter | Passed | Limits |
| --- | ---: | --- |
| `index::` | 74 | Includes 11 new declaration-store tests; one ignored child helper is explicitly invoked by its parent process test |
| `intent::` | 7 | Native intent regression fixtures |
| `model_library::hf::` | 233 | Managed acquisition regressions |
| `api::hf::tests` | 25 | Native lifecycle and refusal fixtures |
| `intent_api_tests` | 9 | Public local intent integration |
| `api_tests --skip runtime --skip launch_llama_cpp` | 30 | Six unrelated runtime cases excluded for the existing baseline reasons |

**378 tests passed** on the final source. The focused new module separately
passed 11 tests before the final index-suite rerun. Controlled process/socket
fixtures ran with sandbox restrictions lifted where required; all state was
isolated in temporary roots.

Additional checks passed:

- `cargo clippy --offline --manifest-path rust/Cargo.toml -p pumas-library --lib --tests --examples -- -D warnings`.
- `cargo check --offline --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features -p pumas-uniffi`.
- Exact changed Rust files formatted with edition 2021; Markdown relative links
  and `git diff --check` passed.

New tests cover populated legacy migration and facts/governance preservation,
transactional migration failure, FULL on the actual writer connection, read-only
legacy behavior, corrupt/future/partial schema refusal, canonical identity and
policy validation, immutable binding, UUID lifetime exclusion, separate consumer
releases, independent-connection claim races, retained claims after reopen, and
projection clearing without authority loss.
The process test pauses inside the production commit path after INSERT/readback
and before COMMIT, or after the real commit returns, then the parent forcibly
terminates the child and reopens the database. It does not mirror production SQL
in a test-only transaction.

## Limits and next integration

Linux process termination is the tested crash model; this does not prove
power-loss or every filesystem/platform behavior. Historical binaries ignore
these tables and do not honor retention. Downgrade with live declarations or
claims remains unsupported; the new version guard protects schema-aware code.

M3b must integrate the [existing lifecycle owner](m3-lifecycle-design.md), the
[actual deletion/relocation paths](m3-deletion-inventory.md), pure immutable target
planning, startup reconciliation, bounded retries, and public ensure/release.
It must prove crash boundaries spanning declaration commit, acquisition admission,
and publication. M3a's store tests do not accept those cross-component guarantees.
The exact storage contract is in the [admitted design](m3-store-design.md).
