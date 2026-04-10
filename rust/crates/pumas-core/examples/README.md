# Examples

## Purpose
This directory contains runnable entrypoints for exercising `pumas-library` behavior against realistic workflows without having to wire a separate host application. The examples are used for targeted debugging, migration dry runs, and operational audits where a short, focused binary is more useful than a permanent API surface.

## Contents
| File/Folder | Description |
|-------------|-------------|
| `basic_usage.rs` | Minimal end-to-end example for initializing `PumasApi` and interacting with the library. |
| `search_models.rs` | Demonstrates local library search behavior and projected record output. |
| `reclassify_model_types.rs` | Re-runs model-type classification across a library after resolver improvements. |
| `repair_library_integrity.rs` | Repairs or audits library state when metadata/index drift is suspected. |
| `reconcile_library_state.rs` | Forces reconciliation workflows that rebuild SQLite-backed state from on-disk metadata. |
| `hf_metadata_audit.rs` | Samples live Hugging Face metadata without downloading weights and records how Pumas classifies and stores the results. |

## Problem
Core library behavior often spans multiple layers: remote metadata ingestion, normalization, model classification, metadata projection, and SQLite indexing. Unit tests are necessary but not sufficient when the failure only appears after several of those layers are exercised in sequence.

## Constraints
- Examples must stay lightweight enough to run locally during investigation.
- They should reuse public or near-public crate entrypoints instead of inventing a parallel debug stack.
- Some examples intentionally touch live external systems, so they must make their side effects and boundaries obvious.
- These binaries are diagnostic tools, not long-term product entrypoints, so they should favor clarity and repeatability over polished CLI ergonomics.

## Decision
Keep focused, single-purpose binaries in `examples/` so maintainers can exercise real workflows quickly while preserving the main library API surface. Each example should document a concrete workflow boundary instead of becoming a grab bag of unrelated flags.

## Alternatives Rejected
- Adding every diagnostic path to the main library API: rejected because it would turn internal investigation workflows into supported surface area.
- Keeping one giant debug binary with subcommands: rejected because the examples are easier to review, reason about, and delete independently when a workflow no longer matters.

## Invariants
- Examples must not redefine core business rules that already exist in the library.
- Live-network examples must make it clear when they inspect remote metadata versus when they would download model assets.
- Output produced by an example should be traceable back to real library projections or real upstream metadata, not hand-built summaries detached from the code path under investigation.

## Revisit Triggers
- A second package needs the same diagnostic entrypoints and shared CLI infrastructure becomes justified.
- Example binaries start sharing substantial parsing or reporting code that should become a reusable internal utility.
- The repository adopts a different convention for operational tooling outside `examples/`.

## Dependencies
**Internal:** `pumas-library` public modules such as `PumasApi`, `ModelLibrary`, classification helpers, and index-backed search routines.
**External:** Standard crate dependencies already used by the package, such as `tokio`, `serde`, `reqwest`, and `tempfile`, when an example needs async execution, JSON handling, HTTP access, or temporary workspaces.

## Related ADRs
- None identified as of 2026-04-10.
- Reason: this directory hosts diagnostic entrypoints rather than a long-lived architectural boundary with its own ADR history.
- Revisit trigger: examples become the primary operational interface for a subsystem or start carrying compatibility obligations across releases.

## Usage Examples
```bash
cargo run -p pumas-library --example search_models -- ./example-models llama

cargo run -p pumas-library --example hf_metadata_audit -- \
  --sample-size 30 \
  --markdown /tmp/pumas-hf-metadata-audit.md \
  --json /tmp/pumas-hf-metadata-audit.json
```

## API Consumer Contract
- Consumers are developers running the examples manually from the package root.
- Inputs are CLI arguments documented inline by each example; unsupported flags should fail fast with a clear error.
- Examples may create temporary workspaces or read local library paths, but they should avoid hidden destructive behavior.
- Output is diagnostic and human-oriented unless an example explicitly documents a machine-readable artifact such as JSON.
- Compatibility is best-effort for repository maintainers; examples may evolve with the codebase as long as they continue to reflect real library workflows.

## Structured Producer Contract
- `hf_metadata_audit.rs` can emit Markdown and JSON artifacts summarizing sampled Hugging Face metadata and resulting Pumas projections.
- JSON output is intended for ad hoc analysis, not a stable external schema; fields may evolve when the audit captures new dimensions.
- Markdown output is a generated report for maintainers and should stay traceable to the sampled JSON data.
- Revisit trigger: generated audit artifacts become inputs to CI or another persistent consumer, at which point a stable schema and versioning policy would be required.
