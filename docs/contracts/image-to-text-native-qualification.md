# Native Image-to-Text qualification

Status: candidate selected; acquisition, import extension and native execution
are pending. This is the next qualification slice for the
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

## Acquisition owner integration proposal

The acquisition integration lead owns the changing generic importer. Keep this
extension separate from its unrelated format and transport edits. The proposed
new helper is `model_library/importer/acquired_vision.rs`; reserve its write
ownership before implementation. Required wiring remains with the importer
owner. Do not simply add `gguf` to the inert auxiliary extension allowlist.

1. Add an explicit closed vision-pair selection with a primary import spec,
   projector logical path, and `image_to_text` semantic task. Initial admission
   is exactly two distinct, same-directory, digest-pinned GGUF members. The
   primary must not be a projector and the secondary must be the one selected
   projector. Refuse extra GGUFs, missing roles, equal paths, ambiguous pairs,
   nested placement that breaks sibling association and unverified members.
2. Bind that complete selection to the existing consumer receipt payload.
   Validate and copy both through `AcquiredArtifactUse` held descriptors and
   the existing owned publication path. Preserve checksum verification,
   mutation exclusion, caller-loss custody and recovery/no-replay behavior.
   Qualification must inspect projector structure/role through held readers;
   a `.gguf` extension or `mmproj` substring is not sufficient. A deliberately
   narrower exact-publisher-digest cohort is an alternative to broad format
   admission, but must remain explicitly scoped to that cohort.
3. Carry the primary identity through `CopyPlan`, metadata and reconciliation.
   Both current main and the old development branch choose the largest copied
   model file when calculating primary hashes in `importer/staging.rs`.
   The selected projector is **190,031,616 bytes**, larger than the primary's
   **175,054,528 bytes**. This candidate therefore requires replacing that
   heuristic for acquired sets with the explicitly selected primary. Do not
   change ordinary unrelated import selection as a side effect.
4. Bind the canonical runtime load path and selected artifact to the primary;
   expose the projector as its companion, not an independently interchangeable
   model. Keep the exact two-file manifest and primary/projector roles in
   the publication/reconciliation proof. Populate task metadata from the
   explicit vision-pair operation; imported `llama` architecture or a `vlm`
   modality label alone must not grant image-to-text availability.
5. Preserve the generic acquired-model operation's existing behavior when no
   explicit vision-pair selection is supplied. Coordinate any public DTO or
   generated schema addition with the integration lead before changing it.

Required regression oracles: a projector larger than its primary retains the
primary hashes/load target; reordered manifest members do not change roles;
missing/extra/duplicate/misplaced/wrong-role members refuse before publication;
changed source bytes fail digest verification; output receipts bind both files;
caller loss retains admitted work; reconciliation observes the exact existing
pair without replaying import. Synthetic GGUF test files establish only these
boundaries, never runtime compatibility or inference.

## Real execution sequence

Use the existing acquisition service and llama.cpp `VersionInstaller` configured
with that service. Resolve the exact release asset and HF revision/member
metadata, compare publisher size/SHA-256 to streamed bytes, and retain all
owner-issued receipts. Do not use llama.cpp's `-hf` auto-download, a new ad-hoc
downloader, ambient cache discovery, or hand-edited model-index rows.

After the bounded importer extension is composed, publish the selected pair
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

## Current execution blocker

On 2026-10-09 this cloud executor returned `Tunnel connection failed: 403
Forbidden` for the public Hugging Face model metadata and GitHub release API.
An approved escalated HF metadata read encountered the same proxy response.
Web/GitHub integration reads resolved the official pins, but do not provide
binary assets to the native acquisition owner. No weights/runtime download or
model execution was attempted, and no additional account terms were accepted.

Required resource change: permit the existing native acquisition owner to reach
`huggingface.co` and `api.github.com`, the exact pinned GitHub asset URL and the
asset hosts in their validated download redirects, or supply equivalent
already verified owner-managed resources in the qualification environment.
Do not disable the proxy, weaken TLS or substitute mutable/unverified assets.
The import-role wiring above is a separate engineering prerequisite; restoring
network access does not complete it or grant inference readiness.
