# Model Library Integration Plan

## Scope
Plan only. No code changes in this phase.

## Decisions Locked
- Canonical model library root is `shared-resources/models/`.
- No separate "model views" directory. Models are linked directly into each app's expected models path.
- App translation configs live in `launcher-data/config/model-library-translation/<app>_<appversion>_<modelconfig>.json`.
- Canonical metadata lives with each model directory as `metadata.json`.
- User overrides live alongside each model as a separate JSON file (e.g., `overrides.json`).
- SQLite index (`models.db`) lives at `shared-resources/models/models.db` and is derived from canonical metadata.
- Version ranges in overrides are semver-aware.
- Naming rules: max length 128; strip invalid chars; no spaces or symbols. Preserve official name in metadata alongside cleaned name. Enforce at import and mapping time; downloads are cleaned after completion.
- ComfyUI uses symlinks; config-based mapping is future work for apps that do not support symlinks.
- Use existing logging + typing systems; no duplicate code paths.
- Dependency checker must verify and install all required Python deps (beyond GUI baseline).

## Proposed Library Layout
Example (exact taxonomy is configurable):

shared-resources/models/
  models.db
  llm/
    <family>/
      <model_id>/
        metadata.json
        overrides.json
        <files...>
  diffusion/
    <family>/
      <model_id>/
        metadata.json
        overrides.json
        <files...>

Metadata must include both official and cleaned names.
Overrides should include app version ranges and any link overrides.

## Phase 1: Consolidate ResourceManager
Goal: use a single ResourceManager implementation.
- Switch all imports/usages to `backend/resources/resource_manager.py`.
- Remove or stub legacy `backend/resource_manager.py`.
- Update tests and any factory wiring in `backend/api/core.py`.
- Document the single entry point.

## Phase 2: Metadata + SQLite Index
Goal: adopt proposal metadata as canonical and derive fast queries via SQLite.
- Define TypedDicts for proposal metadata and overrides.
- Create a metadata loader/writer that writes JSON + updates SQLite.
- Add an indexer that can re-scan the library root and rebuild the DB.

SQLite design (chosen):
Option A: one table with JSON column
- Table: models(id, path, cleaned_name, official_name, model_type, tags_json, hashes_json, metadata_json, updated_at)
- Use JSON1 for filtered queries (tags, compat, etc.)
- Pros: simple, schema flexible; Cons: heavier JSON queries

## Phase 3: Mapping Pipeline (App Translation)
Goal: map library models into app-specific structures via config.
- Define mapping config format (per app version and model config).
- Implement mapping engine that:
  - Loads metadata + overrides (semver ranges)
  - Applies naming normalization and collision handling
  - Resolves target paths based on mapping config
  - Creates symlinks into the app's models directory
- Auto-mapping is default; user overrides restrict via version ranges.

## Phase 4: Import + Download Services
Goal: integrate mockup features with existing logging/progress systems.
- Importer: move/copy local files into library with hash computation and naming rules.
- Downloader: download into a temp location, then clean/rename; update metadata + hashes.
- Reuse existing retry/progress helpers where possible.
- Consistent logging via `backend/logging_config.py`.

## Phase 5: API + GUI Contracts
Goal: align backend endpoints with UI needs.
- Query library via SQLite (filter by app, type, tag, etc.).
- Refresh index and mapping.
- Update overrides (link/unlink or adjust version ranges).
- Expose dependency check + install for added Python deps.

## Dependency Checker Alignment
- Extend `backend/api/dependency_manager.py` to check all new Python deps
  (e.g., huggingface_hub, pydantic, tenacity, blake3).
- Ensure GUI startup path calls install for missing deps.
- Keep this aligned with `requirements.txt` and `requirements-lock.txt`.

## Edge Cases and Policies
- Naming collisions: decide strategy (append hash suffix or incremented suffix).
- Models aging out: semver range overrides block linking to newer app versions.
- File integrity: store hashes in metadata; verify on import/download.
- Symlink failures: log and report; do not delete original model files.

## Testing/Validation
- Unit tests for naming normalization, semver range filtering, mapping outputs.
- SQLite index rebuild test for large libraries.
- Import/download behavior for large files (hash verification, rename).
- End-to-end: add model -> metadata + overrides -> SQLite -> mapping -> ComfyUI sees model.
