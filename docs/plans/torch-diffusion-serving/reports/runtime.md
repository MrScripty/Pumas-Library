# M1 runtime implementation and qualification

Status: native/GPU/sidecar qualification passed for the CUDA 13.0 candidate. The original Pumas-bundle discovery assumption has been superseded by the official PyTorch upstream contract; **packaged-app discovery and shared UI installation acceptance remain pending**.

## Implemented source

- `VersionManager` still owns discovery, installed state, active/default selection,
  progress and cancellation. Torch discovery now queries `pytorch/pytorch` and
  exposes only the qualified upstream `v2.9.1` release. The upstream tag maps to
  the independent recipe `torch-upstream-2.9.1-r1`.
- The app embeds the sidecar and dependency lock. `VersionInstaller` installs the
  direct SHA-256-pinned Torch and torchvision wheels from the official PyTorch
  wheel host, installs the remaining hash-locked dependencies, checks
  recipe/platform/Python/protocol identity, and runs GPU/import/sidecar validation
  before atomic non-replacing publication and shared metadata registration.
- The existing platform directory publication primitive is exposed to the app
  manager; its implementation and model-library ownership are unchanged.
- Failed staging is removed. Previously installed runtime files and metadata are
  preserved. Process cancellation kills the installer process group and waits for
  termination. Download cancellation also covers a stalled HTTP response.
- Installation serialization now remains held for the installation task's lifetime;
  failed release resolution cannot leave a phantom `installing` tag.
- Running Torch environments cannot be switched away from or removed. Torch uses
  `.active-version-torch` so its selection cannot overwrite llama.cpp's marker.
- Legacy source installations missing runtime files lose only their stale installed
  metadata; files remain available for user recovery.

## Qualified recipe

Upstream tag `v2.9.1`, recipe `torch-upstream-2.9.1-r1`, sidecar protocol 3,
Linux x86_64, CPython 3.12, NVIDIA sm_120. Dependencies include Torch 2.9.1+cu130, torchvision 0.24.1+cu130,
Nunchaku 1.2.0+torch2.9, Diffusers 0.37.0, Transformers 4.57.6,
Accelerate 1.12.0 and PEFT 0.18.1. The complete transitive pins and artifact hashes
are in `torch-server/runtime/requirements.lock`. Native/GPU/sidecar qualification passed on this host; model generation
qualification still belongs to M2/M4.

Primary inputs:

