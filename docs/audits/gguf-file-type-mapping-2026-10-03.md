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

## Temporary qualification binary

This repair's PR25 headless job retained its already-tested Linux x86_64
no-inference RPC output for one day. The archive was explicitly unreleased and
includes exact source/checkout commits and trees, Rust/Cargo toolchain, build
command/features, runtime-library listing, transformed/original binary hashes,
per-file hashes, and unchanged license/notice inventory provenance. It contains
no model weights, credentials, Python runtime, or runtime userdata. It is not a
production release or a model-inference executable. Using it against an owned
library requires the prior process to be observed stopped and the supported
inspection API; no cache edits or duplicate library owner are authorized by the
archive. The temporary workflow gate has been removed after qualification;
the allowlisted packager and its tests remain as provenance tooling, without
any automatic artifact publication.

### Qualification receipt

- Final source head `6a3e6ce6998f326e9cf7d2e1bc0ea51e3a2d381c`, source and hosted
  merge tree `e025953b16178f5d953780171fc43fe953b22526`.
- [Build 37098100687](https://github.com/MrScripty/Pumas-Library/actions/runs/37098100687)
  passed all ordinary gates, including native Linux/macOS/Windows.
- Artifact ZIP SHA-256 `698a2cf1f54964ca9c421104127a0f7630aa6fff85367f3cf77d2ec62a212a0b`;
  packaged tar SHA-256 `c05201824d255bc4691f2b404ed813f8cf327ec4fc9930f6724a71e9485397a3`;
  stripped RPC SHA-256 `4495e24298c37c2f861683dfbb74ff55f5c01dd88caa370ba6243883d6dbf26b`.
- ZIP digest, archive sidecar, every per-file checksum, notice source bytes and
  source/checkout tree identity were independently verified. The stripped binary
  executed `--help` successfully; that is not full runtime startup evidence.
- Supported reinspection of the historical managed store remains unqualified:
  the prior process has no observed exit receipt across execution namespaces.
  The store was preserved; no stale-row deletion, duplicate owner, manual cache
  edit, model inference, installed-v0.7 repair or real-model readiness is claimed.
