# ADR 0006: Ranking heads are derived from the model's scoring contract

Status: Accepted (2026-11-09). Implementation is tracked in the
[rerank and embedding runtimes plan](../plans/rerank-embed-runtimes/plan.md).

Affected boundaries:

- the conversion pipeline (`rust/crates/pumas-core/src/conversion/`)
- the `start_model_conversion` RPC
- the [model operations contract](../contracts/model-operations-v1.md), which
  describes rerank requirements

## Context

Causal-LM rerankers score a query–document pair from the language-model
logits of one or two vocabulary tokens at the last position. They do this
under a fixed prompt. Examples:

- Qwen3-Reranker and Prism score `yes − no`.
- zerank-2 scores the raw `Yes` logit.

llama.cpp reranks only from a GGUF tensor, `cls.output.weight`, that holds
those rows. Its converter adds the tensor only for models it recognizes by
name as Qwen3 rerankers. Most quantized releases of zerank-2, and some of
Qwen3-Reranker, ship without the tensor. Pumas refuses such a GGUF with a
reason (since #118).

These models publish their scoring as a sentence-transformers
**`LogitScore` contract**. It is made of:

- `modules.json` locates a `LogitScore` module, whose `config.json` gives
  `true_token_id` and an optional `false_token_id`.
- `chat_template.jinja` renders `query` and `document` messages into the
  prompt.
- `sentence_bert_config.json` gives the template options, for example
  `add_generation_prompt`.

## Decision

- **A ranking head is derived by the conversion pipeline.** A new direction,
  `gguf_to_ranking_gguf`, reads a library GGUF and the source model's
  `LogitScore` contract, and writes a new GGUF. The new GGUF contains:
  - the original tensors, byte for byte;
  - `cls.output.weight`, built from the output (or tied embedding) rows of the
    contract's tokens;
  - rank pooling;
  - the classifier labels;
  - a `rerank` chat template rendered from the contract's template, with
    `{query}` and `{document}` placeholders.

  The derived GGUF is a new library model whose `conversion_source` records:
  - the source model;
  - the contract repository and its resolved revision.

  The source model is never modified.
- **The contract is discovered, not configured.** It is read from the source
  model's package when present. Otherwise Pumas fetches its few small files
  from the source model's recorded `base_model` repository through its own
  Hugging Face client. A request may name the contract repository explicitly.
  Nothing is inferred from model names.
- **The head layout is llama.cpp knowledge and stays in the llama.cpp
  conversion backend.**
  - For architectures whose rank pooling applies softmax over the head
    (`qwen3`, `qwen3vl` in b12099), the head is two rows:
    - the true token's row;
    - the false token's row, or zeros when the contract has no false token.
  - The served score is then `σ(ℓ_true − ℓ_false)`, which orders documents
    exactly as the reference score does.
  - Other architectures are refused with `unsupported` and the reason, until
    llama.cpp supports them. `qwen35` in particular applies `cls_out` in-graph
    as a projection.
- **Derivation verifies before it writes.** It refuses with a reason when:
  - the GGUF's token strings at the contract's ids differ from the contract
    tokenizer's;
  - the output rows are missing;
  - the template does not place each of `query` and `document` exactly once;
  - the architecture has no known head layout.
- **Serving is unchanged.** A derived GGUF is an ordinary reranker GGUF:
  `cls.output.weight` plus rank pooling.

## Alternatives considered

- **Score from token log-probabilities in generation mode.** No new file is
  needed, and it is exact for two-token contracts. Rejected because a
  one-token contract (zerank-2) needs the raw logit, which log-probabilities
  cannot recover. It would also add a second rerank protocol to serving.
- **Only accept GGUFs that already ship a head.** This remains the preferred
  path whenever a popular release has one. On its own it leaves zerank-2
  without a quantized path.
- **Patch the llama.cpp converter for each model.** Rejected. It is model
  specific, and it needs the full-precision safetensors, which are over the
  4 GB testing limit for 4B models.

## Consequences

- A derived model costs one more copy of the source GGUF on disk.
- New causal rerankers that publish a `LogitScore` contract need no Pumas code.
  New llama.cpp architectures need one entry in the backend's head layouts.
- If llama.cpp gains native support for these heads, the direction can be
  removed. Serving and derived models keep working.
