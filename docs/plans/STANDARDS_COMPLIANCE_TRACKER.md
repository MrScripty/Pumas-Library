# Standards Compliance Tracker

## Status
In Progress

## Audit Date
2026-02-27

## Standards Source
`/media/jeremy/OrangeCream/Linux Software/Coding-Standards/`

Reviewed documents:
1. `ARCHITECTURE-PATTERNS.md`
2. `CODING-STANDARDS.md`
3. `DOCUMENTATION-STANDARDS.md`
4. `SECURITY-STANDARDS.md`
5. `TESTING-STANDARDS.md`

## Scope
1. `rust/crates/*`
2. `frontend/src`
3. `electron/src`

## Baseline Findings
1. `RPC bridge stubs` (architecture/layering): multiple RPC methods returned placeholders instead of delegating to core.
2. `src README coverage` (documentation): 0 source directories currently missing `README.md` (closed; was 23 at baseline).
3. `file size target` (coding): 53 code files exceed 500 lines.
4. `TODO format` (documentation): 0 TODO comments missing owner/ticket/date context (closed).

## Workstreams

### WS-01: RPC Bridge Completeness (Priority P0)
Goal: Ensure GUI-facing RPC handlers call real `pumas-core` functionality and only adapt request/response shapes.

Completed:
- [x] Replaced `get_network_status` stub with core-backed response (`pumas-core` aggregation + RPC bridge).
- [x] Replaced `get_library_status` stub with core-backed response.
- [x] Replaced `refresh_model_mappings` placeholder with active-version mapping apply flow.
- [x] Replaced `validate_file_type` placeholder with core file type detection.
- [x] Replaced `sync_with_resolutions` placeholder with conflict resolution parsing + core apply.
- [x] Replaced `get_cross_filesystem_warning` placeholder with core filesystem check.
- [x] Updated RPC response wrapper so `refresh_model_mappings` is passthrough (preserves real success/error payload).

Follow-up:
- [ ] Add integration tests for `sync_with_resolutions` invalid action handling.
- [ ] Add integration tests for cross-filesystem warning bridge path.

### WS-02: Source Directory README Compliance (Priority P1)
Goal: Satisfy `DOCUMENTATION-STANDARDS.md` requirement that every directory under source roots has a `README.md`.

Missing READMEs (0):
1. None.

Plan:
- [x] Add minimal README set for Rust crate `src/` trees first.
- [x] Add README set for `frontend/src` subdirectories.
- [x] Add README for `electron/src`.

### WS-03: File Size Refactors (Priority P2)
Goal: Move toward 500-line target from `CODING-STANDARDS.md`.

Current baseline: 53 files exceed 500 lines.

Top offenders:
1. `rust/crates/pumas-core/src/model_library/library.rs` (3437)
2. `rust/crates/pumas-core/src/index/model_index.rs` (2615)
3. `frontend/src/types/api.ts` (1985)
4. `rust/crates/pumas-core/src/model_library/importer.rs` (1614)
5. `rust/crates/pumas-uniffi/src/bindings.rs` (1508)
6. `frontend/src/components/ModelImportDialog.tsx` (1358)
7. `rust/crates/pumas-app-manager/src/version_manager/installer.rs` (1296)
8. `rust/crates/pumas-core/src/model_library/hf/download.rs` (1203)
9. `rust/crates/pumas-core/src/model_library/dependencies.rs` (1109)
10. `rust/crates/pumas-core/src/model_library/hf_cache.rs` (983)

Plan:
- [ ] Prioritize decomposition of `pumas-rpc/src/handlers/models.rs` and high-churn `pumas-core` files.
- [ ] Create per-file extraction plans before refactors to avoid behavior regressions.

### WS-04: TODO Hygiene (Priority P3)
Goal: Ensure TODO comments include ticket/owner/date context per documentation standard.

Open TODOs without context:
1. None.

Plan:
- [x] Replace remaining TODOs with `TODO(#id)` or `TODO(@owner)` format.

## Changelog
### 2026-02-27
1. Created tracker and baseline audit.
2. Implemented core-backed RPC bridge for remaining mapping/cross-filesystem stubs.
3. Corrected response wrapping behavior for `refresh_model_mappings`.
4. Added `README.md` coverage for all currently missing Rust crate `src` directories.
5. Added `README.md` coverage for all currently missing frontend/electron `src` directories (WS-02 complete).
6. Resolved TODO formatting non-compliance by adding owner context to remaining TODO.
