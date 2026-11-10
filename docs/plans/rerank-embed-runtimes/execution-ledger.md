# Execution ledger: rerankers and embedders

## M0 acquisition (through Pumas RPC only)

| Purpose | Repo | Files | Size | Status |
| --- | --- | --- | --- | --- |
| LFM3 (resume) | LiquidAI/LFM3-Omni-2B-GGUF | `LFM3-Omni-2B-Q4_K_M.gguf`, `mmproj-LFM3-Omni-2B-Q8_0.gguf` | 1.6 GB | pending |
| LFM3 (resume) | LiquidAI/LFM3-VL-2B-GGUF | `LFM3-VL-2B-Q4_K_M.gguf`, `mmproj-LFM3-VL-2B-Q8_0.gguf` | 1.3 GB | pending |
| LFM3 (resume) | LiquidAI/d1-2B-GGUF | `d1-2B-Q8_0.gguf`, `mmproj-d1-2B-Q8_0.gguf` | 2.1 GB | pending |
| LFM3 (resume) | LiquidAI/d1-Omni-2B-GGUF | `d1-Omni-2B-Q8_0.gguf`, `mmproj-d1-Omni-2B-Q8_0.gguf` | 2.2 GB | pending |
| LFM3 | LiquidAI/LFM3-1.7B-GGUF | `LFM3-1.7B-Q4_K_M.gguf` | 1.1 GB | pending |
| Qwen3-Reranker-4B (has a head) | Voodisss/Qwen3-Reranker-4B-GGUF-llama_cpp | `Qwen3-Reranker-4B-Q4_K_M.gguf` | 2.5 GB | pending |
| A1 subject (headless) | mradermacher/Qwen3-Reranker-0.6B-GGUF | `Qwen3-Reranker-0.6B.Q8_0.gguf` | 0.64 GB | pending |
| A1 oracle, Torch | Qwen/Qwen3-Reranker-0.6B | full package | 1.2 GB | pending |
| zerank-2 (headless) | mradermacher/zerank-2-GGUF | `zerank-2.Q4_K_M.gguf` | 2.5 GB | pending |
| bge-reranker, llama.cpp | gpustack/bge-reranker-v2-m3-GGUF | `bge-reranker-v2-m3-Q8_0.gguf` | 0.64 GB | pending |
| bge-reranker, ONNX | onnx-community/bge-reranker-v2-m3-ONNX | `onnx/model_int8.onnx`, tokenizer, config | 0.6 GB | pending |
| bge-reranker, oracle and Torch | BAAI/bge-reranker-v2-m3 | full package | 2.3 GB | pending |
| harrier, Torch and oracle | microsoft/harrier-oss-v1-270m | full package | 0.57 GB | pending |
| harrier, llama.cpp | mykor/harrier-oss-v1-270m-GGUF | `harrier-oss-v1-270M-Q8_0.gguf` | 0.29 GB | pending |
| harrier, ONNX | onnx-community/harrier-oss-v1-270m-ONNX | `onnx/model_fp16.onnx` (+ data), tokenizer, config | 0.6 GB | pending |
| Prism-0.8B contract files | infgrad/Prism-Qwen3.5-Reranker-0.8B | `modules.json`, `config_sentence_transformers.json`, `sentence_bert_config.json` | KB | pending |

Free space before M0: `/media/jeremy/OrangeCream` 76 GB.

### M0 notes (2026-11-09)

- Sixteen concurrent downloads at about 4 MB/s total; at 21:49Z every
  in-flight resume was refused with `HTTP continuation representation or
  resource changed` (signed CDN redirect changed). Single resumes succeed; a
  queue keeps four active. Full-repo downloads with non-LFS files
  (`Qwen/Qwen3-Reranker-0.6B`, `BAAI/bge-reranker-v2-m3`) cannot resume
  (no receipt for partial non-LFS files) and are restarted after deleting the
  partial package once no download holds the library.
- `gpustack/bge-reranker-v2-m3-GGUF` and its ONNX/safetensors siblings were
  classified `decision` (HF `text-classification`, `*ForSequenceClassification`):
  M4 classification fix.
- `mykor/harrier-oss-v1-270m-GGUF` declares `gemma3.pooling_type = 3` (last
  token); llama.cpp uses it without a `--pooling` flag. Its record has no
  `base_model`.

## M1 structural evidence

