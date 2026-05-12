# pumas-core ONNX Runtime

## Purpose

Own the Rust ONNX Runtime provider/session boundary for embedding serving.
This module validates ONNX model paths, model ids, embedding request shape,
execution-provider options, and session lifecycle requests before the serving
adapter or gateway can load or query an ONNX model.

## Contents

| File | Description |
| ---- | ----------- |
| `config.rs` | Model-package `config.json` reader for default embedding dimensions and metadata checks. |
| `mod.rs` | ONNX provider/session contract types and shared validation. |
| `fake.rs` | Deterministic fake embedding backend used by serving/gateway slices until real ONNX execution is wired. |
| `manager.rs` | Bounded session-manager wrapper and backend trait used by fake and real execution backends. |
| `output.rs` | Real ONNX output tensor selection, dtype extraction, and hidden-state shape validation. |
| `package.rs` | Shared model-package file discovery under the validated ONNX model root. |
| `postprocess.rs` | Pure embedding post-processing for pooling, optional layer norm, truncation, and L2 normalization. |
| `real.rs` | Real ONNX Runtime session loader boundary backed by the Rust `ort` crate. |
| `real_backend.rs` | Real ONNX backend that stores loaded sessions and runs tokenization, ONNX inference, and post-processing. |
| `tensors.rs` | Tokenized-input padding into ONNX Runtime input tensor buffers. |
| `tokenizer.rs` | Rust tokenizer loader/tokenization contract for model-package `tokenizer.json` files. |
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

The module keeps the fake and real backends behind the same
`OnnxEmbeddingBackend` contract. The fake backend stays as deterministic test
infrastructure. The real backend owns tokenizer/config discovery, ONNX Runtime
session loading, input tensor construction, output extraction, and embedding
post-processing. A small session manager wraps both backends with a semaphore
so execution cannot bypass concurrency limits. Shutdown closes the manager
before draining operation permits, then unloads sessions through the backend
while holding those permits so new work cannot interleave with cleanup.

## Invariants

- Model ids are validated separately from filesystem paths.
- Load requests carry a validated `.onnx` file under an allowed root.
- Tokenizer loading searches from the `.onnx` file directory up to the allowed
  model root for `tokenizer.json`, then rejects root escapes before parsing.
- Real session loading searches the same package scope for `config.json` and
  uses `hidden_size`/`n_embd` as the source embedding dimensions.
- Embedding input must be non-empty and bounded before backend execution.
- Tokenized input must be non-empty and bounded before tensor construction.
- Dimensions are positive and capped before backend execution.
- Real session loading uses the validated ONNX model path and explicit CPU
  execution-provider session options.
- Real inference pads tokenized inputs into bounded `input_ids`,
  `attention_mask`, and optional `token_type_ids` tensors before executing ONNX
  Runtime.
- Real output extraction accepts `f32`, `f16`, or `bf16` hidden-state tensors
  with shape `[batch, tokens, dimensions]`.
- Post-processing returns one embedding row per input row and rejects shape or
  dimension mismatches before truncation/normalization.
- Unload removes backend-owned session state.
- After shutdown begins, new session-manager operations fail with a typed
  backend error instead of entering the provider backend.

## Lifecycle

- The RPC server composition root owns the ONNX session manager used by serving
  and gateway adapters.
- `serve_model` loads a validated `.onnx` model into the manager and records
  backend served status only after the manager lists the loaded session.
- Duplicate load requests return the existing loaded state rather than creating
  another session.
- `unserve_model` removes the backend served status and unloads the matching
  ONNX session through the manager.
- Gateway `/v1/models` is backed by backend served status; `/v1/embeddings`
  invokes the loaded ONNX session through the Rust gateway adapter.
- Shutdown rejects new manager work, waits for bounded in-flight work, and
  unloads known sessions.

## Gateway Behavior

- ONNX Runtime declares OpenAI gateway support for `/v1/models` and
  `/v1/embeddings` only.
- Chat and completion endpoints are rejected by provider capability checks
  before proxying.
- External clients address ONNX models through the Pumas `/v1` gateway alias
  returned by `/v1/models`; provider-facing session ids are backend-owned.
- Embedding inputs are tokenized by the Rust tokenizer path, padded into ONNX
  input tensors, and post-processed into one embedding vector per input item.

## Limits

- Load requests must resolve to a validated `.onnx` file under the allowed model
  root.
- Tokenizer and config discovery may walk from the `.onnx` directory up to the
  validated model root but cannot escape that root.
- Embedding request item counts, tokenized inputs, dimensions, and tensor
  shapes are validated before backend execution.
- The session manager enforces bounded concurrency for load, list, unload, and
  embedding operations.
- Only CPU execution-provider options are currently represented in the managed
  ONNX profile contract.

## Troubleshooting

| Symptom | Likely Cause | Resolution |
| ------- | ------------ | ---------- |
| `model_not_executable` or `invalid_format` during serve validation | The selected artifact is not a `.onnx` file or is outside the validated model root. | Select the `.onnx` artifact from the imported model package and retry route/serve validation. |
| Tokenizer load failure | `tokenizer.json` is missing from the `.onnx` directory or parent package root. | Re-import or repair the model package so tokenizer files live under the validated model root. |
| Config or dimension failure | `config.json` is missing usable `hidden_size`/`n_embd` metadata, or output dimensions disagree with the loaded model config. | Check the model package config and prefer known embedding exports with standard hidden-state outputs. |
| Embeddings endpoint rejects chat/completion requests | ONNX Runtime is registered as embeddings-only. | Send embedding requests to `/v1/embeddings`; use another provider for chat/completions. |
| Request fails after app shutdown begins | The session manager is closed and rejects new work. | Restart the app or runtime owner before serving again. |

## Revisit Triggers

- ONNX Runtime integration needs tokenizer files, provider options, or output
  post-processing not represented by the current contracts.
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
