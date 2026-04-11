# Importer Recovery Helpers

## Purpose
This directory holds recovery-oriented `ModelImporter` logic that scans the canonical library tree for orphaned model directories, interrupted downloads, and incomplete sharded downloads. It keeps the operational repair flows close to `ModelImporter` while separating them from the main import pipeline.

## Contents
| File | Description |
|------|-------------|
| `recovery.rs` | `ModelImporter` methods for orphan adoption, interrupted-download discovery, shard-recovery detection, and path-based inference helpers. |

## Problem
`ModelImporter` owns both ordinary import execution and filesystem recovery workflows. Leaving orphan adoption, interrupted-download discovery, and incomplete-shard detection inside the primary `importer.rs` file makes the importer harder to review and grows an already large module with code that is operationally distinct from the copy/hash/metadata pipeline.

## Constraints
- Recovery scans must keep using the same `ModelImporter` and `ModelLibrary` state as the main import path.
- Orphan adoption must remain idempotent and safe to run during startup or reconciliation.
- Recovery heuristics must ignore hidden/temp import directories and incomplete `.part` downloads.
- Path inference must continue to support partial library layouts when some path segments are missing.

## Decision
- Keep recovery logic as child-module `impl ModelImporter` blocks so it can still access importer-private state without widening visibility.
- Group orphan discovery, shard recovery, interrupted-download discovery, and path inference together because they all traverse the library tree and infer metadata from existing filesystem state.
- Keep the public recovery DTOs in `importer.rs` so callers do not need to chase submodule-specific type paths.

## Alternatives Rejected
- Keep recovery logic in `importer.rs`: rejected because the primary importer file is already too large and mixes normal import flow with repair operations.
- Move recovery helpers to a sibling module outside `importer/`: rejected because that would require widening private importer state or introducing extra plumbing just to reach `ModelLibrary`.

## Invariants
- Recovery scans never mutate directories that already have `metadata.json`.
- Orphan detection ignores directories containing `.part` files.
- Incomplete shard recovery reports at most one recovery candidate per directory.
- Interrupted-download discovery only reports directories not already tracked by download persistence.

## Revisit Triggers
- Recovery flows start owning enough state or orchestration to justify a dedicated service object.
- Additional repair workflows introduce a second distinct recovery concern that no longer fits this small submodule.

## Dependencies
**Internal:** `ModelImporter`, recovery DTOs in `importer.rs`, `crate::model_library::sharding`.
**External:** `walkdir` and standard-library filesystem traversal.