- [Nunchaku installation](https://nunchaku.tech/docs/nunchaku/installation/installation.html):
  Blackwell requires CUDA >=12.8 and FP4 weights.
- [Nunchaku Z-Image example](https://raw.githubusercontent.com/nunchaku-ai/nunchaku/main/examples/v1/z-image-turbo.py):
  concrete transformer/pipeline classes and Turbo settings.
- [Stable release](https://github.com/nunchux-ai/nunchaku/releases/tag/v1.2.0):
  official API requests for the historical `nunchaku-tech` and `nunchaku-ai` names
  redirect to `nunchux-ai`; the CPython 3.12 Torch 2.9 Linux wheel has SHA-256
  `6196fbea888d6719fd7aff7ec58f1618ff37c35d4542f2c595aadd2a242e500f`.

Lock command:

```bash
uv pip compile torch-server/runtime/requirements.in --generate-hashes --emit-index-url --python-version 3.12 --python-platform x86_64-unknown-linux-gnu --index-strategy unsafe-best-match --output-file torch-server/runtime/requirements.lock --cache-dir launcher-data/cache/torch-qualification/uv
```

`--require-hashes --only-binary=:all:` is enforced at installation. Runtime updates
never resolve new dependencies during model loading and never acquire model assets.

## Current evidence

- Existing app-manager suite passed before the new fixtures (79 tests).
- Added real loopback archive and Python subprocess fixtures: checksum failure,
  dependency failure, validation failure, previous-runtime usability after manager
  restart, process-group cancellation, legacy-file preservation, selection-marker
  isolation, running-runtime removal refusal and release-resolution cleanup.
- Latest app-manager regression suite: 87 passed. A repeated cancellation test
  initially exposed child termination lag; it passes after reusing the existing
  pinned process-group observation and reaping the leader last.
- App-manager Clippy with `--all-targets -- -D warnings` passed.
- Torch unit suite: 18 passed. Ruff check and format check passed.
- The first complete dependency installation failed its native import validation:
  Nunchaku 1.2.0\+torch2.9 requires `libcudart.so.13`. The original CUDA 12.8
  candidate was rejected and removed without publication. Revised the candidate
  to the official Torch 2.9.1 CUDA 13.0 build; host driver is 595.84.
- The corrected real installation completed successfully in
  `launcher-data/cache/torch-qualification/install`, using the production installer
  and a locally served unpublished candidate bundle. The recipe's Torch and
  torchvision artifacts came from the official PyTorch wheel index; other
  dependencies use the pinned sources. This historical bundle qualification is
  not public release discovery or packaged-app acceptance.
- The first real attempt exposed missing extraction-root creation. It failed before
  installation, published no runtime, and was corrected before the second attempt.

Current upstream installer reproduction (use an empty, isolated root):

```bash
cargo run --manifest-path rust/Cargo.toml -p pumas-app-manager --example qualify_torch_runtime -- launcher-data/cache/torch-qualification/upstream-install
```

The example uses the sidecar embedded in the app and downloads the official
hash-pinned wheels. It does not test GitHub release discovery or the packaged
desktop UI. Installation also requires the supported Python, Linux, network, and
GPU prerequisites. The preceding local archive-server workflow was for the
historical unpublished candidate bundle and is no longer the current installer
reproduction.

## Upstream release and wheel contract

The official release is `pytorch/pytorch` tag `v2.9.1`; its GitHub release
metadata identifies the source release, while installable wheels are hosted
separately. The managed installer uses direct, SHA-256-pinned Torch and
torchvision wheels from the official PyTorch wheel host.
The Pumas app embeds the serving sidecar and lock. No Pumas-hosted Torch release
or tag is required or created. Only `v2.9.1` is currently mapped to a qualified
recipe; other upstream versions remain unavailable until their full recipe is
qualified. Packaged-app discovery and shared-UI installation acceptance remain
pending.

## Live host baseline

At initial inspection, Pumas was PID 661433 with gateway `http://127.0.0.1:37941/v1`. Its `/v1/models`
returns two ready Qwen VLM IDs. Port 20617 belongs to the llama.cpp router.
`get_system_resources` on `/rpc` returned GPU total 25,651,314,688 bytes and used
22,808,625,152 bytes (23.89 GiB total, approximately 2.65 GiB free).
No model was unloaded by this implementation. After continuation, those processes
and the gateway listener were no longer present. A separate baseline Pumas release
was started against an isolated telemetry root on port 18765. Its current
`get_system_resources` response reports 4,213,178,368 bytes used, approximately
19.96 GiB free. Port 18765 is this isolated instance, not a configured Tuldok gateway.
The installation validates only small CUDA execution and sidecar health, not
pipeline fit. Current telemetry must be queried again before model generation.

## M2 overlap

The adapter payload is being completed before runtime publication. Shared UI
acceptance remains pending; M2 source changes are not GPU acceptance. The Python
lease suite passes 21 tests on the host. The sandbox cannot reliably deliver the
threaded event-loop wakeups used by the cancellation fixture; host evidence is
retained separately. Shared dependency checks recognize the direct-wheel package
name and refuse in-place repair of an installed Torch bundle.

[Official PyTorch previous-version instructions](https://pytorch.org/get-started/previous-versions/)
list Torch 2.9.1 and torchvision 0.24.1 with CUDA 13.0. The corrected hash lock
uses that distribution. Production staged validation completed successfully:
Torch 2.9.1\+cu130, CUDA 13.0, RTX 5090 Laptop GPU, healthy sidecar protocol 1.
The installer atomically registered `torch-runtime-0.1.0` in the isolated root.
The bundle hash was `328dc486fb7dd0bcb28746d001418d539d1e9077f6b3fd06c37bf0b66a994caf`.
Subsequent source changes still need a new bundle installation before final
generation/browser acceptance. No public runtime release was created.

## Runtime 0.1.1 and shared desktop selection

The production installer qualified and registered `torch-runtime-0.1.1` beside
0.1.0. SHA-256 `1c2ae39c58ad3d8654cf2249d42fb8edec3c002db207e765614946f257f0b721`.
Log: `runtime-0.1.1-install.log`. Native dependencies are unchanged; the payload
includes current cancellation handling. The relocated 0.1.0 environment also
passed CUDA execution and protocol health (`runtime-0.1.0-relocated-health.log`).

The actual enabled Electron desktop, backed by the release RPC on isolated root
`install`, listed both versions. Selecting 0.1.0 and then 0.1.1 updated shared
`lastSelectedVersion` metadata accordingly. At that historical checkpoint,
discovery still pointed at the Pumas repository and required its custom bundle
assets. The corrected source now discovers supported upstream PyTorch tags and
installs official wheels; packaged-app discovery and shared-UI installation
acceptance remain pending, as recorded in the current upstream contract below.

The current 90-test app-manager suite passes on the host. Four HTTP-fixture tests
cannot bind sockets inside the sandbox; they passed on the host with the rest.

The shared desktop created a managed Torch profile, started its native sidecar
on port 13786, and stopped it. Switching versions while that profile ran was
refused and active selection remained 0.1.1. A checksum-corrupted 0.1.2 candidate
failed before publication while the 0.1.1 sidecar remained healthy; metadata still
contained only 0.1.0/0.1.1 (`real-failed-update-preservation.json`). After stopping
the profile, the desktop removed inactive 0.1.0 from the isolated root.

Live desktop checks exposed and fixed two shared UI gaps: switch failures were
only logged, and installed versions disappeared when the remote catalogue was
empty. Both now have regression coverage (8 shared UI tests passed) and installed
rows were verified in the actual desktop. No remote releases were invented.

The production installer also installed 0.1.1 into the actual launcher root for
upcoming image acceptance (`main-runtime-0.1.1-install.log`). NVIDIA wheel URLs
were refetched despite pip caching, including repeated fetches from the same URL;
the earlier progress explanation about different index URLs was incorrect.

Runtime 0.1.2's first main-root installation failed during an NVIDIA wheel stream
read timeout. The staged directory was removed; 0.1.1 and legacy user files
remained. This coincided with repeated Hugging Face network retries; pipeline
transfer subsequently recovered to about 26 MB/s at 75%. Explicit installation
retry completed (`main-runtime-0.1.2-install-retry.log`): native GPU validation,
protocol health, atomic publication and shared registration passed. The main
gateway was gracefully restarted and shared `switch_version` selected 0.1.2.
Real-image acceptance remains pending. Subsequent Ruff formatting changes only
whitespace in the compatibility and validation modules; the next runtime payload
will include that formatting.

## Corrected runtime 0.1.6 exact-candidate qualification

TIPC source `826a270958f7182bd8108c879b4b7fa1da5cab7d` supplies the candidate
sidecar; no candidate-owned product path differs between that commit and the
packaging head. The immutable recipe is protocol `3` with
`["image_generation"]`. Requirements-lock SHA-256 is
`d4073f8e8a1d8b20b48a275a2c63a6d2c084369e831f093b8903a0047c4ca0f4`.

The exact archive is
`launcher-data/cache/torch-qualification/assets/0.1.6/pumas-torch-runtime-linux-x86_64.tar.gz`,
SHA-256 `74f9b593dff1e447a73571dc465efd520238ba0c56d2730393a17eee2b97426c`.
It installed through the production `VersionInstaller` into isolated root
`launcher-data/cache/torch-qualification/tipc-0.1.6-install`; the installed
runtime is 5.2 GiB.

The first attempt exposed that the strict Rust recipe decoder had not evolved
with the required capability field. Fix
`ab3a95a9369a5471127003fcd8e6fff529596cb5` consumes additive capabilities and
requires `image_generation`. Its regression first reproduced the exact failure,
then passed; all 107 app-manager tests pass. The same archive bytes were reused,
so the fix did not obscure artifact identity.

Production staged validation passed with CPython 3.12.3, Torch 2.9.1+cu130,
CUDA 13.0, RTX 5090 Laptop GPU capability `(12, 0)`, real CUDA execution, and a
live sidecar health response with status `ok`, protocol `3`, and capability
`image_generation`. TIPC-09 is passed. Existing runtime installations were not
changed; the candidate was not installed into the main launcher root, selected,
activated, published, or made a default.

The installed candidate then served the real Nunchaku FP4 rank-128 pipeline from
an isolated launcher/model index. A disconnected 1280×720 request logged
cancellation requested at `23:30:56.981` and completed at `23:30:57.584`; a
following request began only after cleanup and produced a valid PNG in 7.276
seconds. TIPC-04 passed. The refreshed Tuldok flow then generated, displayed,
saved, and imported a real 1280×720 result in 10.036 seconds; saved PNG SHA-256
is `cdf9ae632f621606032d91975f86f960d546b39349a83ce6b851b50390fb52a3`.
TIPC-10 passed. The model/profile/gateway were stopped afterward, GPU occupancy
returned to baseline, and main-root runtime selection remained `0.1.4`.
