# M1 runtime implementation and qualification

Status: native/GPU/sidecar qualification passed for the CUDA 13.0 candidate; **A1 shared UI/distribution acceptance remains pending**.

## Implemented source

- `VersionManager` still owns discovery, installed state, active/default selection,
  progress and cancellation. Torch discovery now selects `torch-runtime-*` releases
  with the platform archive and SHA-256 companion from `MrScripty/Pumas-Library`.
  Ordinary Pumas releases and PyTorch source releases are not installable Torch runtimes.
- `VersionInstaller` verifies the archive checksum, rejects special archive entries
  and escaping paths, checks recipe/platform/Python/protocol identity, creates a
  staged venv, installs the hash lock, and runs GPU/import/sidecar validation before
  atomic non-replacing publication and shared metadata registration.
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

## Candidate recipe

`torch-runtime-0.1.0`, sidecar protocol 1, Linux x86_64, CPython 3.12, NVIDIA sm_120.
Candidate dependencies include Torch 2.9.1+cu130, torchvision 0.24.1+cu130,
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
  and locally served unpublished bundle. Dependencies come from the real pinned
  artifact sources. This is package qualification, not public release discovery
  or desktop acceptance.
- The first real attempt exposed missing extraction-root creation. It failed before
  installation, published no runtime, and was corrected before the second attempt.

Reproduction:

```bash
python3 scripts/package-torch-runtime.py --output launcher-data/cache/torch-qualification/assets
python3 -m http.server 18764 --bind 127.0.0.1 --directory launcher-data/cache/torch-qualification/assets
cargo run --manifest-path rust/Cargo.toml -p pumas-app-manager --example qualify_torch_runtime -- launcher-data/cache/torch-qualification/install http://127.0.0.1:18764
```

The example uses the production installer but deliberately does not claim to test
GitHub discovery. It must use an isolated root.

## Release-source dependency

On 2026-09-13 the official GitHub API returned six Pumas releases and **no
`torch-runtime-*` release**. The new source and packaging are concrete, but normal
shared UI discovery cannot accept the candidate until an approved, qualified
runtime bundle is published. No remote release or tag has been created.

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
`lastSelectedVersion` metadata accordingly. Public discovery/install/update via
the remote catalogue still requires a published bundle (T7). Production installer
qualification does not substitute for that missing distribution acceptance.

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
