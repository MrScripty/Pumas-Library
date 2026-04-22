# Model Import Helpers

## Purpose
This directory holds the workflow and extracted support components behind the model import dialog. It keeps import-review state, metadata lookup state, bundle-component disclosure UI, and single-entry lookup rendering out of the main dialog so the UI can render backend-owned classification results without reimplementing import rules in one monolithic JSX file.

## Contents
| File/Folder | Description |
|-------------|-------------|
| `modelImportWorkflowHelpers.ts` | Pure helpers that normalize backend import classifications into review entries, directory findings, shard state, embedded metadata matches, and final import specs. |
| `modelImportWorkflowHelpers.test.ts` | Unit coverage for the extracted import workflow helper transformations and repo-id inference rules. |
| `useModelImportWorkflow.ts` | Owns the import dialog workflow state machine from path classification through metadata lookup and import execution. |
| `useModelImportWorkflow.test.ts` | Hook coverage for classification orchestration, embedded-metadata lookup flow, shard grouping, and import completion state. |
| `useEmbeddedMetadataToggles.ts` | Owns embedded-metadata visibility and on-demand embedded metadata loading for import review entries. |
| `useEmbeddedMetadataToggles.test.ts` | Hook coverage for embedded metadata loading, cached states, unsupported files, and all-field toggles. |
| `metadataUtils.ts` | Pure helpers for security badges, GGUF metadata display priority, and import-review formatting. |
| `metadataUtils.test.ts` | Regression coverage for the pure metadata helper functions used by the dialog. |
| `ImportBundleComponents.tsx` | Presentational disclosure for external diffusers bundle component manifests. |
| `ImportLookupCard.tsx` | Presentational metadata-lookup card for file and directory import entries. |

## Problem
The import dialog has to review mixed inputs such as single files, single-model directories, and external diffusers bundles. That review flow needs transient UI state, formatting helpers, and a few extracted presentational components, but it must still preserve the backend as the source of truth for path classification and import behavior.

## Constraints
- Directory/file classification that affects behavior is backend-owned and must not be guessed in the frontend.
- Security warnings for pickle-based formats must stay visible before import confirmation.
- The workflow has to support mixed import batches without splitting into separate dialogs for files, directories, and bundles.
- Metadata lookup failures must remain reviewable without blocking the entire batch UI from rendering.

## Decision
- Keep the workflow orchestration in `useModelImportWorkflow.ts` so one hook owns import lifecycle state, lookup retries, and completion callbacks.
- Keep embedded-metadata disclosure state in `useEmbeddedMetadataToggles.ts` so optional metadata loading does not grow the import lifecycle state machine.
- Keep classification-to-entry shaping and import spec assembly in `modelImportWorkflowHelpers.ts` so the transformation rules stay pure, testable, and separate from React state mutation.
- Keep display-only logic in `metadataUtils.ts` as pure functions so metadata formatting and badge decisions stay easy to test.
- Extract bundle-manifest and lookup-card rendering into small components so the dialog can focus on step orchestration instead of carrying every import-entry detail inline.
- Preserve backend terminology such as classification kind, bundle format, and HF metadata result fields instead of inventing frontend-only aliases.

## Alternatives Rejected
- Put all import-review state and per-entry rendering inside `ModelImportDialog.tsx`: rejected because dialog rendering and workflow mutation would become harder to test and reason about.
- Recompute directory/file classification in the frontend from dropped paths: rejected because it would create a second import classifier that could drift from the Rust model library.

## Invariants
- Import entries are derived from backend classification results and remain keyed by backend-reported paths.
- Security acknowledgement is required for pickle-risk single-file imports before execution can proceed.
- HF lookup state decorates backend import candidates; it does not replace backend-owned model type or routing decisions.
- Review state in this directory is transient UI state only and must not become a persisted source of truth.

## Revisit Triggers
- The import workflow gains a second entry surface with different lifecycle rules.
- Bundle/classification outcomes expand enough that one hook can no longer own the workflow safely.
- Metadata formatting rules start depending on backend-only state that should instead be normalized before crossing the RPC boundary.

## Dependencies
**Internal:** `frontend/src/api/import.ts`, `frontend/src/types/api.ts`, `frontend/src/utils/logger.ts`, and the import dialog/view components.
**External:** React hooks and `lucide-react` icons for badge presentation.

## Related ADRs
- None identified as of 2026-04-11.
- Reason: import workflow boundaries are documented in implementation plans and module READMEs, but not yet in a standalone ADR.
- Revisit trigger: a second frontend or external host starts depending on this workflow contract directly.

## Usage Examples
```tsx
const workflow = useModelImportWorkflow({
  importPaths,
  onImportComplete,
});

const reviewEntries = workflow.entries;
```

## API Consumer Contract
- None identified as of 2026-04-11.
- Reason: this directory is an internal frontend helper module rather than a host-facing API surface.
- Revisit trigger: the workflow is extracted into a shared package or consumed outside the import dialog tree.

## Structured Producer Contract
- None identified as of 2026-04-11.
- Reason: this directory formats and consumes structured data from the backend, but it does not publish persisted artifacts or schemas of its own.
- Revisit trigger: import review state or generated metadata summaries become persisted outputs consumed elsewhere.
