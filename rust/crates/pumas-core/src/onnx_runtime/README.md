# pumas-core ONNX Runtime

## Purpose

Own the Rust ONNX Runtime provider/session boundary for embedding serving.
This module validates ONNX model paths, model ids, embedding request shape,
execution-provider options, and session lifecycle requests before real ONNX
Runtime execution is wired into serving.

## Contents

| File | Description |
| ---- | ----------- |
| `mod.rs` | ONNX provider/session contracts, session manager, and shared validation. |
| `fake.rs` | Deterministic fake embedding backend used by serving/gateway slices until real ONNX execution is wired. |
| `postprocess.rs` | Pure embedding post-processing for pooling, optional layer norm, truncation, and L2 normalization. |
| `real.rs` | Real ONNX Runtime session loader boundary backed by the Rust `ort` crate. |
| `tokenizer.rs` | Rust tokenizer loader/tokenization contract for sibling `tokenizer.json` files. |
| `tests.rs` | Focused ONNX contract, fake backend, and session-manager tests. |

## Problem

ONNX embedding serving needs a Rust-owned execution boundary that can be tested
without Python sidecars or ad hoc session construction. Serving and gateway code
should depend on validated contracts and a session handle instead of
constructing ONNX sessions or tokenizer state ad hoc.

## Constraints

- Pumas `/v1` remains the external facade.
- ONNX model paths are resolved under a caller-provided root before load.
- Real ONNX Runtime dependencies are added only after dependency review and stay
  owned by the Rust crate/module that performs execution.
- Session manager construction belongs at a Rust composition root.
- Backend implementations must honor bounded inference/lifecycle concurrency.
- Shutdown must stop new load/inference/list/unload work, wait only for a
  bounded drain window, unload known sessions, and report cleanup failures.

## Decision

Start with a fake backend and validated Rust contracts. The fake backend
implements the same load, unload, list/status, and embedding API expected from
the real ONNX backend. A small session manager wraps the backend with a
semaphore so later real execution cannot accidentally bypass concurrency
limits. Shutdown closes the manager before draining all operation permits, then
unloads sessions through the backend while holding those permits so new work
cannot interleave with cleanup.

## Invariants

- Model ids are validated separately from filesystem paths.
- Load requests carry a validated `.onnx` file under an allowed root.
- Tokenizer loading resolves a sibling `tokenizer.json` under the same allowed
  root and rejects root escapes before parsing.
- Embedding input must be non-empty and bounded before backend execution.
- Tokenized input must be non-empty and bounded before tensor construction.
- Dimensions are positive and capped before backend execution.
- Real session loading uses the validated ONNX model path and explicit CPU
  execution-provider session options.
- Post-processing returns one embedding row per input row and rejects shape or
  dimension mismatches before truncation/normalization.
- Unload removes backend-owned session state.
- After shutdown begins, new session-manager operations fail with a typed
  backend error instead of entering the provider backend.

## Revisit Triggers

- Real ONNX Runtime integration needs tokenizer files, provider options, or
  output post-processing not represented by the current contracts.
- GPU execution-provider selection requires platform-specific package or
  runtime-profile contracts.
- Gateway error bodies need richer provider-specific error metadata.

## Dependencies

**Internal:** Rust standard library, `tokio`, `async-trait`.

**External:** Real execution dependencies are declared in the owning Rust
manifest after dependency review. The fake backend does not load native ONNX
Runtime libraries.

## API Consumer Contract

- Consumers construct validated `OnnxLoadRequest` and `OnnxEmbeddingRequest`
  before calling the session manager.
- Consumers receive stable session status, embedding response, and typed error
  values.
- Consumers call `shutdown` from the lifecycle owner with a bounded timeout
  before dropping the manager when they need deterministic cleanup.
- Fake backend behavior is deterministic and exists only to validate the public
  ONNX provider/session contract before real ONNX Runtime execution lands.
