# Plan: Rerankers and embedders across llama.cpp, ONNX and PyTorch

**Plan status:** `Active`

**Current phase:** Milestone 0 (2026-11-09)

**Next slice:** M0: acquire the test and LFM3 models through Pumas

**Acceptance status:** `pending`

**Execution ledger:** [execution-ledger.md](execution-ledger.md)

**Decision:** [ADR 0006](../../adr/0006-derived-ranking-heads.md) (ranking heads).

## Objective

Consumers (phased-memory, Pantograph, Pumas-Playground) can run these models
through the typed `rerank` and `text_embedding` operations:
Qwen3-Reranker-0.6B and 4B, bge-reranker-v2-m3, zerank-2, Prism-Qwen3.5-Reranker,
harrier-oss-v1-270m, and the rerank and embedding models already in the
library. Each model runs on at least one runtime that suits it. Models over 1B
run from quantized GGUFs. Pumas reads each model's own declarations and holds
no per-model code.

The remaining LFM3 and d1 GGUFs are acquired so the
[modality primitives plan](../modality-primitives/plan.md) can finish S4.

## Objective Acceptance

| ID | Observable criterion | Kind | Environment | Mode | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | A ranking head derived from a headless Qwen3-Reranker-0.6B GGUF ranks the same pairs as sentence-transformers on the original safetensors. Every pair's score agrees within the Q8_0 tolerance recorded in the ledger. | `system` | `required-real` | `automated` | `pending` | pending |
| A2 | A true-token-only head (zerank-2's form) derived from the same 0.6B GGUF matches sentence-transformers `LogitScore` with no false token. zerank-2 Q4_K_M with its derived head reproduces its card's reference logits (5.41 and −4.50) within tolerance, and ranks them in that order. | `system` | `required-real` | `automated` | `pending` | pending |
| A3 | Prism-0.8B (library), bge-reranker-v2-m3 and Qwen3-Reranker-0.6B rerank on the Torch sentence-transformers adapter. Prism reproduces its card example (0.98, 0.01). | `system` | `required-real` | `automated` | `pending` | pending |
| A4 | harrier-oss-v1-270m embeds on llama.cpp (GGUF), ONNX and Torch. Each vector has cosine ≥ 0.99 with sentence-transformers on the safetensors, for the same inputs. | `system` | `required-real` | `automated` | `pending` | pending |
| A5 | bge-reranker-v2-m3 reranks on llama.cpp (GGUF) and ONNX. Scores agree with the Torch sentence-transformers scores (A3) within tolerance. | `system` | `required-real` | `automated` | `pending` | pending |
| A6 | Every rerank and embedding model in the library either serves, or reports `unavailable`/`unsupported` with a specific reason. Reasons cover over-4 GB safetensors, NVFP4/FP8 packages, missing weights, an encoder without its heads, and an unsupported head architecture. | `integration` | `representative` | `automated` | `pending` | pending |
| A7 | Existing v1 requests and results are unchanged. Model operations, consumer and ONNX tests pass. New fields and variants are additive. | `contract` | `simulated` | `automated` | `pending` | pending |
| A8 | LFM3-1.7B, LFM3-Omni-2B, LFM3-VL-2B, d1-2B and d1-Omni-2B GGUFs are in the library with their projectors, acquired through Pumas. | `integration` | `required-real` | `manual` | `pending` | pending |

## Scope

### In Scope

- **llama.cpp:** a conversion that derives a ranking head into a new GGUF from
  the source model's sentence-transformers `LogitScore` contract. Pooling for
  embedders follows each model's declaration.
- **Torch:** a sentence-transformers adapter that serves embeddings and rerank
  from local packages.
- **ONNX:** the pooling the package declares (mean, CLS, last token), external
  tensor data, an ONNX primary file inside `onnx/`, and cross-encoder rerank.
- **Classification:** cross-encoders tagged `text-classification` and
  mislabelled library entries.
- Contract, ADR and changelog updates.
- Downloads, all through Pumas RPC.

### Out Of Scope

- Prism above 1B on llama.cpp. llama.cpp applies a Qwen3.5 head as an in-graph
  projection, and its rank pooling aborts on it. Prism 2B and up are refused
  with that reason.
- Running the CLM-v0.1-8B head stack. Its heads are a separate PyTorch file.
- Testing safetensors packages over 4 GB.
- Rewriting downloaded artifacts in place.
- Pinning any runtime version.

## Constraints And Assumptions

### Constraints

- All model files arrive through Pumas download or import. Nothing is fetched
  by hand.
- Runtimes resolve current versions. A version that was confirmed working is
  recorded as evidence, not as a pin.
- Additive changes are non-breaking. Every refusal carries a reason.
  Capabilities are discovered, not configured.
- The frontend is a projection. Behaviour lives in the backend.

### Assumptions

- **A head derived from `LogitScore` reproduces the reference score.**
  - Decision it could change: M1 itself.
  - Validation: A1 and A2.
  - Owner: M1.
  - Stopping condition: a reproducible mismatch beyond quantization tolerance.
- **The source GGUF assigns the same token ids as the contract's tokenizer.**
  - Decision it could change: whether derivation is admitted.
  - Validation: the script compares the token strings at the contract's ids.
    A mismatch refuses.
  - Owner: M1.
- **Base models are recorded.** A GGUF repo's recorded `base_model` names a repo
  that publishes the contract.
  - Decision it could change: how the contract is found.
  - Validation: the M0 metadata check.
  - Fallback: the request names the contract repo explicitly. There is no
    guessing.

## Binding Decisions

| Decision | Owner | Evidence | Supersedes |
| --- | --- | --- | --- |
| Use a GGUF that already ships a head when a popular one exists. Derive a head only when none does. | Owner (2026-11-09) | Conversation | none |
| Ranking heads are derived by the conversion pipeline from the model's sentence-transformers `LogitScore` contract. | ADR 0006 | A1 and A2 | none |
| Qwen3-Reranker-4B uses Voodisss/Qwen3-Reranker-4B-GGUF-llama_cpp Q4_K_M. zerank-2 uses mradermacher Q4_K_M with a derived head. | Owner (2026-11-09) | Conversation | none |
| The QuantFactory Qwen3-Reranker-4B in the library stays refused, with a reason that names the fix. | Owner (2026-11-09) | Conversation | none |

## Design Observations And Remaining Evidence

| Assumption or claim | Decision affected | Owner | Executable procedure | Required environment | Status/evidence |
| --- | --- | --- | --- | --- | --- |
| llama.cpp RANK pooling applies softmax only for `qwen3` and `qwen3vl`. Other archs take the first output raw. | Head layout per architecture | M1 | Source reading at b12099 and master (`build_pooling`, `send_rerank`); confirmed by A1 | llama.cpp b12099 | Source read; live pending |
| `qwen35` applies `cls_out` in-graph as a projection, so RANK pooling cannot use it. | Prism scope | M1 | Source reading (`models/qwen35.cpp`, `build_pooling`) | b12099 | Source read |
| mradermacher Qwen3-Reranker-0.6B and zerank-2 GGUFs carry no head. | Oracle source | M0 | HF parameter totals; GGUF tensor list after download | Library | Totals read; tensors pending |
| harrier's GGUF declares, or lacks, the pooling it needs (last token, causal). | M4 pooling discovery | M4 | GGUF metadata after download | Library | pending |

## Evidence And Oracle Plan

| Claim | Domain | Deciding oracle | Independent authority | Unsupported domain | Intended negative failure |
| --- | --- | --- | --- | --- | --- |
| A1, A2 | Semantics | Sentence-transformers `CrossEncoder` scores on the original safetensors. zerank-2's card reference logits. | sentence-transformers 5.x; ZeroEntropy model card | Models over 4 GB as safetensors | Token-id mismatch, missing output tensor, and unsupported architecture each refuse with their own reason |
| A3 | Semantics | Prism card example scores. Sentence-transformers is the adapter's own implementation, so A3 proves wiring, not scoring. | Model card | none | Package without a `CrossEncoder`/`SentenceTransformer` declaration is refused |
| A4, A5 | Cross-runtime conformance | Cosine and score tolerance against the sentence-transformers reference | sentence-transformers on the safetensors | ONNX exports without a declared pooling | ONNX output of an undeclared shape is refused |

## Simplicity And Ownership Review

**Applicability:** `applicable`

- **Independent concepts:**
  - The **scoring contract**: which tokens score, and the prompt. The model's
    repo owns it.
  - The **head layout**: how llama.cpp reads a head. llama.cpp owns it; the
    llama.cpp conversion backend encodes it.
  - **Serving semantics**: rerank and embeddings. Model operations owns them.
  - **Runtime adapters**: what each runtime can run. Each provider owns its own.
- **State and authority:**
  - A derived GGUF is a new library model. Its `conversion_source` records the
    source model, the contract repo and revision, and the direction.
  - The source model is never modified.
  - Model operations reads the result like any other reranker GGUF:
    `cls.output.weight` plus rank pooling.
- **Caller knowledge:** serving needs no knowledge of derivation. A derived
  GGUF is an ordinary reranker GGUF.
- **Representative changes:**
  - A new causal reranker that publishes a `LogitScore` contract on a `qwen3`
    base needs no code.
  - Supporting another architecture means one entry in the llama.cpp backend's
    head layouts, added once llama.cpp handles that architecture.
- **Stable interfaces:**
  - `ConversionDirection` gains one variant.
  - `ConversionRequest` gains one optional field: the contract repo override.
- **Deletion:** if llama.cpp gains native support for these heads, the
  direction and script can be deleted. Serving is unaffected.

## Milestones

### Milestone 0: Acquisition

**Goal:** the test models and LFM3 GGUFs are in the library, acquired through
Pumas, with their GGUF facts recorded in the ledger.

**Allowed write set:** `docs/plans/rerank-embed-runtimes/execution-ledger.md`

**Status:** `Active`

### Milestone 1: Derived ranking heads (llama.cpp)

**Goal:** A1 and A2.

**Allowed write set:**
- `rust/crates/pumas-core/src/conversion/**`
- the conversion RPC contract and handlers in `rust/crates/pumas-rpc/src/`
- ADR 0006
- `docs/contracts/model-operations-v1.md`
- `CHANGELOG.md`

**Status:** `Planned`

### Milestone 2: Torch sentence-transformers adapter

**Goal:** A3, plus the Torch part of A4.

**Allowed write set:**
- `torch-server/**`
- the Torch runtime installer and client in `rust/crates/pumas-app-manager/`
- Torch serving and dispatch in `rust/crates/pumas-rpc/`
- `rust/crates/pumas-core/src/providers/mod.rs`

**Status:** `Planned`

### Milestone 3: ONNX pooling and cross-encoder rerank

**Goal:** the ONNX parts of A4 and A5.

**Allowed write set:**
- `rust/crates/pumas-core/src/onnx_runtime/**`
- the ONNX serving and gateway handlers
- primary-file selection in `model_library`

**Status:** `Planned`

### Milestone 4: llama.cpp embedder pooling, classification, library sweep

**Goal:** the llama.cpp part of A4, the llama.cpp part of A5, and A6.

**Allowed write set:**
- `runtime_profiles.rs`
- `package_facts/gguf.rs`
- the model-type resolver
- the model operations semantics

**Status:** `Planned`

## Blockers

- `none`

## Re-Plan Triggers

- A1 fails beyond quantization tolerance. The head layout or the template
  rendering is then wrong, and M1 is redesigned before zerank-2.
- A base GGUF's recorded `base_model` cannot locate its contract.
- A Torch install cannot hold the sentence-transformers adapter beside an
  existing adapter without a second full Torch install.

## Final Acceptance

- Acceptance status: `pending`
- Deferred follow-ups: Prism ≥2B on llama.cpp (needs llama.cpp support), CLM heads
- Final status: `Active`
