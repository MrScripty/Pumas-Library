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

**CUDA libraries.** A `+cuda12` or `+cuda13` install brings the NVIDIA CUDA
runtime and cuDNN libraries its CUDA execution provider loads (amended
2026-11-12; see [Amendment](#amendment-2026-11-12-cuda-libraries)). They come
from NVIDIA's wheels on PyPI, through the same acquisition as the archive, and
only their shared libraries are kept, in `cuda/lib` of the installed version.
The ONNX host's library search path points there. CPU builds are unchanged.

**Refusals.** The host initializes its library and execution provider before it
listens, then reports readiness on `/health`. When a host cannot initialize,
Pumas stops it and refuses the launch with the host's reason. For CUDA the
host asks the NVIDIA driver which CUDA version it supports, and the reason
names the missing piece: the driver is missing, the driver is too old for the
build's CUDA major version, or the CUDA runtime or cuDNN libraries Pumas
installed with the version did not load (reinstall the version), or the
version predates them (reinstall it to get them).

**NVIDIA driver.** The driver belongs to the host system and is never managed
like a runtime. Pumas detects a missing or too-old driver and points at the
official NVIDIA download, but never installs, tracks or removes it.

**Telemetry.** Pumas disables third-party telemetry. Every ONNX host runs with
`ORT_DISABLE_TELEMETRY=1`, which ONNX Runtime reads as its environment
initializes: on Linux and macOS it stops the 1DS uploader before it creates
its device id and event database under the user cache
(`Microsoft/DeveloperTools/.onnxruntime`). The host also commits ONNX
Runtime's environment with telemetry events disabled, the switch Windows
builds honor.

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
- GPU execution depends on the host's NVIDIA driver, which stays the user's
  to install. Pumas installs the CUDA runtime and cuDNN libraries with each
  CUDA version. With the releases current on 2026-11-12 that adds 1.2 to
  1.7 GB of wheels to a CUDA install's download, and more once the libraries
  are extracted.
- A CUDA build whose published package metadata is missing or names packages
  NVIDIA never published as wheels cannot be installed; the refusal says so.
- DirectML is not offered. Microsoft publishes the DirectML build only as a
  NuGet package, and it stops at 1.24.4, so it would pin Pumas to an old
  release and need a second download source with SHA-512 evidence.
- One model id is loaded by at most one host at a time. Loading it on a second
  profile is refused with the host that holds it.

## Revisit When

- Microsoft publishes CUDA package metadata somewhere else, stops publishing
  it, or ships the CUDA libraries in its archives;
- NVIDIA renames or repackages its CUDA library wheels in a way the
  requirements of a build no longer name;
- Microsoft publishes DirectML builds outside NuGet or past 1.24.4, or ROCm or
  another execution provider is required; or
- ONNX Runtime changes its archive layout or its C API compatibility policy.

## Amendment 2026-11-12: CUDA libraries

The owner decided that Pumas installs the CUDA runtime and cuDNN that ONNX
Runtime GPU builds need, from NVIDIA's pip wheels through the shared
acquisition owner, stored with that ONNX Runtime version's install, resolved to
current compatible releases without pins, with the host's library search path
pointing at them. The NVIDIA driver stays the host system's.

**What a build needs comes from Microsoft's metadata.** The GitHub release
archives do not say which CUDA libraries a build needs. The `onnxruntime-gpu`
Python package of the same release does: its `cuda` and `cudnn` extras list the
NVIDIA packages and version ranges (for 1.31.1, `nvidia-cuda-runtime~=13.0`,
`nvidia-cuda-nvrtc~=13.0`, `nvidia-cufft~=12.0`, `nvidia-curand~=10.0` and
`nvidia-cudnn-cu13~=9.0`). Microsoft publishes one CUDA major version of each
release on PyPI and the other on its own feeds (`onnxruntime-cuda-12` and
`onnxruntime-cuda-13` on `aiinfra.pkgs.visualstudio.com`), so Pumas reads
PyPI first, then the feed for the build's CUDA major version, through the JSON
simple index API (PEP 691) and the core metadata served beside each wheel
(PEP 658), checked against its published digest. Metadata counts only when
its CUDA runtime requirement names the build's CUDA major version
(`nvidia-cuda-runtime-cu12`, or `nvidia-cuda-runtime` with a range starting
at 13). Pumas downloads no `onnxruntime-gpu` wheel.

As published on 2026-11-12, that metadata covers:

| Build | Metadata | Result |
| --- | --- | --- |
| CUDA 12, 1.24 to 1.26 | PyPI | Installs |
| CUDA 12, 1.27 to 1.29 | `onnxruntime-cuda-12` feed | Installs |
| CUDA 12, 1.30 and later | none published | Refused with the reason |
| CUDA 13, 1.28 and later | PyPI | Installs |
| CUDA 13, 1.27 and earlier | PyPI or `onnxruntime-cuda-13` feed | Refused: it names `nvidia-*-cu13` runtime packages that NVIDIA published only as placeholders without wheels |

Pumas does not substitute another release's metadata or a list of its own for
a refused build, because that would be a guess about what the build links.

**Listed builds carry their install state.** The ONNX Runtime version listing
runs the same resolution for every CUDA build it lists and gives each one an
`installAvailability` (an additive field of the listed release): `checking`
while its lookup runs, `available`, `unavailable` with `unavailableReason`, or
`unknown` when the indexes could not be read. An unknown build stays
installable, because the install resolves again and decides. The desktop app
shows "Checking…" or the reason in place of Install, and nothing else: it
renders what the listing reports.

The reason, and the refusal of an install attempted anyway, also names what
installs instead, derived from the same lookups over the listed builds: the
newest build of the same CUDA major that installs, and the other major's
installing builds, in the form "+cuda13 installs from <oldest> (newest <tag>)"
when every listed build in that range installs.

A listing waits up to ten seconds for running lookups. When any build is still
`checking`, the listing says so (`availability_pending`), and the desktop app
asks again at once; the backend paces those requests, because each one waits
on the same running lookups rather than starting new ones.

The answers are kept per build. They last until the indexes change or a
refresh is asked for, not for a fixed time: a forced version refresh, or an
install (which always reads the indexes as they are), starts a new
generation, and the next lookup revalidates each index page it reads with the
index (`If-None-Match`, `If-Modified-Since`; PyPI answers "not modified" when
nothing changed). Core metadata of a file never changes and is kept for the
process.

**Current compatible releases.** The NVIDIA packages and the NVIDIA packages
they require in turn (cuDNN requires cuBLAS, cuBLAS requires NVRTC, cuFFT
requires nvJitLink) are resolved on PyPI: for each, the newest final,
non-yanked release that satisfies every range and has a wheel for the host
(win_amd64, or a manylinux x86_64 wheel whose glibc version the host has).
When what that release requires cannot be satisfied, older releases of it are
tried. A requirement on a project outside NVIDIA's
`nvidia-*` packages is a Python dependency and is not followed; a marker on a
Python variable cannot exclude a library. The resolved CUDA runtime must have
the build's CUDA major version. Each wheel needs the SHA-256 its index
publishes; one without it is refused.

**Download and install.** The release archive and every wheel form one
acquisition set with one demand, so the wheels download resumably and are
verified against their SHA-256 before anything is extracted, exactly like the
archive. Pumas reads the wheels as zip files and needs no pip or Python. Only
files under `nvidia/` that the platform loader loads by name (`lib*.so*` on
Linux, `*.dll` on Windows) are kept, flattened into `cuda/lib`, with each
package's license under `cuda/licenses/`. `cuda/libraries.json` records the
metadata source, each package, wheel digest and library digest, and the
installation receipt's output tree digest covers all of it. Removing the
version removes them.

**Selection is kept for the attempt.** The resolved set is stored in the
install attempt's workspace. A failed or interrupted install that is retried
reuses that set, so completed wheels are not downloaded again and partial ones
resume, even after NVIDIA publishes newer releases. Cancelling or finishing the
attempt removes it with the workspace; the next install resolves again.

A kept set is replaced only when one of its wheels can no longer be
downloaded: its project or file answers not found (404 or 410) on the index,
the index no longer lists it, or its file URL answers 404 or 410. The retry
then resolves against the current indexes, withdraws the earlier attempt's
unfinished download through the acquisition owner
(`withdraw_unfinished_transfer`), and carries the finished files the new set
still names (same name and SHA-256, the release archive included) into the new
attempt, where acquisition verifies them again before using them. The install
log says which wheel vanished and what was selected instead. A server error,
a network failure or a different digest for the same file name keeps the set:
a retry or the verified download decides those.

**Search path.** When the installed version has `cuda/lib` and its record, the
ONNX host starts with `LD_LIBRARY_PATH` set to that directory on Linux (the
inherited `LD_LIBRARY_PATH` is still never passed to owned children), and with
that directory first on `PATH` on Windows. The loader and ONNX Runtime's CUDA
provider then find the libraries by name.

Not verified by this change: a CUDA session on real NVIDIA hardware with the
installed libraries, on Linux or Windows.
