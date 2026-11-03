# v0.8.0 model categories and modalities

**Plan status:** Active. **Acceptance status:** not yet satisfied.
**Current phase:** implementation. **Next slice:** M1 taxonomy and evidence rules.

## Objective and scope

Group library models by what a person uses them for, show the inputs and
outputs each model accepts, and stop a category change from moving or
renaming a model. Observed on the owner's library (2026-11-03):

| Model | Actual use | Shown as | Cause |
| --- | --- | --- | --- |
| Kev, Laya | decision/evaluation | LLM | HF tag `text-classification`; no decision category |
| LTX-2.3 (fp8, nvfp4, base) | video generation | audio, embedding | HF tag `image-to-video` unmapped; no video category |
| LTX-2.3 IC-LoRA-HDR | video LoRA adapter | diffusion | adapters are not recognized |
| Granite Speech 4.1 2b-nar | speech-to-text | embedding | HF tag `feature-extraction` outvoted config `GraniteSpeechNarForASR` |

Out of scope: changing the HTTP model-operations contract (it already routes by
capability input/output kinds), physical library re-layout, adapter
composition at serve time.

## Owner decisions (2026-11-03)

- A model has arbitrary declared inputs and outputs. Its category is the one
  use a person would choose it for, derived from those declarations; other
  modalities are shown as badges, not as extra categories.
- Add `video` and `decision` categories.
- The category is metadata. Changing it never moves a folder or changes a
  model ID.
- An explicit user choice of category wins over detection and survives re-scans.

## Design

- **Categories** (`ModelType`, wire string unchanged): `llm`, `vlm`,
  `decision`, `embedding`, `reranker`, `diffusion` (image generation),
  `video`, `audio`, `vision`, `unknown`. New values are additive: the desktop
  contract is regenerated and the HTTP contract already allows new enum values.
- **Evidence precedence:** user override > hard runtime/file signals (GGUF
  `<arch>.decision.type`, config architecture such as `…ForASR`) > HF task tag
  > name and file hints. A hard signal is never outvoted by a task tag.
- **Output-first use:** video output → `video`; image output → `diffusion`;
  decision output → `decision`; vector output → `embedding`; audio in or out
  (speech) → `audio`; text output with image input → `vlm`; text → `llm`.
- **Modalities:** `input_modalities`/`output_modalities` are always filled
  from the resolved task signature (adding `image->video`, `text+image->video`,
  `video->video`, `text->decision`) and shown as badges.
- **Adapters:** LoRA/adapter packages (`adapter_config.json`, `*lora*`
  weights, HF `lora` tag) are flagged `adapter` and take the category of the
  task they adapt; the UI shows an "Adapter" badge.
- **Category as metadata:** reclassification and migration keep a model's
  first path segment (its storage bucket) and ID; only `model_type` metadata
  and the index change. New imports still file under their detected category.
  Code that read the category from the ID prefix reads metadata instead.
- **Override:** `set_model_category` RPC records `model_type_resolution_source:
  "user"`; detection and reclassification skip such models.

## Milestones

| ID | Result | Evidence | State |
| --- | --- | --- | --- |
| M1 | `video` and `decision` categories; evidence precedence; task signatures; decision GGUF signal; `…ForASR`; adapter detection | resolver/identifier unit tests reproducing each observed case | pending |
| M2 | Category changes never move folders or change IDs; ID-prefix readers use metadata; user override honored by re-scan | library/reconcile/migration tests | pending |
| M3 | Desktop catalog carries category, input/output modalities and adapter role; `set_model_category` RPC | contract export, conformance, RPC tests | pending |
| M4 | UI groups by category (with Video and Decision), shows modality and adapter badges, lets the user set a category | frontend tests | pending |
| M5 | Owner library re-scanned: Kev/Laya → decision, LTX → video (LoRA as adapter), Granite nar → audio, without moving folders | live check on the owner library; CHANGELOG/release notes; candidate rehearsal | pending |
