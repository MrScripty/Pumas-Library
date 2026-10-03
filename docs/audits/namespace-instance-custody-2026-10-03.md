# Instance custody across process and network namespaces

Status: confirmed unsupported ownership boundary; repair required before shared-store or networked deployment. This is a separate finding from the corrected GGUF enum/cache issue. No existing library was reclaimed or modified to establish this finding.

## Observed limitation

An approved Pumas-managed model acquisition finished transferring the expected bytes, but its enclosing execution ended before the supervisor wrote an observed child-exit receipt. The persistent instance row still identified that acquisition's owner. A later execution had a different process/network namespace. The earlier cleanup receipt belonged to a different startup attempt and could not prove this owner's cessation.

The current implementation cannot safely answer whether that historical owner has exited from the new namespace. A numeric PID may identify another process or no visible process there. Loopback identifies the observer's network namespace; an unreachable endpoint does not prove the old process is gone. A failed observer is not an empty ownership domain.

The original store remains preserved with ownership unresolved. A newly repaired binary cannot retrospectively prove the old process exited. The approved model's byte identity, the new inspector's source/native test qualification, and the old store's owner cessation are distinct evidence.

## Source trace

Reviewed main `dfc3d0f81f1a28db025bb776f66e3e9f5c6e9d55` and the PR25 qualification tree `e025953b16178f5d953780171fc43fe953b22526`:

- `rust/crates/pumas-core/src/registry/library_registry.rs`, `InstanceEntry`: PID, endpoint, connection token and wall-clock start time are stored; no independently verifiable creator/process-lifetime custody is retained in this record.
- `LibraryRegistry::try_claim_instance` observes `platform::is_process_alive(existing.pid)` inside the SQLite claim transaction. Atomic claim serialization prevents same-registry writers from both winning the transaction, but does not make a namespace-local PID observation globally authoritative.
- `rust/crates/pumas-core/src/platform/process.rs`, `is_process_alive`: Unix uses `kill(pid, 0)`. Its boolean result also collapses observation failures; it is not an identity- or namespace-scoped exit receipt.
- `LibraryRegistry::cleanup_stale` uses the same PID observation and pathname existence to remove instance records. It is invoked by local client/instance discovery. It must not be used as a diagnostic probe against this unresolved historical store.
- `rust/crates/pumas-core/src/model_library/download_recovery.rs`, `RootExecutionGrant`: the existing held-filesystem exclusion protects active mutations. Its documentation explicitly distinguishes an active grant from a configured root. It is not proof that a primary instance has retained an exclusive store lease throughout its complete lifetime.

## Required repair boundary

1. Design independently verifiable lifetime custody for a primary store owner, such as a supported held store lease bound to the physical store plus creator identity. Additional PID fields or failed namespace-local probes alone are insufficient.
2. Keep primary lifetime ownership distinct from finite mutation grants so composing the two cannot deadlock normal work or accidentally release the primary lease while a child still uses the store. Define inherited-handle, owner crash, dropped supervisor, process namespace and filesystem replacement behavior explicitly.
3. Observation permission/error/unknown identity must remain unknown, rather than authorize reclamation. Discovery must be read-only unless it holds the authority needed for the exact cleanup transition.
4. Fence any removal or replacement against the exact observed owner generation. Preserve startup-claim and ready-instance distinctions and do not delete a successor from a stale observation.
5. Legacy rows without independently verifiable custody require explicit reconciliation. New software must neither fabricate a historical exit receipt nor silently migrate/reclaim an unknown live store.
6. State supported filesystem/host boundaries. A local advisory lock is not automatically a safe distributed lease on every shared filesystem. No real-cluster qualification is available or claimed.

## Acceptance evidence required

- Deterministic competing-owner tests, including stale observer versus successor generation.
- Creator/observer namespace mismatch, reused numeric PID, inaccessible owner and failed endpoint observation, all without false reclamation.
- Lifetime lease retained through startup, finite effects, shutdown and observed child cessation; crash recovery only when the supported lease semantics prove the predecessor cannot continue.
- Physical-store/lock replacement refusal and explicit legacy unknown-owner behavior.
- Native Linux/macOS/Windows coverage for each claimed lifetime mechanism. Multiprocess or synthetic namespace evidence must be identified separately from a real networked/shared-filesystem deployment.

This audit does not authorize another process to start against the unresolved historical root, PID-based signalling, stale-row deletion, metadata overrides, or a claim that the approved real-model readiness test has completed. The narrow GGUF source fix can be accepted on its own qualified evidence while old-store reinspection remains blocked.
