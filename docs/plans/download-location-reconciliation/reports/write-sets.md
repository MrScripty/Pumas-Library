# Exact milestone write sets

Authority: [plan](../plan.md). Paths are repository-relative. These are the
allowed files, not an instruction to change every file. No directory glob grants
production scope. Re-plan before adding another production file or dependency.
M1 creates the lasting contract/ADR once, with the plan as execution authority;
later slices amend that owning documentation rather than creating rival rules.

## Shared execution documentation

Every milestone may update these exact existing planning files for material
state/evidence changes:

```text
docs/plans/download-location-reconciliation/plan.md
docs/plans/download-location-reconciliation/execution-ledger.md
docs/plans/download-location-reconciliation/issues.md
docs/plans/download-location-reconciliation/reports/design.md
docs/plans/download-location-reconciliation/reports/acceptance-matrix.md
docs/plans/download-location-reconciliation/reports/write-sets.md
docs/plans/download-location-reconciliation/reports/verification.md
docs/plans/download-location-reconciliation/reports/standards.md
```

New evidence may be appended to the following exact reports by their milestone:
`reports/m1-evidence.md`, `reports/m2-evidence.md`, `reports/m3-evidence.md`, beneath
this plan directory. Untracked private logs may stay in ignored build/evidence
output directories; do not commit user paths, credentials, raw live stores or
model payloads. Evidence reports reference only retained, reviewable evidence.

## M1 — core-to-desktop recovery

```text
rust/crates/pumas-core/src/model_library/mod.rs
rust/crates/pumas-core/src/model_library/download_resolution.rs
rust/crates/pumas-core/src/model_library/download_resolution/tests.rs
rust/crates/pumas-core/src/model_library/download_store.rs
rust/crates/pumas-core/src/model_library/mutation_authority.rs
rust/crates/pumas-core/src/model_library/hf/download.rs
rust/crates/pumas-core/src/api/hf.rs
rust/crates/pumas-core/src/api/state_hf.rs
rust/crates/pumas-core/src/acquisition/store.rs
rust/crates/pumas-core/tests/download_location_workflow.rs
rust/crates/pumas-rpc/src/contract.rs
rust/crates/pumas-rpc/src/contract/export.rs
rust/crates/pumas-rpc/src/handlers/mod.rs
rust/crates/pumas-rpc/src/handlers/models.rs
rust/crates/pumas-rpc/src/handlers/models/downloads.rs
rust/crates/pumas-rpc/tests/download_location_resolution.rs
electron/src/rpc-method-registry.ts
electron/src/preload.ts
electron/tests/preload-rpc-contract.test.mjs
electron/tests/ipc-validation.test.mjs
electron/scripts/desktop-contract-conformance.test.mjs
electron/src/generated/desktop-contract.ts
electron/src/generated/desktop-contract.validators.js
electron/src/generated/desktop-contract.validators.d.ts
frontend/src/generated/desktop-contract.ts
frontend/src/generated/desktop-contract.validators.js
frontend/src/generated/desktop-contract.validators.d.ts
frontend/src/types/api-bridge-models.ts
frontend/src/api/models.ts
frontend/src/hooks/useModelDownloads.ts
frontend/src/hooks/useModelDownloads.test.ts
frontend/src/components/ModelManager.tsx
frontend/src/components/RetainedDownloadFailures.tsx
frontend/src/components/RetainedDownloadFailures.test.tsx
frontend/src/components/LocalModelsList.tsx
frontend/src/components/LocalModelRow.tsx
frontend/src/components/LocalModelRowActions.tsx
frontend/src/components/LocalModelDownloadActions.tsx
frontend/src/components/LocalModelDownloadActions.test.tsx
frontend/src/components/DownloadLocationResolutionDialog.tsx
frontend/src/components/DownloadLocationResolutionDialog.test.tsx
docs/contracts/download-location-reconciliation.md
docs/contracts/artifact-acquisition.md
docs/adr/0003-download-location-reconciliation.md
docs/ARCHITECTURE.md
docs/DEVELOPMENT.md
rust/crates/pumas-core/README.md
```

