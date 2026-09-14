# NVFP4 conversion extension

The existing conversion dialog offers Safetensors (NVFP4), using the existing
`SafetensorsToNvfp4` direction, backend setup owner, cancellation, temporary
output publication and indexing. No model-specific converter was introduced.

The former backend changed an FP8 recipe and called ordinary `save_pretrained`,
which did not establish packed FP4 export. It now uses Model Optimizer 0.40.0's
NVFP4 recipe and `export_hf_checkpoint`. Output validation requires NVFP4 config,
packed byte weights, and block/global scales across the exported shards.

## Focused evidence

- Real CUDA conversion of a small locally serialized Qwen3 model: seven packed
  weight tensors; each contains two FP4 values per byte.
- Reopened Safetensors, decoded using ModelOpt's reference NVFP4 decoder, loaded
  all model parameters strictly, and ran a finite GPU forward pass. This proves
  storage/reload correctness, not native FP4 matrix multiplication or 8B quality.
- Pumas telemetry before the probe: GPU total 25,651,314,688 bytes, used
  2,044,723,200 bytes; RAM 47.15% used of 66,815,029,248 bytes.
- Conversion dialog tests: 9 passed, including FP8 and NVFP4 routing. Frontend
  type check, frontend build and Electron build passed.

Ignored local artifacts under `launcher-data/cache/torch-qualification/`:
`nvfp4-format-roundtrip.log`, `nvfp4-roundtrip-result.json`,
`nvfp4-telemetry.json`, `nvfp4-ui-tests.log`, `nvfp4-ui-types.log`.

## Serving boundary

The qualified Torch runtime uses Transformers 4.57.6, which does not have a
ModelOpt NVFP4 quantizer. The working FLUX pipeline remains on its FP8 text
encoder. The dialog explains this limitation. NVFP4 conversion does not claim
NVFP4 encoder serving support. Existing Tuldok/disabled-artifact acceptance is
retained; no repeated broad diffusion or disabled-build matrix for this
conversion-only change.

Release `a64bfc5d0574b6d8fdb3a6466c60d5a8857e13996e1a18f00bde698494a6852e`
built successfully and is running in the real desktop. The NVFP4 option and
retained managed setup state were visually checked in
`nvfp4-conversion-desktop.png`. Both existing image models were restored
successfully through the release gateway at `http://127.0.0.1:43571/v1`;
`nvfp4-release-{nunchaku,flux}-load.json` records those responses.
The initial managed installation reached the shared 15-minute command limit
while downloading CUDA wheels. Pip did not retain these large wheels in its
HTTP cache. The NVFP4 recipe now allows 45 minutes for dependency installation;
other recipes keep their existing limit, and shared cancellation/child cleanup
remain unchanged. Completed temporary NVIDIA wheels were retained only after
matching official PyPI SHA-256 hashes, to avoid repeating those downloads.
The final managed installation completed successfully (operation
`39cc9ed5-71c9-458c-96b6-87e59dcb9a03`). The live release reports NVFP4 and FP8
backends ready. The deployed converter and its freshly installed environment
then converted the small Qwen3 package, validated seven packed weight tensors,
and passed strict saved-weight reload plus a finite GPU forward using ModelOpt
reference dequantization. Evidence: `nvfp4-managed-setup-result.json`,
`nvfp4-final-backend-status.json`, `nvfp4-installed-roundtrip.log`.

The actual dialog was visually checked on the preceding build with identical
frontend assets. Final desktop CDP capture was unavailable because its debugging
port was occupied during restart; the running application and live RPC remain
available. No extra restart was performed solely for another screenshot.
The 8B BF16 and FP8 library models remain intact; no full 8B NVFP4 conversion or
native NVFP4 encoder inference is claimed. T16 records the observed pre-existing
shutdown issue and the operational cleanup of the previous installer.
