# v0.8.0 consumer API

**Plan status:** Active. **Acceptance status:** not yet satisfied.
**Current phase:** M3. **Next slice:** M3 — image-text-to-text and EmbeddingGemma 2.

## Objective and scope

Make the public HTTP inference surface usable by independent consumers
(phased-memory, Tuldok, Lanternwake, Pumas-Playground; Pantograph and Eidetic
natively) before the v0.8.0 freeze, and add the model support the owner
requires: image-text-to-text, EmbeddingGemma 2 multimodal embeddings, and the
llama.cpp decision models (Kev, OpenJev).

Out of scope: release packaging gates, briefs not listed here, production
speech, Torch text generation, crate restructuring.

## Owner decisions (2026-11-02)

- Additive changes never break consumers. Responses are extensible; consumers
  ignore fields, list entries and enum values they do not use. Changing the
  meaning or shape of an existing operation bumps `contract_version`.
- Every rejection carries a human-readable reason; the caller's `request_id` is
  echoed whenever it can be read.
- Flat, model-agnostic limits are removed. Limits derive from the serving
  configuration and runtime; only transport and decode-safety bounds stay fixed.
- Consumers discover served models, profiles and capabilities from Pumas rather
  than hardcoding them. Served IDs are listed once per model/profile.
- Pumas provides text, image, audio and embedding primitives.

## Acceptance claims

| ID | Observable result | Evidence | State |
| --- | --- | --- | --- |
| C1 | Every typed rejection has `error.message`; parseable `request_id` is echoed; provider error text reaches the caller bounded | RPC unit/integration tests | satisfied: model-operation and gateway tests |
| C2 | One request grammar: `capability` and `options` optional; all requests accepted today remain accepted with identical results | existing + new model-operation tests | satisfied: existing suite unchanged in behavior; one-grammar test |
| C3 | A served-model listing route returns each (model, profile) once with capabilities, availability and derived limits | RPC tests | satisfied: served-models and /v1/models tests |
| C4 | No flat per-model limits remain on the typed path; advertised limits come from profile/runtime | tests + code inspection | satisfied for the typed path; the Torch image adapter keeps its 4,000-character prompt limit (adapter-specific) |
| C5 | `image-text-to-text` models advertise chat and image-to-text | descriptor tests | pending |
| C6 | EmbeddingGemma 2 (GGUF + mmproj) serves text and image embeddings via llama.cpp with MRL truncation | tests + live run on owner's library | pending |
| C7 | Kev/OpenJev serve a typed decision operation via llama.cpp `/v1/systemone` | tests + live run | pending |
| C8 | Contract docs state the extensibility rule; consumers tolerate additions and show Pumas reasons | consumer test suites | pending |

## Milestones

| M | Goal | Write set | State |
| --- | --- | --- | --- |
| M1 | Rejection reasons, request-ID echo, single grammar (C1, C2) | `pumas-rpc/src/handlers/model_operations*`, `openai_gateway.rs` error projection, `docs/contracts/model-operations-v1.md` | done |
| M2 | Served-model listing, derived limits, no duplicate IDs (C3, C4) | `model_operations*`, `openai_gateway.rs`, `server.rs`, `pumas-core` ONNX limits export | done |
| M3 | Image-text-to-text and EmbeddingGemma 2 (C5, C6) | `model_operations*`, llama.cpp launch specs/router presets, Ollama load path, embedding projection, `pumas-core` primary-artifact selection (projector files) | active |
| M4 | Decision capability (C7) | `model_operations*`, gateway route for `/v1/systemone`, contract doc | planned |
| M5 | Consumer updates and contract docs (C8) | phased-memory, Tuldok, Lanternwake, Pumas-Playground (separate repos) | planned |

Re-plan if a milestone needs a core (`pumas-core`) contract change not listed
above, or if any consumer cannot keep working across M1–M4 without its own update.

## Composed design (applicable: new public route and new adapter)

1. Concerns: request grammar (what the caller wants), model selection (which
   served model/profile), capability resolution (what that model can do now),
   provider projection (how a provider is called), outcome projection (how
   results and rejections are returned).
2. Required interleaving: capability availability depends on runtime readiness;
   kept in descriptor computation, not in the grammar.
3. Callers know: one request shape, discovery via the listing route, error
   `code`/`outcome`/`message`.
4. Adding a capability touches: `Capability`/`SemanticTask`, descriptor
   computation, provider projection, result projection. Consumers are untouched.
5. Stable interfaces: the typed DTOs and descriptors; provider envelopes stay
   behind projection.
6. The decision adapter and listing route can be replaced independently of the
   existing text/embedding/image adapters.
7. Deletion: removing the second parser path deletes code; the listing route and
   decision adapter are the only new permanent mechanisms.
8. Inherent complexity stays in descriptor computation (readiness and semantics).

## Links

Review that produced this plan: owner conversation 2026-11-02. No ledger is kept
for this plan; commit messages and test results carry history.
