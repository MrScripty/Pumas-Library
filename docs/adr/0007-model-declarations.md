# ADR 0007: Sentence-transformers declarations are library facts

Status: Accepted (2026-11-09). Implementation is tracked in the
[rerank and embedding runtimes plan](../plans/rerank-embed-runtimes/plan.md).

Affected boundaries: model metadata (`metadata.json`
`sentence_transformers`), the Hugging Face client, the
`resolve_model_declarations` RPC, and every runtime adapter that pools,
normalizes or scores (ONNX Runtime, the Torch sidecar, ranking-head
derivation).

## Context

Embedding and reranking models declare how to use them through their
sentence-transformers package:

- `modules.json` lists the pipeline;
- `1_Pooling/config.json` gives the pooling (mean, CLS, last token, ...);
- a Normalize module L2-normalizes embeddings;
- a `LogitScore` module names the scored tokens of a causal cross-encoder;
- `config_sentence_transformers.json` and `sentence_bert_config.json` give the
  prompts, activation and template options;
- `chat_template.jinja` holds the prompt template.

Exported formats usually drop these files. ONNX and GGUF repositories carry
the weights and tokenizer and name the original repository as their
`base_model`. Without the declarations:

- an ONNX export of a last-token embedder is mean-pooled into wrong vectors;
- a headless reranker GGUF cannot be given its ranking head.

Library copies can also be incomplete: a download that selected only some
files may lack `modules.json`.

## Decision

- **Declarations are a validated record in the model's metadata.** The record
  holds:
  - pooling, normalization, the `LogitScore` tokens and their strings;
  - the chat template, its options and its special-token strings;
  - prompts, activation and module types;
  - each module's directory and role (`module_details`), with the layer of a
    Dense module (sizes, bias, activation) and the dimension of a LayerNorm.
    Their weights stay in the model's package; a runtime that cannot apply a
    declared module refuses with the reason instead of skipping it;
  - where the record was read: the package, or a repository at an immutable
    commit.

  `model_library::declarations` is the only parser of the package files.
  Runtimes read the record, not the files.
- **They are resolved from the model's lineage.** The search order is:
  1. the model's own package;
  2. its source repository;
  3. its single recorded base model.

  A caller may name the repository instead. The first repository that
  publishes `modules.json` wins; one whose files fail to fetch or validate
  does not end the search, and when none declares them the failures are
  returned with their reasons. The source repository is read at the commit the
  model's files were downloaded at (`upstream_revision`); a base model, which
  records none, at its current `main`. The commit read is kept in the record.
  Files are fetched by the Hugging Face client into scratch space outside the
  library, bounded by Hugging Face's size facts and LFS digests, and a
  transfer that delivers no data for a minute is abandoned. Only the record is
  kept, written by changing only the model's `sentence_transformers` field.
- **Resolution happens once, off the serving path.**
  - A reactor follows the library's update feed and resolves each model that
    becomes complete without a record, whether a download, an import or a
    conversion completed it. It is best-effort: a failure is logged and
    retried on the model's next change, and never fails the operation. (A
    download's record exists from the moment it starts, so completion is the
    change that clears `download_incomplete`, not the model being added.)
    Only embedders, rerankers and decision models are resolved. Reading the
    feed never waits on a fetch; if the feed lags anyway, the library is
    rescanned for complete models without a record. The reactor stops at
    shutdown, cancelling a fetch in progress.
  - `resolve_model_declarations` resolves on request, and replaces the record.
  - Ranking-head derivation resolves at admission when the record is missing.

  Serving never fetches.
- **A lineage with no package is a result, not an error.** It returns
  `not_declared` with the repositories searched. Consumers then use their
  documented behaviour without declarations, or refuse with that reason.

## Alternatives considered

- **Each operation fetches what it needs.** Rejected. The lookup is
  duplicated in every consumer, and serving would need network access.
- **Use only the downloaded package.** Rejected. Common ONNX exports would
  produce wrong vectors, and headless reranker GGUFs could not be repaired.
- **Copy the package files into the model directory.** Rejected. The files
  would change the package's identity and classification. The validated
  record is all the consumers use.

## Consequences

- Models acquired before this change have no record until it is resolved, by
  request or by derivation admission.
- A record names its source and commit, so it can be audited and refreshed.
- The derived ranking-head model inherits its source's record. It scores by
  the same contract.
