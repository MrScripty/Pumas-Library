# Acquired output publication-identity proof repair

Isolated branch `fix/acquisition-publication-proof-ea1c4ae7` preserves frozen
reconciliation `ea1c4ae75a8a6d4719a95f31c684e1d0398acbb8` and accepted main.
Its composition base `33cbb05344aac5d5a4864a3fe270287ca0e90274` reproduces reconciliation tree
`736424cfd86f4c3d24ce268718ebec21e5e9613e` exactly. Full file-set counterpart
`159fd371d7669e53959e874ffc7f989a66f999b0` preserves 772294c1 and the same
accepted main separately; no file-set implementation is added to this branch. Reconciliation ea1c4ae7,
dispatch e1ee893f and accepted reader histories are preserved. Accepted main `73fec2e06c75ddae8c0e58be6ffcf352923376bd` is now verified locally:
tree `b449f2c88d3e34a9cfa16dd14c778d6b802c8495`, ordered parents
`96c2dca97fad673a2735f9f778013067569fe692` and
`e1ee893ff67752bdc839a067f15519f25a323c1a`. A direct SHA fetch initially failed
authentication. The subsequent normal repair push and branch fetch succeeded
through existing access, without any credential/settings change. The exact
accepted-main merge identity is preserved in this repair composition. The isolated
composition first reproduced the frozen reconciliation tree exactly; the bounded
guard, controls and evidence changes followed. The full counterpart's main
composition preserved its already qualified runtime source and changed only docs.

## Demonstrated defect and bounded owner repair

Independent review identified that acquired output reconciliation reused legacy
readiness helpers. Those helpers intentionally return true when canonical/indexed
metadata lacks `import_publication`. Equality of two absent/null projections passed;
the held receipt helper then returned before Confirmed state, physical root identity
and payload verification. A matching version-2 acquisition binding alone could
therefore adopt changed or missing output. Prior positive test counts are accurate,
but their acquired-output proof claim was incomplete and is superseded here.

Actual runtime reproduction on unchanged 772294c1 production, with the exact
new single-file fixture patch, failed `identity-bytes unexpectedly settled`:
both publication identities were removed, the original matching v2 acquisition
receipt was retained, and same-length model bytes were changed after confirmed
publication. The initial build never reached runtime because the linker terminated
with SIGBUS under disk pressure. After retiring only known completed task metadata,
the rerun reached runtime and reproduced the incorrect settlement. Both logs and
exact pre-repair test-only patch/SHA accompany handoff. No expected test failure
is reported as a passed check.

The acquired-only reconciler now requires a canonical nonmissing publication
identity and explicitly matching indexed field before it may reach the held
Confirmed/root/payload proof. Missing and explicit null refuse, including canonical-
only and index-only loss. The generic legacy readiness and held receipt helpers
remain byte-for-byte unchanged. No output/acquisition format version, migration,
publication writer, worker/retry owner, cleanup or task-lifecycle change occurs.
Uncertain output remains retained; only the existing acquisition consumer may
settle a proven exact generation. The same acquired observer guard also protects complete bundles in the separately
qualified full-set counterpart (33 acquisition/import cases, 49 total).

## Qualification and limits

Cold controls mutate disposable evidence after completed publication and before
fresh same-process owner construction. Canonical/indexed projections are set
explicitly after startup when needed; refusal is compared against post-startup
baselines. Missing/null identities with changed or missing primary payload,
canonical-only loss and index-only loss must return typed recovery-required, retain the original
Using row and exact acquisition/store/input/output/metadata/index evidence, and
make no source replay. Successful bound single-GGUF cold controls remain in the
same focused target. The full-set counterpart separately covers missing/null
bundle identities with changed auxiliary bytes. Ordinary version-1 copied output is unchanged; an explicit legacy primary/index
projection without identity or receipt remains Ready through a cold read-only
selector. In the full variant, two added legacy-control runs each had 32 passes/one fixture failure:
the first omitted the cached index projection, and post-startup index mutation
still left the effective metadata cache. The fixture now establishes both legacy
primary/index projections before cold startup, without changing production legacy
behavior. Both intermediate failed logs are retained.

Final isolated local qualification passes 23 acquisition/import tests plus 11
reader and five HTTP cases (39 focused runtime tests), strict enabled-S3 all-target Clippy,
actual headless compile, 12 feature contracts, canonical attribution/dependency
ownership, formatting and 20 release/workflow tests. Exact command results are
recorded with handoff. Locked/offline Linux x86_64,
Rust 1.92, existing shared target, jobs=1, task-local XDG config and package-local
profile overrides fit constrained storage. Only identified completed/failed task
outputs are retired with size/SHA journals; unrelated work and targets remain.
No full/default ONNX suite, hard SIGKILL/power loss, deployed migration,
cross-platform/native execution, live providers/credentials, inference or
application source workflow is claimed. Hosted acceptance and independent review
of the repair remain coordinator-owned. AQ-S3 remains not ready.
