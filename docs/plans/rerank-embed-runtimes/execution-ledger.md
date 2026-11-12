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
- **Library errors fixed (2026-11-10, user report):**
  - **Two stranded copy-imports.** The cause was a metadata refresh that
    overlapped the import's hash pass, so the final index commit conflicted.
    Fixed on this branch (ADR 0008). Running `finish_retained_import` on them
    still refused: their receipts recorded device 259:6, and the NVMe
    partition is 259:8 since the 11-09 reboot (Linux file identity is
    `st_dev` plus inode). Their payloads hashed equal to their receipts. They
    were deleted and re-imported from their own files through Pumas, and the
    catalog now shows 116 models and no `unavailable_records`. Moving Linux
    identity to the filesystem's stable ID is a separate, later change.
  - **Five LFM3 downloads in error with no reason.** The reason was not
    persisted (fixed; records from before the fix now say so), and they had
    failed on the HF CDN query reordering (#126). All were resumed.
  - **The Torch v2.15.0 install failed** with "exceeded the installation
    deadline" (a flat one hour per stage) while still downloading at about
    80 KB/s. A stage now fails only after 30 minutes without bytes or log
    output. The install was restarted.
  - **Registry row left behind on a graceful shutdown.** The failed install
    remained an owned-task error, so shutdown was "incomplete" and the row
    stayed. I removed it by hand after a backup (`registry-backup-2026-11-10b.db`).
    Telling a failure whose effects were settled from one whose effects were
    not needs a typed installer error; that is still open.
- **Open items closed (2026-11-10):**
  - **Linux volume identity** is now the filesystem UUID from udev by-uuid,
    with the device number as fallback. Records that hold a device number are
    translated when read. Pinned and legacy-record tests pass.
  - **A failed Torch install no longer fails shutdown or keeps the registry
    row.** A test extending `torch_direct_install_enters_stage…` asserts that
    shutdown succeeds after a failed install. It failed before the change.
    The native-install custody gate is unchanged.
  - **Torch text serving on v2.14.0** is refused with the capability reason.
    That runtime carries the sidecar copy from before the text routes, so the
    checks wait for the v2.15.0 install. The changelog no longer mentions a
    "repair" operation, because none exists.
  - **Pending live check after the next restart:** in-flight downloads keep
    their custody under the new volume IDs, and a graceful shutdown removes
    the registry row.
- **A1 (2026-11-12): pass.** Qwen/Qwen3-Reranker-0.6B safetensors completed.
  On the oracle pairs, the head derived from the headless mradermacher
  Q8_0 GGUF agrees with sentence-transformers CrossEncoder (bf16, local
  package) to within 0.24 in logit space (l_yes − l_no), in the same order
  on every pair. Script: `scratchpad/oracle/a1_compare.py`.
- **zerank-2 (A2, partial).** Its declarations come from
  `zeroentropy/zerank-2-reranker@5eae30d5`: true token "Yes" 9454, no false
  token, Identity. `gguf_to_ranking_gguf` completed on the Q4_K_M GGUF. On the
  card's pairs ("What is 2+2?" against "4" and against "The answer is
  definitely 1 million"), the derived head gives logits 4.42 and −4.46. The
  card's bf16 values are 5.41 and −4.50. The order is correct, and the
  negative pair is within 0.04. The positive pair is 1.0 low, which is
  plausibly Q4_K_M quantization, but that has not been confirmed: the bf16
  package exceeds the 4 GB testing limit. The true-token-only half of A2
  (the 0.6B against sentence-transformers) is still pending.
- **Network outage (2026-11-11/12).** DNS failed for download-r2.pytorch.org
  and the Torch v2.15.0 install stopped at 415/540 MB ("incomplete-download");
  it was restarted. Model downloads continued.
- **Torch v2.15.0 (2026-11-12): installed, active and default.** It ships
  Transformers 5.22.2 and torch 2.15.0+cu134.
  - **Prism-0.8B, served:** card pairs give 0.867 and 0.294. Sentence-
    transformers in the same runtime gives identical values (parity script:
    difference 0.0), and fp32 gives 0.874 and 0.309. The card's 0.98/0.01
    comes from its CrossEncoder example, which loads
    `infgrad/Prism-Qwen3.5-Reranker-4B`, the 4B sibling. Pumas matches the
    reference library for the 0.8B. Batching moves the first pair slightly
    (0.881 alone vs 0.867 batched): Qwen3.5 linear attention runs on the
    reference implementation without causal_conv1d or flash-linear-attention.
    That matches sentence-transformers' batched output.
  - **Qwen3-Reranker-0.6B, served:** within bf16 of sentence-transformers
    (8.69 vs 8.75 …), same order on every pair.
  - **bge-reranker-v2-m3 (loader parity in v2.15.0):** identical to
    sentence-transformers CrossEncoder (difference 0.0). harrier-270m:
    cosine 1.001 against SentenceTransformer, because bf16 normalization
    leaves norms slightly off 1. Serving both was blocked by the sidecar's
    128-character model-name limit, fixed in `99a1f4ba`; the installed v2.15.0
    still carried the old sidecar copy.
  - **bge and harrier served on Torch (2026-11-12, v2.15.0 reinstalled through
    Pumas with the `flux2` adapter from the local pip cache):** bge gateway
    scores equal the in-process loader (difference 0.0); harrier gateway
    embeddings have cosine 1.0 against it (640 dimensions). The first
    reinstall used the default `none` adapter and lacked Transformers. Its
    refusal named a "Transformers adapter", which does not exist, so the
    message needs to name the adapters that do. A separate PR makes runtimes
    run the sidecar shipped with Pumas, so sidecar fixes stop requiring a
    reinstall.
  - **Declarations, found live and fixed in `d5969a20`:** named
    `pooling_mode` (EmbeddingGemma 2, Qwen3-VL-Embedding), and lineage search
    past a partial package (local Qwen3-Embedding-0.6B).
  - **ONNX model IDs** were also capped at 128 bytes (harrier and bge ONNX);
    fixed in `b14bcc6f`.
- **Disk:** the library drive reached 0 bytes free from build output. My
  worktree's target was 79 GB, 52 GB of it incremental cache, which I removed.
  The main checkout's `rust/target` is 317 GB and `worktrees/integration` is
  48 GB; neither is mine.
- **Cross-runtime checks (2026-11-12), against the Torch references:**
  - harrier GGUF (llama.cpp): cosine 0.9993–0.9998.
  - harrier ONNX (fp16, pre-pooled `sentence_embedding`): cosine ≥ 0.9995
    once pooled outputs are read (`f402c09c`).
  - bge ONNX (INT8): same order on both oracle sets, within about 0.5 logit.
  - bge GGUF (Q8_0): same order except one near-tie (−3.30/−3.56 vs Torch
    −3.29/−3.08).
  - Voodisss Qwen3-Reranker-4B Q4_K_M (its own head): matches the derived 4B
    Q8_0 head; the only order difference is a 0.019 vs 0.018 near-tie.
- **LFM3 / modality primitives S4:** see the modality-primitives plan. All
  four remaining models pass their live checks.
- **Library change by the user:** five Qwen3.5/3.6 GGUFs were deleted
  through Pumas at 08:11–08:13 to free disk space (user-confirmed).
