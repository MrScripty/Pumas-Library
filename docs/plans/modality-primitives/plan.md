# Plan: Modality primitives

**Plan status:** `Active`

**Decision:** [ADR 0005](../../adr/0005-modality-primitives.md).

**Goal:** run Liquid AI's LFM3-VL-2B, LFM3-Audio-2B, LFM3-Omni-2B, d1-2B and
d1-Omni-2B (and later models with the same runtime support) through the typed
model operations, by composing modality primitives instead of adding one
capability path per model and modality.

**Baseline:** `origin/main` plus [#118](https://github.com/MrScripty/Pumas-Library/pull/118)
(rerank, Laya). Runtime: llama.cpp b12099. The LFM3 and d1 cards need b11960
or newer, and LFM3-VL needs b12088 or newer.

**Current phase:** S1 (2026-11-06).

## Acceptance

1. `GET /v1/served-models` and `/v1/capabilities` report each served model's
   input and output modalities from its runtime, and each capability's
   accepted input and produced output modalities.
2. One audio part shape is accepted in chat messages, embedding content items
   and decision input, wherever the served model declares `audio`. The same
   holds for images. A part in an undeclared modality is refused before
   admission, naming the model's declared set.
3. Existing v1 requests, capability names and results are unchanged. The
   current model-operations, vision and consumer tests pass unchanged except
   for additive descriptor fields.
4. Live on llama.cpp b12099:
   - d1-2B answers text and image decisions.
   - d1-Omni-2B answers text, image and audio decisions, and a spoken question.
   - LFM3-VL-2B describes an image.
   - LFM3-Audio-2B answers about a speech clip.
   - LFM3-Omni-2B answers about an image and a speech clip.
5. Speech output is reported unavailable with a reason.

## Slices

| Slice | Content | Status |
| --- | --- | --- |
| S1 | Runtime modality facts: one bounded llama.cpp probe (`/props` modalities and `/v1/models` architecture) that the vision and decision checks share; static declarations for other providers; additive discovery fields | Next |
| S2 | Primitive layer: shared text, image and audio validation and budget; chat, embeddings and systemone encoders. Image code moves unchanged; audio packs PCM as WAV | Planned |
| S3 | Grammar and resolution: audio parts in messages and content items, decision `audio`; acceptance gated by declared modalities; image-only messages still resolve to `image_to_text` | Planned |
| S4 | Live qualification of the five models, docs, release notes | Planned |
| S5 | `json` output primitive: chat structured output against a caller schema; decisions and rankings expressed as `json` results | Planned |

Later, outside 0.8.0: video input (llama.cpp needs `ffmpeg` for it), speech
output (realtime WebSocket transport), and `vector` input.

## Blockers and decisions

- **D1, schema validation for `json` (S5).** No JSON Schema validator is in
  the dependency tree. The options are adding the `jsonschema` crate (full
  draft support, a new audited dependency) or validating a documented subset
  (types, required properties, enums, arrays). This is needed before S5, not
  before S1–S4.
- **D2, router-mode multimodal.** Image input currently requires a dedicated
  llama.cpp profile, while the router preset already loads the projector. If
  the router's `/props?model=` reports the same modalities, the restriction
  can be lifted in S1. Otherwise serving LFM3-Omni keeps needing a dedicated
  profile.
