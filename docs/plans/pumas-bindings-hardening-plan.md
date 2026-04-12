# Plan: Pumas Bindings Hardening

## Objective

Restore `pumas-uniffi` build correctness, add binding-surface verification
modeled on Pantograph, and add a minimal C# smoke and packaging path so Pumas
bindings are treated as a validated release surface rather than a one-shot code
generation script.

## Scope

### In Scope

- Fix `FfiDownloadRequest` to match the current `DownloadRequest` contract so
  `pumas-uniffi` compiles again.
- Add a UniFFI metadata/surface verification script for Pumas.
- Add a minimal generated-C# compile/runtime smoke harness for Pumas.
- Add a packaging script for Pumas native/C# release artifacts with manifests
  and checksums.
- Update binding and release documentation required to keep the new workflow
  discoverable and contract-traceable.

### Out of Scope

- Expanding Pumas to match Pantograph's full embedded-runtime/session API shape.
- Adding non-C# host-language smoke coverage in this slice.
- Changing Pumas core domain behavior unrelated to binding correctness.
- Redesigning all release artifact naming in one pass beyond what is necessary
  to make Pumas bindings standards-compliant and consumer-usable.

## Inputs

### Problem

`pumas-uniffi` has already drifted from `pumas-core` and currently fails to
compile because the FFI-side `FfiDownloadRequest` conversion no longer matches
the authoritative `DownloadRequest` fields. The current repo also treats
bindings as generated output without metadata verification, host-language smoke
coverage, or a consumer-ready packaging flow, which creates ongoing contract
drift risk.

### Constraints

- Follow `PLAN-STANDARDS.md`, `LANGUAGE-BINDINGS-STANDARDS.md`,
  `TESTING-STANDARDS.md`, `DOCUMENTATION-STANDARDS.md`,
  `TOOLING-STANDARDS.md`, and `RELEASE-STANDARDS.md` from the standards repo.
- Preserve the existing three-layer binding architecture:
  `pumas-core` -> `pumas-uniffi` / `pumas-rustler` -> generated bindings.
- Generated C# must remain generated-only and must not be hand-edited.
- Smoke verification should stay lightweight enough for local developer use and
  CI adoption.
- Packaging should separate the native product library from the generated
  host-language binding package by default.

### Assumptions

- The current Pumas product-facing native identity remains `pumas_uniffi` for
  this slice; product-facing renaming can be evaluated later if desired.
- `.NET SDK` and `uniffi-bindgen-cs` are acceptable optional local/CI
  dependencies for the C# smoke and packaging workflow.
- A minimal smoke that validates generated names, native loading, and one or
  two basic calls is sufficient for this slice; it does not need to exercise a
  model download path.
- Adding new scripts under `scripts/` and smoke sources under `bindings/csharp/`
  fits existing repo structure and standards.

### Dependencies

- `rust/crates/pumas-core/src/model_library/types.rs`
- `rust/crates/pumas-uniffi/src/bindings.rs`
- `rust/crates/pumas-uniffi/src/bin/uniffi_bindgen.rs`
- `rust/crates/pumas-uniffi/src/README.md`
- `scripts/generate-bindings.sh`
- `README.md`
- `RELEASING.md`
- New `bindings/csharp/` smoke/package support files
- Local tools: `cargo`, `uniffi-bindgen-cs`, `dotnet`, `zip`, `sha256sum` or
  `shasum`

### Affected Structured Contracts

- `pumas_library::model_library::DownloadRequest` field shape and its FFI
  projection.
- UniFFI metadata exported by `pumas-uniffi`.
- Generated C# namespace/type/method names consumed by foreign-language hosts.
- Packaging manifests describing which generated binding requires which native
  library.

### Affected Persisted Artifacts

- Generated binding outputs under `bindings/` or `target/` depending on the
  workflow.
- Release/package artifacts under a packaging output directory.
- Checksums and machine-readable manifests produced by packaging.
- Documentation describing artifact layout and compatibility expectations.

### Concurrency / Lifecycle Review

- No long-running polling, retry loops, or background daemons are introduced by
  this plan.
- Smoke harnesses must create and dispose UniFFI-owned objects cleanly so native
  library lifetime is deterministic.
- Packaging scripts must generate into disposable build directories under
  `target/` or another ignored output root to avoid concurrent edits to
  committed files.
