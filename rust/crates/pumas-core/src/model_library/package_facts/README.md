# Package Facts

## Purpose

This directory owns bounded package inspection for `ModelLibrary` package facts.
`ModelLibrary` remains the public facade for resolving detail facts, summary
facts, selector snapshots, cache writes, and update-feed publication.

## API Consumer Contract

Consumers receive only versioned DTOs from `crate::models::package_facts`.
Package facts describe evidence found in model package files. They do not select
a backend, device, scheduler, queue policy, or runtime support verdict.

Selector and summary paths must stay SQLite-backed and non-hydrating. Full
package inspection is only for targeted detail resolution or bounded refresh and
migration work.

## Structured Producer Contract

Extraction is split by evidence owner:

- `manifest.rs` selects bounded package files and builds source fingerprints.
- `artifact.rs` projects artifact kind, components, weights, shards,
  quantization hints, companion files, and class references.
- `transformers.rs` extracts Transformers-compatible config evidence,
  custom-code sources, dependency manifests, and advisory backend hints.
- `generation.rs` extracts model-provided generation defaults.
- `summary.rs` projects compact selector summaries from full detail facts.

All filesystem reads must stay inside the validated package directory and must
use bounded standard file lists or selected artifact files. Human diagnostic
messages may aid display and debugging, but machine-readable semantics belong in
typed DTO fields.

## Unsupported Behavior

Package facts must not infer image-generation family, backend, task, or runtime
support from display names, workflow names, directory names, or repository
lookup tables. Ambiguous or missing evidence should remain explicit facts for
the consumer to handle.

## Revisit Triggers

Revisit this boundary when a new public package-facts DTO is added, selected
artifact cache keys become mandatory, a cache migration changes summary
freshness semantics, or a package standard requires a new bounded parser.
