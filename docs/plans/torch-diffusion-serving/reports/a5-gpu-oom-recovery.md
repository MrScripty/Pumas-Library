# A5 real CUDA OOM recovery

**Status:** Passed on 2026-09-22 (America/Vancouver; 2026-09-23 UTC).

## Setup

The test used the isolated `torch-runtime-0.1.6` installation at
`launcher-data/cache/torch-qualification/tipc-0.1.6-install`, whose candidate
archive SHA-256 is
`74f9b593dff1e447a73571dc465efd520238ba0c56d2730393a17eee2b97426c`. Its
managed `torch-image-acceptance` profile served the real
`diffusion/nunchaku-ai/nunchaku-z-image-turbo` model on the RTX 5090 Laptop GPU.
The sidecar ran the current branch's `torch-server` source over the candidate's
qualified dependencies and assets; the candidate source files were restored and
SHA-256 checked after the run.

## OOM and recovery

A one-shot test hook ran inside the loaded adapter's `generate` call. It queried
`torch.cuda.mem_get_info()` and requested the currently free bytes plus 256 MiB
with a CUDA `uint8` tensor. The actual CUDA allocator raised
`torch.cuda.OutOfMemoryError`. This avoided oversized image dimensions and did
not allocate host memory.

The public `POST /v1/images/generations` request returned HTTP 507 with code
`out_of_memory` and the sanitized message `Insufficient GPU memory`. The same
profile remained healthy. A separate subsequent request through the gateway
returned a decodable 512×512 PNG: 302,715 bytes, SHA-256
`cddd2386cb8ca5105325aca2d202db68885dd71402dca04f55949b87f379120d`. It used
seed 42, 8 steps, sequential CPU offload, and completed in 10.309 seconds.

The model unloaded and the profile stopped successfully. GPU memory was 657 MiB
before the run and 662 MiB after cleanup; the only listed CUDA process was the
existing desktop process. The main launcher remained unchanged with runtime
0.1.1–0.1.4 installed, 0.1.4 active, and no default runtime. No runtime was
published or tagged.

This verifies a real CUDA allocator OOM followed by successful real inference
recovery. It does not claim that a naturally oversized diffusion request caused
the OOM. Existing [GPU cancellation and reuse evidence](nunchaku.md) covers the
other required real-GPU lifecycle case.
