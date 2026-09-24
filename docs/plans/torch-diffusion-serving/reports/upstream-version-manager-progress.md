# Upstream Torch version manager progress

Date: 2026-09-23. This report records implementation checks, not a completed
real-UI acceptance run. The prior [A1 packaged acceptance](a1-packaged-acceptance.md)
remains evidence only for its original `v2.9.1` / CPython 3.12 / CUDA 13.0 /
Linux x86_64 combination.

## Current behavior

- Stable `vMAJOR.MINOR.PATCH` PyTorch releases are discovered independently of
  the handwritten `v2.9.1` recipe, up to ten GitHub release pages. Prereleases,
  nightly builds, and source builds remain outside managed installation scope.
- The qualified 2.9.1 recipe is retained. Other stable releases resolve official
  binary artifacts for an installed Python 3.10–3.13 interpreter. The CPU index
  is the default; `PUMAS_TORCH_BUILD` can select an official CUDA/ROCm index.
  Python provisioning is not supported.
- The pip resolution report, every resolved wheel URL and SHA-256, the exact
  package versions, and scoped probe results are stored with the installation.
  Installation verifies the exact Torch version and build. It does not require
  optional Nunchaku or diffusion adapters. Probe failures for CPU tensor and
  sidecar app construction are recorded separately from package installation.
- Installation uses a staging directory and does not select or default an
  unverified runtime. Explicit selection and startup use existing lifecycle
  operations; startup waits for `/health` and returns its execution error.

## Checks run

- Rust Torch installer suite: 14 passed, including discovery of `v2.10.0`
  and `v2.9.0`, unverified publication without activation/default, cancellation,
  failed staging, and preservation of existing installs.
- Resolver fixtures: 3 passed for exact official build, SHA-256 recording,
  wrong-build rejection, and missing-hash rejection.
- Frontend dialog tests: 15 passed. Older-patch and installed-offline fixture:
  1 passed. Frontend TypeScript check passed.
- Real `v2.10.0` CPU / CPython 3.12 resolution was attempted. Sandbox DNS could
  not reach `download.pytorch.org` or PyPI; an elevated 90-second attempt also
  timed out. No real non-2.9.1 install, sidecar start, or UI acceptance is claimed.

## Acceptance still required

The desktop flow must expose exact resolved Python/build/artifacts before
installation, allow a user to select among available CPU/CUDA/ROCm builds and
interpreters, and show unavailable combinations with discovered alternatives.
Real desktop tests must then cover installation, trial, basic use, adapter
failure, cancellation, switching, and restart for a non-2.9.1 version. Deep
probe results also need hardware-aware invalidation and an on-demand UI view.

The first serving scope is the existing Nunchaku Z-Image and FLUX.2 Klein image
models. A generic user-defined PyTorch model plugin contract is a separate
decision. The PR acceptance gate should use the actual desktop and Tuldok
browser to select a release, preview and install exact artifacts, explicitly
select and start the runtime, load one of these models, and generate a PNG at a
requested resolution. Record the exact Torch/Python/build, adapter versions,
model assets, GPU, gateway and Tuldok revisions. Run this before requesting PR
acceptance; a merge alone is not evidence of inference compatibility.
