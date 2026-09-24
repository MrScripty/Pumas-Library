# FLUX.2 (real Tuldok acceptance passed)

The preflight history below records the earlier qualification stages. Final
acceptance evidence is recorded at the end of this report.

M4 source preparation was admitted during the slow pipeline acquisition;
Nunchaku remains the first real-image gate. Preflight established the following:

- The local checkpoint is the 9B KV FP8 variant, not a 4B model. Its safetensors
  metadata declares per-layer `float8_e4m3fn`, with separate weight/input scales.
  A blind BF16 cast would lose those scales and is not an acceptable conversion.
- The [BFL model card](https://huggingface.co/black-forest-labs/FLUX.2-klein-9b-kv-fp8/raw/main/README.md)
  supports text-to-image and describes four-step distillation and an 8B Qwen3
  embedder; KV caching targets reference-image editing, outside this request.
- The [BFL reference loader](https://github.com/black-forest-labs/flux2/blob/main/src/flux2/util.py)
  selects Qwen3-8B and the FLUX.2 VAE for Klein 9B/KV.
- Public upstream tree metadata reports Qwen/Qwen3-8B at 16,397,461,266 bytes.
  The complete FLUX.2-dev repository is 177.64 GB, so acquiring that whole bundle
  would be inappropriate for a single VAE dependency.
- [Comfy-Org's separate VAE](https://huggingface.co/Comfy-Org/flux2-dev/tree/main/split_files/vae)
  is 336,213,556 bytes. Its 28,056-byte safetensors header contains 251 tensors,
  Diffusers down/up-block keys, quantization convolutions and batch-normalization
  buffers. Header inspection downloaded no weight tensors. This layout can avoid
  a second VAE conversion implementation, subject to actual strict load checks.

No matching asset acquisition or GPU/PNG acceptance has been claimed. Future
acquisition must use Pumas library authority; runtime execution stays offline.
Evidence JSON lives under `launcher-data/cache/torch-qualification/` with
`*-preflight-tree.json`, `flux2-vae-header-preflight.json`, and the local FP8 header
summary. Official gated tree metadata masks weight hashes; matching sizes alone
are not a weight-identity proof.

The local checkpoint's 9,078,581,248 parameters strictly match the installed
Diffusers Klein 9B architecture after its existing converter. Actual CPU decoding
also passed strict load and finite-value checks in 20.10 seconds
(`flux-real-cpu-weights.log`). No FLUX GPU execution occurred.

The adapter follows the [checkpoint format's scale semantics](https://github.com/Comfy-Org/ComfyUI/blob/master/QUANTIZATION.md):
decode FP8 weights with their scalar weight scales into BF16, retain unquantized
weights, validate activation scales, and execute BF16 activations. It explicitly
reports `scaled_fp8_to_bf16_sequential_cpu_offload`; this does not claim FP8 kernel
execution or FP8 resident-memory size. Pumas telemetry admission requires 42 GiB
available system RAM before this model loads. Scale fixtures pass, including
missing, zero, non-finite and nonscalar rejection.

The adapter reuses the existing Diffusers converter and the common diffusion
cancellation/cleanup method. Qwen3-8B and the separate VAE remain library inputs.
Four steps and guidance 1 use the BFL reference defaults; the dynamic exponential
scheduler matches the [published Klein scheduler](https://huggingface.co/black-forest-labs/FLUX.2-klein-4B/raw/main/scheduler/scheduler_config.json)
and installed Klein pipeline's empirical shift. Real output must still qualify
that configuration for this checkpoint. Rust load transport adds only an optional
VAE component path; gateway/client image transport and updater are unchanged.

## FP8 text encoder support

Qwen3-8B acquisition is complete. The user requested reducing encoder storage
and making its role visible. The initial local FP8 artifact contains
9,436,626,944 weight bytes (8.79 GiB), versus 16,381,516,776 BF16 bytes
(15.26 GiB). Native prompt features from the converted encoder were finite.
At this encoder-preparation checkpoint, full FLUX/Tuldok acceptance remained
pending. Real image display/save/import passed later in this report.

Pumas's existing conversion manager now has a general `fp8` backend and
`safetensors_to_fp8` direction, available in the existing conversion dialog.
It writes Transformers-compatible E4M3FN weights and per-128x128-block float32
scales, preserves tokenizer/config assets, and keeps embedding/output weights
in BF16. Its CPU-only conversion environment uses the existing setup owner;
it does not install CUDA or introduce a Torch serving updater. Cancellation,
non-replacing publication, provenance and library indexing reuse existing owners.
The original BF16 source is preserved. This is limited to supported Transformers
causal-language-model packages with compatible linear dimensions, not arbitrary
Safetensors checkpoints.

Both current library entries have explicit FLUX.2 Klein 9B text-encoder names.
No merging, component hiding, or new model-link UI has been implemented.

### Actual Pumas conversion and native FLUX result

The general CPU backend completed through the real desktop dialog in 43.132
seconds (conversion `conv-1`). Its final output has 9,437,473,248 weight bytes
(8.79 GiB), including FP32 block scales. This supersedes the initial prototype
size above; the prototype import and staging weights were removed. Evidence:
`fp8-managed-conversion.json`, `fp8-managed-artifact.json`,
`fp8-conversion-complete-desktop.png`, `fp8-conversion-desktop.json`.

Torch runtime 0.1.4 loaded the Pumas-converted package using Transformers FP8
linear layers. A real FLUX generation with the resolved VAE file completed in
14.174 seconds (14.149 runtime-reported), four steps, guidance 1, seed 42, 512x512.
The PNG visibly matches the red teapot/yellow lemon prompt. This is a native
runtime result, not final gateway/Tuldok acceptance. Evidence:
`flux-fp8-native-load.json`, `flux-fp8-native-image.json`, `flux-fp8-native.png`.
The diffusion transformer still uses scaled FP8-to-BF16 execution; the text
encoder uses FP8 weights/activations. Sequential CPU offload is active for both.

## Final release and Tuldok acceptance — passed

Pumas release SHA-256
`b11742c95e7dc68d82abd790618a6cba9e012dbc95e7c8ad8cb60f9165b45b74`,
Torch runtime 0.1.4, and gateway `http://127.0.0.1:38035/v1` served both
Nunchaku and FLUX.2. The actual Tuldok browser discovered FLUX by capability,
submitted the teapot/lemon prompt, displayed a 512x512 image and saved/imported
matching PNG bytes. A4 is passed.

- End-to-end generation: 8.843 seconds; runtime: 8.697 seconds.
- Four steps, guidance 1, seed 42. Visually inspected prompt-relevant PNG.
- PNG SHA-256: `3d17e882d0b4b7b4d67e37e0df8a2cf8833756563971cb965397344b5be9a190`.
- Eleven Pumas telemetry samples: highest observed GPU usage 1,996,488,704 bytes,
  highest observed host RAM usage 46.7773%. Sampling can miss brief peaks.
- Evidence under `launcher-data/cache/torch-qualification/`:
  `tuldok-flux2-fp8-real/{saved.png,display.png,result.json}`,
  `flux2-fp8-browser.log`, `flux2-fp8-browser-telemetry.json`,
  `final-fp8-flux-load.json`, and `final-fp8-nunchaku-load.json`.

No bitwise-determinism or arbitrary-checkpoint compatibility claim is made.