Preserve existing public status variants and host bindings. New DTO generation
uses the current canonical RPC declaration and generator; never hand-edit the
six generated artifacts. If required generator semantics are unsupported, record
the finding and revise the write set before changing the generator itself.

Focused gate: C2/C3/C5/C7 and local resolution portions of C4/C6, with ordinary
core/IPC/frontend tests and an actual developer desktop recovery path. C5's
smallest real source/retained-reader observation is
the dependency gate before UI expansion. Block only dependent implementation if
that representation fails. Evidence must show one durable publication, preserved
unrelated state, no forged receipts and immediate UI projection convergence.

Re-plan on any new durable owner, payload/model/index write, public-binding shape
change, new dependency or required unlisted registration/schema output.

## M2 — composed custody and failure proof

```text
rust/crates/pumas-core/src/model_library/download_resolution.rs
rust/crates/pumas-core/src/model_library/download_resolution/tests.rs
rust/crates/pumas-core/src/model_library/download_store.rs
rust/crates/pumas-core/src/model_library/mutation_authority.rs
rust/crates/pumas-core/src/model_library/hf/download.rs
rust/crates/pumas-core/src/model_library/library.rs
rust/crates/pumas-core/src/model_library/library/migration.rs
rust/crates/pumas-core/src/model_library/merge.rs
rust/crates/pumas-core/src/model_library/importer.rs
rust/crates/pumas-core/src/model_library/importer/admission_tests.rs
rust/crates/pumas-core/src/api/reconciliation.rs
rust/crates/pumas-core/src/acquisition/store.rs
rust/crates/pumas-core/tests/download_location_workflow.rs
rust/crates/pumas-rpc/tests/download_location_resolution.rs
.github/workflows/build.yml
docs/contracts/download-location-reconciliation.md
docs/contracts/artifact-acquisition.md
docs/adr/0003-download-location-reconciliation.md
```

Prefer preserving the existing correct guards and adding missing composed proof.
Repair a sibling only if the bounded audit demonstrates the shared invariant is
violated. Tests use existing seams or private owner-local fault hooks; no new
public testing framework. Native CI additions run only the affected qualification
on existing supported runners. Do not add bypass flags or a second transfer owner.

Focused gate: C1/C4/C6/C8, existing acquisition/import/mutation/migration tests,
deterministic interleavings and real process/store restart. Each sibling's actual
entry points and disposition appear in M2 evidence. Re-plan if custody cannot be
established, proof requires broader mutation or supported native exclusion differs.

## M3 — delivery and acceptance

```text
scripts/acceptance/download_location_reconciliation.py
scripts/acceptance/test_download_location_reconciliation.py
docs/runbooks/download-location-reconciliation.md
CHANGELOG.md
RELEASING.md
```

The acceptance helper creates tiny authored disposable metadata/files and exercises
the actual packaged RPC operations; it neither opens nor mutates the live library.
Its tests prove fixture validity and precise observations. Existing package
verification owns install/extraction/provenance; this helper supplies only the
new missing workflow assertions and uses Python's standard library.

No package-version bump is assumed for a rebuilt review `.deb`. Record candidate
source/hash distinctly even if the package version is unchanged. If an actual
versioned release is selected, re-plan its exact manifests/lockfiles/attribution
write set before changing them. No tag or publication is implicit here.

Full gate: C1–C10, repository local QA in `RELEASING.md`, scoped standards review,
real installed/extracted package UI recovery, checksum and staged backend identity.
Build binaries/packages only in ignored repo output locations. The write set
does not authorize committing generated build output or repairing live metadata.

Re-plan if packaging must change executable composition, dependencies, release
versions or supported runtime/sandbox policy. A failing source test returns to
its owning milestone rather than gaining miscellaneous M3 production scope.
