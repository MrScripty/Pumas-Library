# Mypy Error Remediation Plan

Goal: Fix all mypy errors without silencing, and tighten typing gradually.

## Current Status

- Baseline `mypy.ini` added (lenient settings).
- Current run: 269 errors across 25 files.

## Approach

1) Fix type correctness in small, high-signal modules first.
2) Add precise types where a real contract exists.
3) For mixins, add a shared Protocol/base class that declares required attributes.
4) Remove any implicit Optional defaults by making types explicit.
5) Keep `ignore_missing_imports = True` only until we can add stubs or tighten.

## Error Buckets (from latest run)

### A) Implicit Optional defaults
Files:
- `backend/exceptions.py`
- `backend/api/core.py`

Fix:
- Change `param: str = None` to `param: Optional[str] = None` (or required).

### B) Missing third-party stubs
Files:
- `backend/launcher_updater.py` (`requests`)
- `backend/github_integration.py` (`cachetools`)

Fix:
- Add types packages (`types-requests`, `types-cachetools`) or vendor minimal protocols.

### C) Any-return and Any-typed JSON loads
Files:
- `backend/metadata_manager.py`
- `backend/release_data_fetcher.py`
- `backend/package_size_resolver.py`
- `backend/release_size_calculator.py`
- `backend/installation_progress_tracker.py`
- `backend/github_integration.py`

Fix:
- Add TypedDicts / typed models for JSON payloads and return types.
- Validate loaded JSON and return typed structures.

### D) Mixins missing attributes (attr-defined)
Files:
- `backend/version_manager_components/state.py`
- `backend/version_manager_components/launcher.py`
- `backend/version_manager_components/constraints.py`
- `backend/version_manager_components/dependencies.py`
- `backend/version_manager_components/installer.py`

Fix:
- Introduce a `Protocol` (e.g., `VersionManagerContext`) listing required attributes.
- Have mixins inherit from that Protocol to satisfy mypy.

### E) TypedDict key issues
Files:
- `backend/resource_manager.py`
- `backend/resources/model_manager.py`
- `backend/version_manager_components/dependencies.py`

Fix:
- Align TypedDict definitions with actual keys or update usage.

### F) Incorrect types / Optional handling
Files:
- `backend/api/core.py`
- `backend/version_manager.py`
- `backend/api/process_manager.py`
- `backend/api/shortcut_manager.py`
- `backend/api/size_calculator.py`
- `backend/api/system_utils.py`
- `backend/api/version_info.py`

Fix:
- Update field types and narrow `Optional` before use.

### G) Incorrect "any"/"callable" annotations
Files:
- `backend/release_data_fetcher.py`
- `backend/package_size_resolver.py`
- `backend/release_size_calculator.py`

Fix:
- Replace `any` with `Any`, `callable` with `Callable`, add proper signatures.

## Work Plan (Suggested Order)

1) **Implicit Optional defaults** (quick wins).
2) **Protocol for VersionManager mixins** (removes large attr-defined cluster).
3) **Fix `any`/`callable` annotations** (mechanical).
4) **TypedDict alignment + JSON parsing** (core correctness).
5) **Missing stubs** (install types packages or add local protocols).
6) **Finalize Optional handling and Any-return** (clean up).

## Progress Log

- [x] Step 1: Fix implicit Optional defaults.
- [x] Step 2: Add Protocol for mixins.
- [x] Step 3: Fix invalid `any`/`callable` types.
- [x] Step 4: Align TypedDict usage / JSON return types.
- [x] Step 5: Add missing stubs.
- [x] Step 6: Resolve remaining Optional/Any issues.

## Outcome

- `mypy backend/` passes with no errors.
