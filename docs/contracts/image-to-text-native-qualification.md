# Native Image-to-Text qualification

Status: candidate selected; acquisition and native execution are pending. This is the next qualification slice for the
[existing adapter](image-to-text-v0.8.md), not a shipping runtime policy or a
receipt that model bytes have been read. The machine-readable
[cohort selection](image-to-text-native-cohort.json) contains publisher pins and
explicitly null acquired identities. Do not substitute this document for an
owner-issued acquisition, import, installation or native-session receipt.

## Selected upstream cohort

Use ggml-org's **SmolVLM-256M-Instruct Q8_0** with the **F16 projector** from
one repository revision, `b9e4379657e1450d04d02eec8e345667265b0a00`.
The [official metadata](https://huggingface.co/api/models/ggml-org/SmolVLM-256M-Instruct-GGUF?blobs=true)
reports public, ungated Apache-2.0 assets and individual LFS SHA-256 values.
The [base model card](https://huggingface.co/HuggingFaceTB/SmolVLM-256M-Instruct)
describes an English image/text model. It does not establish the quality of our
quantized runtime cohort.

The runtime candidate is the official llama.cpp **b11429 Ubuntu x64 CPU**
archive, asset ID `613152023`, at source
`d81235049384534c167caea52b85a694f6103d14`.
The [release](https://github.com/ggml-org/llama.cpp/releases/tag/b11429) supplies
its publisher SHA-256 through GitHub release metadata. The
[pinned multimodal guide](https://github.com/ggml-org/llama.cpp/blob/d81235049384534c167caea52b85a694f6103d14/docs/multimodal.md)
explicitly lists SmolVLM-256M and separate local model/projector arguments.
The [pinned server protocol](https://github.com/ggml-org/llama.cpp/blob/d81235049384534c167caea52b85a694f6103d14/tools/server/README.md)
documents the adapter's `modalities`, string `build_info`, `is_sleeping`, chat
image parts and model alias. This supports choosing the candidate; compatibility
must still be observed with these exact installed bytes. Runtime source is MIT;
retain the archive's actual third-party notices as well.

The selected downloads total 382,779,606 bytes: 365,086,144 model/projector bytes
plus a 17,693,462-byte runtime archive. Allow at least 2 GiB free disk for
acquisition staging, copied model publication and runtime extraction, and 4 GiB
free RAM for an initial CPU run. These are conservative planning budgets, not
measured peak usage. Use one slot, four CPU threads, a 2048-token context and
at most 128 generated tokens initially. Use `--n-gpu-layers 0` and
`--no-mmproj-offload`; no GPU or paid service is required.

## Import roles

The receipt-bound two-GGUF import this cohort needs is implemented; the
[adapter contract](image-to-text-v0.8.md#receipt-bound-acquired-modelprojector-roles)
owns its behavior. This cohort's projector (**190,031,616 bytes**) is larger
than its primary (**175,054,528 bytes**), so qualification must confirm that
the explicitly selected primary keeps the model hashes and load target.

## Real execution sequence

Use the existing acquisition service and llama.cpp `VersionInstaller` configured
with that service. Resolve the exact release asset and HF revision/member
metadata, compare publisher size/SHA-256 to streamed bytes, and retain all
owner-issued receipts. Do not use llama.cpp's `-hf` auto-download, a new ad-hoc
downloader, ambient cache discovery, or hand-edited model-index rows.

Publish the selected pair
through that owner and launch a managed `LlamaCppDedicated` CPU profile through
Pumas. Record the canonical model ID, profile, process generation, runtime
executable/dependent-library identities and model/projector hashes. Require the
ordinary live readiness query; do not inject an available descriptor.

Run both image-only caption and ordered image/text annotation via Pumas
`/v1/model-operations`, with real PNG and JPEG stimuli and finite options.
Record exact input hashes and returned typed text, including failures and
semantic mistakes. A purpose-built image with known colored objects may be a
controlled stimulus, but the response must come from the real pinned model;
it must never be replaced with a fixture reply. Inspect semantic content against
the visible stimulus; nonempty text alone does not establish caption quality.

Observe disconnect and shutdown during real admitted generation. Verify that
there is no replay, the exact managed child/session drains, no runtime process
remains, and a subsequent owned launch can complete another real request. HTTP
closure alone cannot pass native teardown. Tuldok application acceptance is
owned separately and must pin the final combined Pumas source/schema.
