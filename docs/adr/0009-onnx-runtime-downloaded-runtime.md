# ADR 0009: ONNX Runtime is a downloaded runtime hosted per profile

- Status: Accepted
- Date: 2026-11-12
- Supersedes: [ADR 0001](0001-onnx-runtime-provider-model.md) on two points.
  ADR 0001 executed ONNX in the Pumas Rust process and kept ONNX Runtime
  packaging in the Rust native dependency closure; this ADR replaces both.
  Its provider model otherwise stands.

## Context

Under ADR 0001, desktop packages and headless inference archives bundled one
pinned ONNX Runtime library beside `pumas-rpc`, and the in-process provider
loaded it. llama.cpp and Torch, by contrast, are external runtimes that Pumas
downloads and runs one process per runtime profile.

The owner decided:

- ONNX Runtime is not shipped inside the app or the headless archives. Pumas
  installs runtimes.
- Pumas must not need a restart. llama.cpp, Torch and ONNX Runtime may run
  several instances when different versions are wanted, for example for models
  with different runtime requirements. Restarting one runtime instance is fine.
- Pumas must support GPUs.

ONNX Runtime keeps the first native library a process loads (`ort` stores it
in a process-wide `OnceLock`, and Pumas already refused a different library
after adoption). One process therefore cannot switch versions or run two at
once. In-process execution with "restart Pumas to switch" is rejected.

## Decision

**Version management.** ONNX Runtime (app id `onnx-runtime`) is a
version-managed app. Its releases are the official `microsoft/onnxruntime`
GitHub releases.

- No release is pinned. The listing keeps stable releases whose version
  exports the C API the `ort` binding requests: 1.x with x at least the
  binding's C API version, which is 24 today.
- Each build Microsoft publishes for the host is listed as a variant:
  - `+cpu` on every supported platform. The macOS build also carries the
    CoreML execution provider.
  - `+cuda12` and `+cuda13` on Linux x64 and Windows x64. Upstream names the
    CUDA 12 build `gpu` before 1.27 and `gpu_cuda12` after.
- Installation reuses the llama.cpp native-archive path: shared acquisition
  (resumable, publisher SHA-256), custody, receipts and restart reconciliation.
  ONNX Runtime has its own acquisition consumer (`runtime.onnx-runtime`),
  versions directory (`onnx-runtime-versions/`) and active-version marker.

**One host process per profile.** Each managed ONNX Runtime profile runs its
own ONNX host process, like the llama.cpp server and the Torch sidecar.

- The host loads the library of the installed version the profile launches:
  its pinned `runtime_version` (#145), else the active version.
- Two profiles pinned to different versions serve at the same time.
- Changing a profile's version restarts only that host, never Pumas.

**Host form.** The host is the `pumas-rpc` executable started with
`--onnx-host`. That is the existing `ort` code (`RealOnnxEmbeddingBackend`,
`OnnxSessionManager`) running in another process. We chose this over a
separate binary because a second executable would need its own staging,
packaging, notarization, SBOM and smoke coverage on every platform, for no
behavioural gain. The RPC server registers its own executable as the host at
startup. An embedding application that registers none cannot launch ONNX
profiles, and the launch is refused with that reason.

**Lifecycle.** Hosts are launched and owned by the existing runtime-profile
process owner, with no parallel lifecycle:

- PID and identity records under `launcher-data/runtime-profiles/onnx-runtime/<profile>/`;
- the `<pid file>.version` sidecar;
- listener custody;
- the stale-instance scans.

The host exits when its launching backend exits. Every request except
`/health` carries a per-backend token that reaches the host only through its
environment.

**Execution provider.** The host derives its execution provider from the
installed build and the profile's device mode, and registers it with
`error_on_failure`:

| Device mode | Execution provider |
| --- | --- |
| CPU | CPU |
| GPU | CUDA for `+cuda*` builds, CoreML on macOS. Refused with the reason on any other build. |
| Auto | The build's GPU provider if it has one, otherwise CPU. |

**Refusals.** The host initializes its library and execution provider before it
listens, then reports readiness on `/health`. When a host cannot initialize,
Pumas stops it and refuses the launch with the host's reason. For CUDA the
reason names the NVIDIA driver plus the CUDA runtime and cuDNN libraries.

**Serving.** Pumas reaches hosts through `RemoteOnnxEmbeddingBackend` behind
the existing session manager, so the `/v1` gateway and model operations are
unchanged. Serving launches the profile's host when it is not running.

**Developer override.** `ORT_DYLIB_PATH` remains an explicit developer
override inside a host. Nothing falls back to a library beside the
executable.

**Packaging.** Desktop packages and headless archives bundle no runtime. The
headless archive manifest moves to schema 2, which has no runtime record. The
archive verification installs ONNX Runtime through the extracted `pumas-rpc`.

## Consequences

- A fresh install cannot run ONNX embeddings until the user installs an ONNX
  Runtime version. Every refusal says so.
- GPU execution depends on the host's NVIDIA driver and CUDA/cuDNN libraries.
  Pumas does not install those; the refusal names what is missing.
- DirectML is not offered. Microsoft publishes it only as a NuGet package, last
  updated at 1.24.x. Offering it would need a second download source and
  SHA-512 evidence in the acquisition manifest, which is an owner decision.
- One model id is loaded by at most one host at a time. Loading it on a second
  profile is refused with the host that holds it.

## Revisit When

- Pumas should install CUDA/cuDNN libraries for GPU builds;
- a DirectML, ROCm or other execution provider is required; or
- ONNX Runtime changes its archive layout or its C API compatibility policy.
