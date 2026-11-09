# Plan: Imports use the download pipeline

**Plan status:** `Active`

**Goal:** a local import is a download whose bytes are already on disk. Pumas
searches backwards from the files to the repository, revision and file set
they came from, then ingests them through the same preflight, verification,
custody and finalization a download uses. Imported and downloaded models end
up with identical records. There is no second registration path.

**Baseline:** `origin/main` at `ca68f41e`, plus
[#119](https://github.com/MrScripty/Pumas-Library/pull/119) (copy imports keep
the caller's `repo_id`), which is the stopgap until this lands.

**Target:** 0.8.x, the first change after 0.8.0.

**Current phase:** planning (2026-11-06).

## Why

The backend import (`import_model`, `import_batch`) makes no Hugging Face
call. It writes a record with an unknown task, unknown modalities and no
license, and marks it `pending_online_lookup`, a flag nothing processes.
Identification lives in the desktop import wizard instead
(`useModelImportWorkflow`, `modelImportMetadataLookup`,
`buildEmbeddedMetadataMatch`): it runs `lookup_hf_metadata_for_file` and
forwards only part of the match. RPC consumers, Pantograph and scripts
therefore get incomplete records. Backend functionality must not live in the
frontend; the UI is a projection.

Evidence from 0.8.0 testing (LFM3-Audio-2B Q4_0, Qwen3-Reranker-0.6B Q8_0):

- `import_model` with `repo_id` stored `repo_id: null` (fixed by #119).
- The filename lookup matched `lfm3-audio-2b-q4_0.gguf` to
  `LiquidAI/LFM3-Audio-2B` (the safetensors original) as `filename_exact` at
  confidence 1.0. The true source is `LiquidAI/LFM3-Audio-2B-GGUF`.
- It matched the reranker to `arthurcollet/Qwen3-Reranker-0.6B-mlx-6bit` as
  `filename_fuzzy` at confidence 0.67, with `requires_confirmation: false`.
  The true source is `ggml-org/Qwen3-Reranker-0.6B-Q8_0-GGUF`.
- The lookup used the library's renamed file name
  (`qwen3-reranker-0_6b-q8_0.gguf`), not the recorded original
  (`qwen3-reranker-0.6b-q8_0.gguf`).
- `refetch_metadata_from_hf` without a `repo_id` derives one from the model
  ID's second and third segments. For imports the second segment is the
  family, not the owner (`audio/lfm3/lfm3-audio-2b-gguf` gives
  `lfm3/lfm3-audio-2b-gguf`).
- All three files matched their true repository's published LFS SHA-256
  exactly. A hash check identifies the source with certainty where the name
  heuristics did not.

## Design

1. **Reverse search (backend).** Candidates come from, in order:
   1. a caller-claimed `repo_id`;
   2. GGUF embedded metadata (`general.repo_url`, `general.base_model.*`,
      `general.name`, `general.basename`, `general.quantized_by`);
   3. original file names;
   4. Hugging Face search.

   A candidate is **confirmed** only when every local file's SHA-256 equals
   the repository's published LFS SHA-256 at a resolved revision. Anything
   weaker is returned as a candidate needing confirmation and is never
   applied silently.
2. **Plan.** From the confirmed source, Pumas builds the `DownloadRequest` a
   user would have made (repository, revision, files). Today
   `prepare_hf_download_owned_at_revision` returns only the snapshot and
   Hugging Face evidence; file selection (projector, shards, config and
   tokenizer companions) and the destination are decided inside
   `start_prepared_hf_download_owned`, which also starts the download. S2
   extracts that selection into a shared, read-only planning step that returns
   the selected files, artifact ID and proposed destination without side
   effects. Downloads and imports then both call it.
3. **Completeness.** The local files are compared with the selected package.
   A missing companion (for example the `mmproj`) or a truncated file is
   reported by name. With the caller's consent, the existing download custody
   (`recover_download`, `resume_partial_download`) fetches only the missing
   bytes.
4. **Ingest.** Local bytes are placed at the download destination through the
   same staging and verification a download uses. The only difference is that
   the source is a local copy instead of the network, checked against the
   published SHA-256. Then `finalize_downloaded_directory_at_revision`
   registers the model, so the metadata and index rows match a download
   exactly.
5. **No match or offline.** The model is imported as today and keeps
   `pending_online_lookup`. A backend job runs the reverse search when the
   network is available. Candidates that need confirmation are listed for the
   user.
6. **Projection.** A backend `plan_model_import` RPC returns, per path, the
   identification (evidence, confidence, candidates), the selected package,
   anything missing and the destination. The wizard renders it and sends back
   confirmations. The frontend contains no matching logic.

## Acceptance

1. Importing a file whose bytes match a Hugging Face repository yields a
   record equal to a download of the same files at the same revision:
   `repo_id`, `upstream_revision`, Hugging Face evidence, `pipeline_tag`,
   task, modalities, license, model card, artifact ID, expected files and
   storage path. Tests compare an import with a download field by field.
2. An import missing a companion or holding a truncated file reports what is
   missing by name. With consent it completes through download custody;
   without consent it is refused with that reason.
3. Weak matches are never applied without confirmation. Each lookup bug above
   has a regression test.
4. RPC, native crate and desktop imports run the same backend path. The
   wizard's matching code is removed.
5. Existing imported records can be re-identified without re-copying. The two
   0.8.0 test imports above are repaired this way.

## Slices

| Slice | Content | Status |
| --- | --- | --- |
| S1 | Reverse search with hash confirmation; tests for each lookup bug | Next |
| S2 | Shared read-only download planning (selection, artifact ID, destination) extracted from `start_prepared_hf_download_owned`; completeness; `plan_model_import` RPC | Planned |
| S3 | Ingest through download staging and finalization; import/download equivalence tests | Planned |
| S4 | Completion of missing or truncated files through download custody | Planned |
| S5 | Backend pending-lookup job; re-identifying existing records | Planned |
| S6 | Wizard becomes a projection of `plan_model_import`; remove frontend matching | Planned |

## Blockers and decisions

- **D1, consent for fetching missing bytes (S4).** The options are a
  per-import flag, or always asking through the plan. Fetching without
  consent is not an option.
- **D2, Diffusers and external directories.** These keep their current
  registration paths until S3 proves the equivalence for single-file and GGUF
  packages.