- If future smoke coverage adds async callbacks or background state, ownership,
  shutdown, and overlap rules must be documented before expanding scope.

### Public Facade Preservation Note

- This plan is facade-first. Existing exported Pumas binding entrypoints should
  remain compatible unless a concrete bug requires a breaking correction.
- Any unavoidable public binding contract change must be documented in
  changelog/release notes and called out explicitly during implementation before
  merge.

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| FFI wrapper fix only patches the current drift and misses other latent contract mismatches | High | Add metadata verification and compile smoke in the same implementation slice so future drift becomes detectable |
| Packaging script accidentally treats generated code as checked-in source of truth | High | Keep generated outputs in `target/` for smoke/package generation; check in only harness/docs/templates |
| C# smoke becomes too coupled to unstable implementation details | Medium | Keep the smoke minimal and validate only stable surface names plus a tiny runtime call path |
| Release artifact names conflict with existing CI assumptions | Medium | Keep first slice additive, document artifact layout, and update release docs before wiring CI |
| New scripts become under-documented and drift from repo usage | Medium | Update module README/docs/release docs in the same slice and record regeneration/compatibility rules |

## Definition of Done

- `cargo check --manifest-path rust/Cargo.toml -p pumas-uniffi` passes.
- `FfiDownloadRequest` and any related FFI projection code match the current
  authoritative `DownloadRequest` contract.
- A Pumas metadata/surface script verifies key exported UniFFI items from the
  compiled native library.
- A minimal generated-C# smoke script generates bindings into ignored output,
  compiles a small C# harness, and runs it successfully against the native
  library.
- A packaging script creates separate native-library and C# binding artifacts
  with manifests and checksums.
- README/release/binding documentation explains regeneration, artifact
  separation, compatibility, and how to run the new verification steps.

## Milestones

### Milestone 1: Restore FFI Contract Alignment

**Goal:** Make `pumas-uniffi` compile again by re-aligning the FFI wrapper with
the current core contract and locking in the intended facade behavior.

**Tasks:**
- [ ] Update `FfiDownloadRequest` and its `From<FfiDownloadRequest>` conversion
      to include the current `DownloadRequest` fields or explicit defaults that
      preserve intended semantics.
- [ ] Review nearby FFI conversion code for similar contract drift while the
      affected area is open.
- [ ] Add or update focused Rust tests around the request conversion so future
      field additions fail in an obvious place.
- [ ] Confirm whether the new fields should be surfaced as stable FFI inputs or
      deliberately defaulted; document that choice in code/comments only if the
      reasoning would otherwise be non-obvious.

**Verification:**
- `cargo check --manifest-path rust/Cargo.toml -p pumas-uniffi`
- Targeted Rust tests for `pumas-uniffi` conversion behavior
- Optional: `cargo test --manifest-path rust/Cargo.toml -p pumas-uniffi`

**Status:** Not started

### Milestone 2: Add Binding Surface Verification

**Goal:** Add a fast Pumas metadata check that proves the compiled UniFFI
surface still exports the expected contract.

**Tasks:**
- [ ] Add a `scripts/check-uniffi-...` style script for Pumas that builds the
      library, prints UniFFI metadata, and asserts the presence of key exported
      records/functions/objects.
- [ ] Choose a stable set of metadata assertions that catch contract drift
      without overfitting to internal implementation details.
- [ ] Document where the script writes transient metadata output and keep that
      output under ignored build directories.
- [ ] Update relevant script and binding docs so developers know when to run
      the metadata check.

**Verification:**
- Run the new metadata check script successfully
- Re-run `cargo check --manifest-path rust/Cargo.toml -p pumas-uniffi`
- Manual review against `LANGUAGE-BINDINGS-STANDARDS.md` three-layer and
  generated-artifact rules

**Status:** Not started

### Milestone 3: Add Minimal Generated-C# Smoke Coverage

**Goal:** Prove that generated Pumas C# bindings compile and can load the native
library for a tiny stable call path.

**Tasks:**
- [ ] Add `bindings/csharp/README.md` describing smoke-only checked-in assets,
      generated-output policy, and constraints.
- [ ] Add a minimal C# harness under `bindings/csharp/` that uses generated
      bindings and exercises a stable Pumas call such as `version()` and one
      additional low-risk API path if available.
- [ ] Add a smoke script that builds `pumas-uniffi`, generates C# into
      `target/`, verifies expected namespace/type/method text, compiles the
      harness offline, and runs it with the native library on the loader path.
