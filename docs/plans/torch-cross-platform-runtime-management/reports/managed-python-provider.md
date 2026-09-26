# Managed Python Provider Admission

**Status:** Current-source manual native run
[36229508586](https://github.com/MrScripty/Pumas-Library/actions/runs/36229508586)
on runtime commit `21041697` passed Torch 2.14.0 CPU/Core RPC installation and
restart on Linux x86_64, Windows x86_64 MSVC, and macOS arm64. Each target
provisioned Pumas-managed CPython 3.14.7 with uv 0.12.18, resolved and installed
25 hashed official artifacts, persisted the interpreter across restart, passed
the CPU-operation and resolver-probe checks, completed protocol 3 sidecar
trial/stop, and shut down gracefully with safe cleanup. Retained reports:
[Linux](v2.14.0-linux-current-source-cpu-rpc-restart-acceptance/README.md),
[Windows](v2.14.0-windows-current-source-cpu-rpc-restart-acceptance/README.md),
and [macOS](v2.14.0-macos-current-source-cpu-rpc-restart-acceptance/README.md).
The three target-specific CPython full-archive notice supersets are included in
the generated 0.7.0 attribution inventory. Native RPC acceptance does not
establish Electron UI-driven installation, packaged Windows/macOS Torch
installation, CUDA/MPS execution, or v2.14.0 Tuldok image-generation support.

The current-branch local Linux AppImage/deb backend separately passed the full
Torch 2.14.0 CPU/Core install/restart acceptance with backend `PATH` cleared;
see the [packaged Linux acceptance](v2.14.0-linux-packaged-cpu-acceptance/README.md).
This verifies the packaged RPC backend but not user interaction through the
Electron UI. Windows/macOS packaged installation, CUDA/MPS, and v2.14.0 Tuldok
image-generation support remain unverified.

The attribution gate hashes the Rust provider source and checks each
`NativeTarget::pin()` arm against its exact target triple and uv SHA-256. It
also checks the selected install-only URL against the reviewed full-archive
release/flavor mapping, validates the full-archive SHA-256, and verifies an
exact one-to-one set of legal file paths and hashes. The 0.7.0 inventory
contains 371 package entries, 78 hashed inputs, and 57 CPython legal texts;
Windows legal bytes retain CRLF.

**Decision:** Use the app-manager's exact-hash-pinned uv release to provision
stable, standard CPython distributions from uv's embedded Python Build Standalone
catalog. Pumas requires CPython 3.10 or newer for the sidecar and has no upper
minor-version cap. Do not use a host Python, ambient `uv`, runtime `latest` URL,
shell installer, system package manager, PATH shim, or Windows registry
registration.

## uv target artifacts

The official uv `0.12.18` release is immutable and was published 2026-09-22.
It is the latest published release selected for this pin; the unreleased
`0.12.19` development version is not used. The three shipped target
archives and published SHA-256 checksums are:

| Pumas target | Official asset URL | Published SHA-256 |
| --- | --- | --- |
| Linux x86_64 GNU | [`uv-x86_64-unknown-linux-gnu.tar.gz`](https://releases.astral.sh/github/uv/releases/download/0.12.18/uv-x86_64-unknown-linux-gnu.tar.gz) | `89eadd7c76fc063887959510d5ba0ab1264dfd5f1143b925ddb73021a40acf16` |
| Windows x86_64 MSVC | [`uv-x86_64-pc-windows-msvc.zip`](https://releases.astral.sh/github/uv/releases/download/0.12.18/uv-x86_64-pc-windows-msvc.zip) | `cae6a3bc25239f83dffb467a4b180508d9da23986c04639ebfa44e43e6a84bff` |
| macOS arm64 | [`uv-aarch64-apple-darwin.tar.gz`](https://releases.astral.sh/github/uv/releases/download/0.12.18/uv-aarch64-apple-darwin.tar.gz) | `cf40e0c6a202190ccd9e0406dcfdd5b2d6668a9a5c779b17948963df32aafe5b` |

The digest values come from the release's official `.sha256` assets for [Linux](https://releases.astral.sh/github/uv/releases/download/0.12.18/uv-x86_64-unknown-linux-gnu.tar.gz.sha256), [Windows](https://releases.astral.sh/github/uv/releases/download/0.12.18/uv-x86_64-pc-windows-msvc.zip.sha256), and [macOS](https://releases.astral.sh/github/uv/releases/download/0.12.18/uv-aarch64-apple-darwin.tar.gz.sha256). The upstream [release page](https://github.com/astral-sh/uv/releases/tag/0.12.18) marks the release immutable and documents GitHub artifact attestations. The selected official Python Build Standalone records came from uv's embedded catalog. Manual native RPC acceptance exercised the pinned provider on Linux, Windows, and macOS and retained each target's uv archive SHA-256 and selected CPython identity.

The app-manager must download the exact target URL into a private temporary file,
enforce a finite size/time budget, verify SHA-256 before extraction, extract only
the expected executable, and atomically publish it under the private managed
Python root. A failed download, digest, or extraction leaves no executable
eligible for invocation. This is runtime bootstrapping from versioned assets,
not a package-time bundled binary.

## Interpreter catalog and provenance

uv documents that available Python versions are frozen for each uv release and
that its managed CPython distributions come from
[python-build-standalone](https://docs.astral.sh/uv/concepts/python-versions/).
The exact uv binary pin therefore pins the catalog implementation and data
compiled into that release; Pumas must not fetch catalog metadata from `main` or
maintain a second Python-version allowlist.

At runtime, the manager asks the pinned uv binary to enumerate downloadable
interpreters for the current native target, validates the returned structured
records, filters to stable standard CPython 3.10+, and sorts by numeric version
descending. It selects the newest stable version within each minor and installs
that exact version; it does not keep an upper minor allowlist. For each selected
interpreter the retained runtime record stores the full version, catalog key,
source URL, target triple, pinned uv version and archive SHA-256, canonical
executable path, and executable fingerprint. uv performs the Python archive
integrity check against the checksum carried by its embedded metadata; provider
output alone is not treated as proof of the installed interpreter's identity.
The manager re-probes the installed interpreter's implementation, version,
native architecture, and wheel tags before use.

The public Python catalog page identifies the current tier-one standard CPython
range through 3.14 and states that 3.15 is pre-release support. Candidate
stability is decided from structured version metadata, not from a hard-coded
upper limit or an assumption based on a Torch wheel filename. A stable Python
minor absent from the pinned catalog remains unavailable until a reviewed uv
provider update adds its artifact and native evidence.

The provider implementation runs `uv python list --managed-python
--only-downloads --show-urls --output-format json`. The pinned uv 0.12.18
provider returned structured URL-bearing records and provisioned CPython 3.14.7
on each shipped target.
The live v2.14.0 CUDA 13.2/Core preview resolved 44 artifacts. A separate CPU
Core preview resolved 25 artifacts, and `install_version` installed its
retained wheel resolution, checked installed identity and CPU operation,
published `v2.14.0`, and allowed explicit selection. The same RPC acceptance
started the managed sidecar, passed health and protocol 3, stopped its owned
generation, and shut down the backend successfully. Linux ran with Python,
pip, and uv absent from the backend's child `PATH`; it directly executed the
installed venv and verified that its base interpreter resolves to the recorded
managed executable and hash. Provider version `0.12.18` and archive SHA-256
`89eadd7c76fc063887959510d5ba0ab1264dfd5f1143b925ddb73021a40acf16` are
retained with the runtime, resolution, probe, and backend log artifacts. The
temporary launcher root was removed after evidence collection. The CUDA wheel
set was resolved but not installed. Provider fixtures cover native filtering,
stable-version ordering, the 3.10 floor, exact-version install selection, and
malformed catalogs.

## Licensing and remaining admission gates

The uv release uses the [MIT/Apache-2.0 license policy](https://docs.astral.sh/uv/reference/policies/license/).
The exact `0.12.18` [MIT](https://raw.githubusercontent.com/astral-sh/uv/0.12.18/LICENSE-MIT)
and [Apache-2.0](https://raw.githubusercontent.com/astral-sh/uv/0.12.18/LICENSE-APACHE)
texts are pinned in `scripts/release/licenses/sources.json` with SHA-256 values
`860e3d7a86b84e6a7012c7a635fc64df475cebc6cce34dfeb73a5982ec58176c`
and `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`.
They are included in the 0.7.0 attribution inventory for the uv runtime executable
downloaded by Pumas, which is not bundled in the application package.

The Python Build Standalone documentation says distribution archives include
license texts and individual bundled dependencies carry their own terms
([distribution runtime and licensing](https://github.com/astral-sh/python-build-standalone/blob/main/docs/running.rst)).
The Linux x86_64, Windows x86_64, and macOS arm64 CPython 3.14.7 full archives
were downloaded from the official release API and verified by SHA-256. Native
RPC acceptance confirmed CPython 3.14.7 selection on all three targets. Each
target has 19 license texts, raw `PYTHON.json`, and a separate full-archive
manifest in the [target archive evidence index](managed-python-license-collection/README.md).
The generated 0.7.0 notices include all three target-specific full-archive
license supersets. The selected install-only URL and its CPython/provider
identity remain recorded separately from each full archive's SHA-256.

Further admission remains pending for packaged desktop Torch installation,
provider cancellation/tamper/retry and durable-retention cases, and any
device-execution or Tuldok claims. The native RPC reports establish the
CPU/Core install and sidecar lifecycle only; they do not qualify CUDA, macOS
MPS, or v2.14.0 image generation.
