# GGUF file-type inspection correction

## Demonstrated failure

The approved Qwen3-4B Q4_K_M artifact was acquired through Pumas's pinned intent
API. Its exact size and SHA-256 matched the approved upstream artifact, but
Pumas 0.7 reported a quantization mismatch. A read-only header/tensor-table probe
observed `general.file_type = 15`, with 216 Q4_K tensors, 37 Q6_K tensors and 145
F32 tensors. No model weights are included in this repository.

The [upstream llama_ftype enum at a pinned revision](https://github.com/ggml-org/llama.cpp/blob/99b95488cac0f00ce3f05af113a8c1e287753f87/include/llama.h#L106-L151)
assigns 15 to MOSTLY_Q4_K_M. Pumas's table incorrectly compressed retired numeric
slots 5 and 6, assigning 15 to MOSTLY_Q5_K_M and shifting every later label. The
[tensor-type enum](https://github.com/ggml-org/llama.cpp/blob/99b95488cac0f00ce3f05af113a8c1e287753f87/ggml/include/ggml.h)
is a different contract and must not be used to decode general.file_type.

## Repair and compatibility

The mapping preserves persisted enum numbers, including unsupported gaps, and
retains documented historical values without pretending removed quantizers are
available for inference. Unknown numbers remain UNKNOWN. A label is inspection
evidence, not a backend-support claim.

GGUF inspection now contributes a parser revision to package source fingerprints.
Existing GGUF detail/summary caches therefore become stale on targeted validation and must be recomputed
from the current package by the supported resolve_model_package_facts path. No
model bytes, metadata, user declarations, or SQLite rows are manually rewritten.
Fast cache snapshots remain explicitly unvalidated Cached projections until targeted
resolution; this is lazy invalidation, not a database-wide eager rewrite.
Non-GGUF fingerprints and the public package-facts DTO version remain unchanged.
The old installed 0.7 binary cannot correct its own enum by reinspection; the
fixed code must be deployed before a fresh supported inspection is authoritative.

## Evidence and gates

- A standalone build of the exact old table failed the type15→Q4_K_M assertion;
  the corrected table passes it, along with gap and Q5_K_M assertions.
- Repository tests cover raw-header extraction, stable upstream numeric values,
  unsupported gaps, deterministic GGUF-only legacy-fingerprint invalidation and
  unchanged source bytes.
- Native Linux/macOS/Windows workflow steps explicitly run the parser and cache
  regressions. Their exact-head hosted results are required before qualification.
- No large local source build or live-store migration is part of this repair.

Initial hosted qualification also exposed older intent/package-facts fixtures
encoding the same wrong enum numbers. Their Q4_K_M/Q5_K_M bytes are corrected to
15/17 while retaining their assertions. A new composed regression seeds stale
detail and summary rows, invokes the supported summary resolver, and verifies
regeneration from the raw header, both refreshed fingerprints, and unchanged
model/metadata bytes. No live store is edited by that synthetic test.