- [ ] Ensure the smoke does not require model assets, network access, or a full
      launcher environment.
- [ ] Fix current README/examples if generated namespace/class names differ from
      documented usage.

**Verification:**
- Run the new generated-C# smoke script successfully
- Confirm generated outputs remain outside committed source-of-truth paths
- Manual acceptance check per `TESTING-STANDARDS.md`:
  Rust producer contract -> generated binding -> compiled host harness ->
  native call success

**Status:** Not started

### Milestone 4: Add Packaging and Release Documentation

**Goal:** Package Pumas bindings as consumer-ready artifacts with documented
compatibility and release expectations.

**Tasks:**
- [ ] Add a packaging script that builds the native library, generates C# into a
      disposable package root, stages docs/examples/manifests, and writes
      separate native and C# packages plus checksums.
- [ ] Decide and document Pumas artifact naming consistent with release and
      language-binding standards, keeping native and generated artifacts
      separate by default.
- [ ] Add a package README and machine-readable manifests describing required
      native library, platform identity, and compatibility rules.
- [ ] Update `README.md`, `RELEASING.md`, and any binding module README files
      with regeneration, smoke, package, and compatibility guidance.
- [ ] Record whether follow-up CI wiring is required or intentionally deferred.

**Verification:**
- Run the packaging script successfully and inspect produced manifests,
  checksums, and artifact layout
- Manual review against `RELEASE-STANDARDS.md` and
  `LANGUAGE-BINDINGS-STANDARDS.md` product-native artifact rules
- Manual doc review against `DOCUMENTATION-STANDARDS.md` host-facing and
  structured-producer contract requirements

**Status:** Not started

## Execution Notes

Update during implementation:
- 2026-04-12: Plan created after comparing `Pumas-Library` bindings against
  `Pantograph` and confirming `pumas-uniffi` currently fails to compile because
  `FfiDownloadRequest` drifted from `DownloadRequest`.

## Commit Cadence Notes

- Commit after Milestone 1 once the binding crate compiles and targeted tests
  pass.
- Commit after Milestone 2 and Milestone 3 together if the metadata check and
  C# smoke share helper changes and are verified as one logical slice.
- Commit after Milestone 4 once packaging/docs are verified and artifact layout
  is stable.
- Follow commit format/history cleanup rules from `COMMIT-STANDARDS.md`.

## Optional Subagent Assignment

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| None planned | N/A | N/A | N/A |

## Re-Plan Triggers

- The required FFI fix changes public binding semantics instead of being a
  backward-compatible repair.
- The generated C# surface reveals additional namespace/type drift that
  requires broader facade cleanup.
- Packaging needs a product-facing native rename rather than an additive first
  pass.
- CI or repo tooling constraints make `dotnet`-based smoke verification
  infeasible in the intended environments.
- Documentation standards reveal missing module README/ADR work larger than a
  normal same-slice update.

## Recommendations

- Recommendation 1: Keep the first packaging iteration additive and
  standards-compliant rather than bundling a product-native rename into the
  same slice. This reduces migration risk and keeps the bug fix, verification,
  and packaging work reviewable.
- Recommendation 2: Prefer `target/` for generated smoke/package outputs and
  keep checked-in `bindings/csharp/` content limited to harness/docs/templates.
  This best matches the language-binding standards and reduces generated-source
  drift.
- Recommendation 3: Treat the metadata script and C# smoke as required
  regression guards for future binding changes, not optional developer helpers.
  That is the shortest path to preventing a repeat of the current drift.

## Completion Summary

### Completed

- None yet. Plan only.

### Deviations

- None yet.

### Follow-Ups

- CI wiring for the new scripts may be handled in this implementation if small,
  or as an immediate follow-up if repo CI changes materially expand scope.
- Additional host-language smoke coverage can be evaluated after C# is stable.

### Verification Summary

- Comparison review completed against Pantograph binding workflow.
- `cargo check --manifest-path rust/Cargo.toml -p pumas-uniffi` currently fails
  on `FfiDownloadRequest` drift and serves as the baseline defect.

### Traceability Links

- Module README updated: N/A yet
- ADR added/updated: N/A yet
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: N/A yet

## Brevity Note

This plan stays concise by default and expands only where execution order,
contract stability, or release-surface risk affects implementation decisions.
