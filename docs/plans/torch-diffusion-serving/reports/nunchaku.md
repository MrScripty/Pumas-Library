# Nunchaku evidence

A2 and A5 passed within their recorded scopes. The image-specific TIPC-04
cancellation/reuse gate also passed. A5 includes public failure mapping,
managed lifecycle cases, a real CUDA allocator OOM, and post-OOM generation;
it does not claim a naturally oversized pipeline request. See the
[A5 GPU recovery report](a5-gpu-oom-recovery.md).

Source includes an offline Nunchaku FP4 rank-128 adapter with sequential CPU
offload, device admission/unload exclusion, cancellation checkpoints, bounded
image requests and PNG output. Pumas uses the existing provider registry,
managed runtime profiles, process-ownership receipts and serving operations.
The public image contract is in `docs/contracts/image-generation.md`.

Verification so far:

- Rust check passes with the Torch provider and image gateway.
- Sidecar host suite: 21 tests passed, including busy/unload exclusion and a
  cancelled HTTP task retaining its lease until the worker stops.
- Gateway host suite: 24 tests passed, including existing text/embedding routing
  and rejection of invalid image inputs. Sandbox socket creation failed before
  handler execution; host tests provide the applicable integration evidence.
- Pipeline acquisition uses Pumas `start_model_download_from_hf`, download ID
  `96e36a17-aa05-4e7f-a508-c09e06417be3`, selected artifact
  `tongyi-mai--z-image-turbo__bundle_3af3fa2de99a`. It is still downloading.
- The library resolves that bundle to
  `diffusion/tongyi-mai/tongyi-mai--z-image-turbo__bundle_3af3fa2de99a`.
  The implementation resolves its library ID from the exact upstream repository,
  rejecting absent or ambiguous matches rather than inferring a storage path.
- Current release gateway used for library acquisition is
  `http://127.0.0.1:18767/v1`. It now runs the M2 release identified in the ledger.

Required next evidence includes corrected runtime native/GPU qualification,
complete library assets, a real release gateway PNG, cancellation followed by
another generation, busy/unloaded/OOM behavior, and process-exit invalidation.

Further evidence:

- Sidecar host suite now passes 22 tests, including repeated cancellation during
  a model load retaining its device lock until the loader returns.
- A real Axum/client/backend TCP fixture confirms client disconnect closes the
  gateway backend request. This proves transport propagation, not GPU cancellation.
- The local FP4 checkpoint header identifies
  `NunchakuZImageTransformer2DModel`, `fp4_e2m1_all`, rank 128. The adapter checks
  these fields before allocating model weights.
- Real library descriptor resolution currently chooses the INT4 rank-256 file as
  its generic entry point; the concrete adapter chooses the qualified FP4 sibling
  inside that same library-owned model directory. No model files are copied or
  acquired by the runtime adapter.

## Native kernel compatibility qualification

A real FP4 transformer forward in runtime 0.1.1 failed with
`TypeError: argument of type 'int' is not iterable`. Nunchaku 1.2 forwards patch
sizes positionally; Diffusers 0.37 inserted ControlNet parameters at those
positions. A weight-free reproduction observed `controlnet_block_samples=1` and
`return_dict=2`, identifying the cause without changing checkpoint or GPU state.
This direct binding evidence justified skipping a broad hypothesis search in the
diagnosing-bugs workflow. Native regression tests use the real dependency classes.

`nunchaku_compat.py` retains Nunchaku's packed rotary hooks and supplies Diffusers
options by name. Both boundary tests pass, including hook cleanup after an
exception. Repeating the original real-checkpoint/GPU probe produced finite
`[16, 1, 64, 64]` output, total load/forward time 16.17 seconds, peak Torch allocated
memory 4,412,111,872 bytes. Logs: `native-nunchaku-kernels.log` (failure),
`nunchaku-binding-red.log`, `nunchaku-native-boundary-{red,green}.log`, and
`native-nunchaku-kernels-fixed.log`. This random-latent probe is native kernel
evidence, not a generated PNG or full pipeline acceptance. The corrected payload
was installed and qualified as 0.1.2; installed environments were not patched in place.

Model admission now uses the library record's exact qualified upstream repository
identity (including the upstream organization rename), rather than one
machine-specific library ID. Library IDs remain unchanged and are used for
serving/discovery. The concrete FP4 file and header checks still apply. This lets
fresh acquisitions with the current library naming scheme use the same adapter.

The installed 0.1.2 Python environment passes four native-dependency tests,
including OOM/backend-error mapping, lease release followed by an explicit new
request, and deadline cancellation waiting for the worker to stop. These use
controlled workers, not GPU OOM or real image inference. Log:
`native-current-tests.log`. Actual GPU cancellation remains required.

## Real release and Tuldok image passed

Release SHA-256 `764198b4fa47f7ab117d1eb92f5f38ff8a8ed1ca02b476744584838c80b7bef6`
served the complete Nunchaku pipeline using installed runtime 0.1.2 and managed
profile `torch-image-acceptance`. The gateway is `http://127.0.0.1:18767/v1`.
The real run exposed and fixed duplicate profile launch, startup health waiting,
and generic descriptor shard paths being passed where the pipeline directory was
needed. Component resolution now uses the indexed, validated library package.

Tuldok discovered the image model, submitted a red-teapot/yellow-lemon/blue-table
prompt at 512×512 with seed 42, displayed the result, downloaded matching PNG
bytes and preserved them through collection import. Browser elapsed time was
11.047 seconds; runtime generation reported 10.972 seconds, eight steps,
guidance zero and sequential CPU offload. Visual inspection confirmed the prompt.
PNG SHA-256: `497f4ea4645bb2d529b3db1b6112f3ad99145d03c8e7cdfde8ede5526cbe8e02`.

Evidence under `launcher-data/cache/torch-qualification/`:
`tuldok-nunchaku-real/{display.png,saved.png,result.json}`,
`nunchaku-real-load.json`, `nunchaku-browser-real.log`, and
`nunchaku-browser-telemetry.json`. One-second Pumas samples observed GPU usage up
to 988,807,168 bytes and RAM usage up to 20.57%; sampling can miss brief peaks.
The pipeline used real local weights and no mock backend. A2 and A3 are passed.

## Exact-candidate cancellation and reuse

TIPC-04 passed separately against production-installed exact
`torch-runtime-0.1.6` archive SHA-256
`74f9b593dff1e447a73571dc465efd520238ba0c56d2730393a17eee2b97426c`
and Pumas `ab3a95a9369a5471127003fcd8e6fff529596cb5`. The runtime loaded the real
FP4 rank-128 pipeline on the RTX 5090 Laptop GPU in 15.255 seconds.

A live 1280×720 generation was disconnected after admission. Runtime evidence
records cancellation requested at `23:30:56.981`, with the worker/device cleanup
completed at `23:30:57.584`. A following request was admitted only after that
completion and returned HTTP 200 with one 265,041-byte PNG in 7.276 seconds;
decoded PNG SHA-256 is
`4fc58b0a9d21b33380006600af8cc26afc8f33577d7cf6b3fc6180937b96d013`.
Evidence is the isolated profile `runtime.log` plus
`launcher-data/cache/torch-qualification/tipc-0.1.6-tipc-04-reuse-response.json`.

The model was unloaded, the isolated profile and gateway stopped, and GPU
occupancy returned to the pre-run baseline. The main launcher retained runtimes
`0.1.1`–`0.1.4`, selected `0.1.4`, and no default. No candidate was published or
installed into the main version store.
