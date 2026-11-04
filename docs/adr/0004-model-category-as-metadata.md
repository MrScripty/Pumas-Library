# ADR 0004: A model's category is metadata chosen by use

Status: Accepted and implemented (0.8.0).

Affected boundaries: `ModelType` in
`rust/crates/pumas-core/src/model_library/types.rs`, the shared resolver
(`model_type_resolver.rs`, `library.rs`), classification rule tables in the
model index (`index/model_index/governance.rs`), reclassification and
migration (`library/migration.rs`), the `set_model_category` RPC
(`rust/crates/pumas-rpc/src/contract.rs`) and the desktop library grouping
(`frontend/src/utils/libraryModels.ts`,
`frontend/src/components/ModelCategoryControl.tsx`).

## Context

Library models were grouped by a single detected type that was also the first
segment of their folder and ID. Several real models were grouped wrongly: Kev
and Laya (decision models) as LLMs because Hugging Face tags them
`text-classification`; LTX-2.3 (video) as audio or embedding; Granite Speech
(speech-to-text) as embedding because its repository tag said
`feature-extraction` while its config said `GraniteSpeechNarForASR`. Fixing a
category moved the model's folder and changed its ID, which broke served-model
IDs and runtime profiles. Models also have arbitrary inputs and outputs, which
one type cannot describe.

## Decision

- The category is the one use a person would choose the model for. Other
  inputs and outputs are shown as modality badges, not extra categories.
  Categories: `llm`, `vlm`, `decision`, `embedding`, `reranker`, `diffusion`,
  `video`, `audio`, `vision`, `unknown` (`ModelType::ALL` is the one list).
- The category is metadata. Changing it never moves a folder or changes an ID;
  the first ID segment is a storage bucket frozen at import. Only a family
  change still moves a model. New imports file under their detected category.
- Evidence precedence: a person's choice (`model_type_resolution_source:
  "user"`) > file and architecture facts (GGUF `<arch>.decision.type`,
  `…ForASR`) > the Hugging Face task tag > name and file hints > the storage
  bucket. A lower source that disagrees with a strong one adds a
  `model-type-hint-conflict` review reason instead of overruling it, and the
  model's task and modalities follow the category.
- Output decides the use: video output → `video`; image → `diffusion`;
  decision → `decision`; vector → `embedding`; speech in or out → `audio`;
  text with image input → `vlm`; text → `llm`.
- LoRA/PEFT packages are marked as adapters and take the category of the task
  they adapt.
- `set_model_category` records a person's choice, which re-scans keep. Clearing
  it re-detects the category in place. `unknown` cannot be chosen.

## Alternatives considered

- **Keep the category in the path and move models on a fix.** Rejected: it
  breaks IDs that consumers and runtime profiles hold.
- **Several categories per model.** Rejected: grouping needs one answer; the
  modality badges carry the rest.
- **Trust the task tag first.** Rejected: tags are set per repository and were
  wrong for each misgrouped model above, while file facts were right.

## Consequences

- Opening a library updates its system rule tables in place; the next refresh
  re-detects categories without moving anything.
- A model reports only the modalities its metadata records. An adapter with no
  task or base-model evidence (for example the LTX-2.3 IC-LoRA) stays under
  image generation until a person chooses its category.
- New categories are additive on the wire: the desktop contract is regenerated,
  and HTTP consumers must ignore enum values they do not use.
