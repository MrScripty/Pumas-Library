# Bounded public Nomic fixture investigation

Observed 2026-10-14. No model bytes were downloaded or executed in this task.

## Authoritative identity

The existing Rust optional real-fixture tests name `nomic-embed-text-v1.5` and
default to `onnx/model_fp16.onnx`. The packaged verifier uses the same layout.
The publisher's [official model card](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5)
declares Apache-2.0 and documents task-prefixed inputs and Matryoshka dimensions.
Use the publisher's own repository; no third-party conversion, remote Python
model code, private model directory or unrelated model family is selected here.

Immutable fixture revision:
[`ac6fcd72429d86ff25c17895e47a9bfcfc50c1b2`](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/tree/ac6fcd72429d86ff25c17895e47a9bfcfc50c1b2).
The publisher's [commit/pointer](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/commit/ac6fcd72429d86ff25c17895e47a9bfcfc50c1b2)
records:

| Required file | Immutable upstream identity / observation |
| --- | --- |
| `onnx/model_fp16.onnx` | 273,859,028 bytes; SHA-256 `cf5b5a86edb00f895561803cfc04729090a958340b8ca2ad76c143f565f6bb04`; public Git-LFS pointer, not a locally computed digest |
| `tokenizer.json` | [Same revision](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/blob/ac6fcd72429d86ff25c17895e47a9bfcfc50c1b2/tokenizer.json), approximately 711 kB; local SHA-256 unavailable |
| `config.json`, `tokenizer_config.json`, `special_tokens_map.json` | Same immutable repository revision; root listing reports approximately 2.06 kB, 1.19 kB and 695 bytes. Exact transferred bytes/digests are unavailable here. The config page also returned browser HTTP 429. |

The entire repository is approximately 2.22 GB and includes alternative weights.
Only the selected approximately 275 MB file set is relevant. No whole-repository
clone, Safetensors, quantized alternatives, vision/ASR weights, model execution
scripts or training dependencies are necessary for this fixture.

## Feasibility and stopping condition

The first executor request was the public HF model metadata endpoint, not a
weight request. Its HTTP proxy rejected the tunnel with **403** before any bytes
transferred. No alternate host, proxy bypass, private credentials, Git mirror,
or download retry was attempted. Public browser pages supplied the observations
above; they do not transfer a usable model into the executor. The raw API was
also unavailable through the browser. Thus bounded trained-model execution is
**unavailable in this executor**, rather than failed inference.

On a native host with authorized HF access, fetch only those five paths from the
exact revision. Bound the weight to 273,859,028 bytes and the four small files to
a reviewed small-file budget (2 MiB each is sufficient for the observed sizes).
Verify the weight SHA-256 against the upstream pointer and record SHA-256/size
for every tokenizer/config byte before using the directory. Retain the immutable
source reference and publisher license declaration with the evidence. Do not
execute repository Python or substitute a different fixture after a refusal.

Run the existing positional verifier against each exact extracted native RPC:

```bash
python scripts/release/verify-packaged-onnx.py EXTRACTED_RPC NOMIC_DIRECTORY
```

It records the copied model/config/tokenizer hashes, import/load, 256 finite
embedding values, selected packaged native runtime, unload and clean disposal.
Its model filename is copied to `model.onnx`; compare that receipt hash with the
upstream FP16 pointer. A successful receipt is a lifecycle result for those exact
bytes, not a quality benchmark or provenance authority. Windows/macOS need their
own actual result; a Linux receipt cannot substitute. The model is test input,
not a redistributed release asset. Maintainer publication/license acceptance
remains separate; no fixture license text or model data was invented or bundled.

The investigation stops at the verified public identity and exact transfer
blocker. Future evidence must come from an authorized transfer and actual native
execution, not more mocks or untrained graph runs.