Derivation script on `mradermacher/Qwen3-Reranker-0.6B-GGUF` Q8_0 (headless)
with the `LogitScore` contract of `Qwen/Qwen3-Reranker-0.6B` (true 9693
`yes`, false 2152 `no`, its `chat_template.jinja`), compared with
`ggml-org/Qwen3-Reranker-0.6B-Q8_0-GGUF` (head from llama.cpp's converter):

| Property | Derived | ggml-org |
| --- | --- | --- |
| `cls.output.weight` rows | F32, cosine 1.0 and max abs diff 0.0 vs dequantized reference | Q8_0 |
| `qwen3.pooling_type` | 4 (rank) | 4 |
| classifier labels | `yes`, `no` | `yes`, `no` |
| `tokenizer.chat_template.rerank` | identical bytes | converter's hard-coded template |

This is structural evidence; A1's score agreement with sentence-transformers
remains pending the reference safetensors and a served session.

## Live evidence (2026-11-10, pumas-rpc from this branch)

- **Download resumes:** root cause of the mass `HTTP continuation
  representation or resource changed` failures: Hugging Face's CDN
  (`us.aws.cdn.hf.co/xet-bridge-us`, CloudFront-signed) returns the same query
  parameters in a different order on each redirect. Fixed by comparing retained
  parameters as a sorted multiset (PR #126; also on this branch). No recurrence
  after restart.
- **Declaration resolution:** `resolve_model_declarations` on
  `mradermacher/Qwen3-Reranker-0.6B-GGUF` searched the repository (no
  `modules.json`), then its card's base model `Qwen/Qwen3-Reranker-0.6B` at
  `e61197ed…`. It recorded `CrossEncoder`, `LogitScore` true 9693 `yes`, false
  2152 `no`, activation Identity, and the 741-character template. This needed
  two fixes found live: base model from `model_card`, and declaration files
  fetched without the API client's 30 s total timeout.
- **Conversion through Pumas:** `setup` → `check_conversion_environment` ready
  (jinja2 probe) → `start_model_conversion` `gguf_to_ranking_gguf` →
  completed. 311 tensors; output `…-ranking`, `model_type` reranker,
  `pipeline_tag` text-ranking, declarations inherited.
- **Serving and rerank:** served on llama.cpp b12099 (dedicated, GPU).
  `/v1/served-models` reports `rerank` available and `text_embedding`
  `model_task_mismatch`. Typed `rerank` on two queries × eight documents
  ranks relevant documents first (for example 0.9999 / 0.9751 / 0.8437 for the
  three boiling-point passages vs ≤0.0004 for unrelated ones).
- **Still pending:**
  - A1 numeric agreement with sentence-transformers. The reference
    safetensors are still downloading.
  - The ggml-org comparison run. Serving that older entry is refused with
    `conflict` while downloads hold the library.
- **Library resilience (found live):** one record with
  `dependency_bindings: null` (`audio/lfm3/lfm3-audio-2b-gguf`, a copy-import
  from 2026-11-07) failed `get_models`, so the desktop showed "Library
  unavailable". The catalog now leaves such records out and lists them in
  `unavailable_records` with the reason. After the fix the live library
  loads 112 models. Two records are reported: that one and
  `reranker/qwen3/qwen3-reranker-0_6b-q8_0-gguf`. Both are copied
  publications that never acknowledged a Ready index; Pumas refuses to
  reclassify them and has no recovery API for them.
- **Derived 4B head:** `qwen3-reranker-4b-gguf` (QuantFactory, Q8_0) had no
  ranking head and its card names no base model, so
  `resolve_model_declarations` with `repo_id` `Qwen/Qwen3-Reranker-4B`
  recorded the declarations (at `22e68366…`; yes 9693, no 2152, Identity).
  `gguf_to_ranking_gguf` completed, and the output served on llama.cpp
  ranks the oracle pairs in the expected order (top document 0.996 and 0.997;
  unrelated documents ≤0.03). Results are in `scratchpad/oracle/derived_4b.json`.
- **Found live: derived models were hidden.** A derived model inherited its
  source's `repo_id` and quant, so duplicate-artifact grouping folded it into
  its source and `get_models` never listed it. Fixed on this branch.
- **Sweep of refusals** (`validate_model_serving_config`, complete packages):
  every embedding GGUF, the bge GGUF, Prism, and the Torch safetensors pass.
  Refused with a reason: reranker GGUFs without a ranking head
  (`CLM-v0.1-8B-GGUF` has no sentence-transformers package in its lineage
  (`czl/CLM-v0.1-8B-GGUF`, `Contrastive-LM/CLM-v0.1-8B`), so no head can be
  derived).
- **ONNX live (nomic-embed-text-v1.5 ONNX, CPU profile `onnx-text-live`):**
  declarations resolved from `nomic-ai/nomic-embed-text-v1.5@e9b67630`
  (mean pooling, no Normalize, max_seq_length 8192). `serve_model` failed
  with a bare "could not load". The cause, found by loading the same
  package through `OnnxRuntimeSession::load`, was `config.json`'s differing
  `max_position_embeddings` (2048) and `n_positions` (8192). Both are fixed
  on this branch: the smaller limit applies, and load refusals carry their
  reason. The loader then embeds with ONNX Runtime 1.24.2. A dev backend
  needs `ORT_DYLIB_PATH`; packaged builds ship the library.
