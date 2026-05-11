# pumas-core ONNX Runtime

## Purpose

Own the Rust ONNX Runtime provider/session boundary for embedding serving.
This module validates ONNX model paths, model ids, embedding request shape,
execution-provider options, and session lifecycle requests before real ONNX
Runtime dependencies are introduced.

## Contents

| File | Description |
| ---- | ----------- |
| `mod.rs` | ONNX provider/session contracts, fake embedding backend, session manager, validation, and tests. |

## Problem

ONNX embedding serving needs a Rust-owned execution boundary that can be tested
without adding native ONNX Runtime packages or Python sidecars. Serving and
gateway code should depend on validated contracts and a session handle instead
of constructing ONNX sessions or tokenizer state ad hoc.

## Constraints

- Pumas `/v1` remains the external facade.
- ONNX model paths are resolved under a caller-provided root before load.
- Real ONNX Runtime dependencies are added only after dependency review.
- Session manager construction belongs at a Rust composition root.
- Backend implementations must honor bounded inference/lifecycle concurrency.

## Decision

Start with a fake backend and validated Rust contracts. The fake backend
implements the same load, unload, list/status, and embedding API expected from
the real ONNX backend. A small session manager wraps the backend with a
semaphore so later real execution cannot accidentally bypass concurrency
limits.

## Invariants

- Model ids are validated separately from filesystem paths.
- Load requests carry a validated `.onnx` file under an allowed root.
- Embedding input must be non-empty and bounded before backend execution.
- Dimensions are positive and capped before backend execution.
- Unload removes backend-owned session state.

## Revisit Triggers

- Real ONNX Runtime integration needs tokenizer files, provider options, or
  output post-processing not represented by the current contracts.
- GPU execution-provider selection requires platform-specific package or
  runtime-profile contracts.
- Gateway error bodies need richer provider-specific error metadata.

## Dependencies

**Internal:** `crate::error`, Rust standard library, `tokio`, `async-trait`.

**External:** None in this slice. Candidate ONNX Runtime binding is reviewed in
the execution dependency slice.

## API Consumer Contract

- Consumers construct validated `OnnxLoadRequest` and `OnnxEmbeddingRequest`
  before calling the session manager.
- Consumers receive stable session status, embedding response, and typed error
  values.
- Fake backend behavior is deterministic and exists only to validate the public
  ONNX provider/session contract before real ONNX Runtime execution lands.
