# Managed Python Provider Admission

**Status:** Candidate pin recorded; target-native download, byte verification,
package execution, clean-host provisioning, and license-material acceptance are
still pending.

**Decision:** Use the app-manager's exact-hash-pinned uv release to provision
stable, standard CPython distributions from uv's embedded Python Build Standalone
catalog. Pumas requires CPython 3.10 or newer for the sidecar and has no upper
minor-version cap. Do not use a host Python, ambient `uv`, runtime `latest` URL,
shell installer, system package manager, PATH shim, or Windows registry
registration.

## uv target artifacts

The official uv `0.12.19` release is immutable, published 2026-09-24, and points
to commit `bea138450f0e620a4ce5765b0e38cff7b9f0799f`. The three shipped target
archives and published SHA-256 checksums are:

| Pumas target | Official asset URL | Published SHA-256 |
| --- | --- | --- |
| Linux x86_64 GNU | [`uv-x86_64-unknown-linux-gnu.tar.gz`](https://releases.astral.sh/github/uv/releases/download/0.12.19/uv-x86_64-unknown-linux-gnu.tar.gz) | `23bf5552d220e0842b65c862097b2ebaeba0064b74eda5e565e77fd25969d8c8` |
| Windows x86_64 MSVC | [`uv-x86_64-pc-windows-msvc.zip`](https://releases.astral.sh/github/uv/releases/download/0.12.19/uv-x86_64-pc-windows-msvc.zip) | `6dbb02d79e419522f1c500f0adb1cddcff0cda7d59b0d66ea7f5e3b4a1b2f5f0` |
| macOS arm64 | [`uv-aarch64-apple-darwin.tar.gz`](https://releases.astral.sh/github/uv/releases/download/0.12.19/uv-aarch64-apple-darwin.tar.gz) | `a9a8df1eedeb192f2e47e40e2faabfb387db4b850209118786d42f89dde3e0ba` |

The digest values come from the release's official `.sha256` assets for [Linux](https://releases.astral.sh/github/uv/releases/download/0.12.19/uv-x86_64-unknown-linux-gnu.tar.gz.sha256), [Windows](https://releases.astral.sh/github/uv/releases/download/0.12.19/uv-x86_64-pc-windows-msvc.zip.sha256), and [macOS](https://releases.astral.sh/github/uv/releases/download/0.12.19/uv-aarch64-apple-darwin.tar.gz.sha256). The upstream [release page](https://github.com/astral-sh/uv/releases/tag/0.12.19) states that the release is immutable and publishes GitHub artifact attestations. These are published checksum values; Pumas has not yet downloaded the archives in this environment, so local verification of downloaded bytes and executable fingerprints remains an acceptance gate.

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
--only-downloads --show-urls --output-format json`. A Linux local-uv control
returned structured URL-bearing records, and provider fixtures cover native
filtering, stable-version ordering, the 3.10 floor, exact-version install
selection, and malformed catalogs. Those checks do not substitute for executing
the pinned uv 0.12.19 asset on a clean host.

## Licensing and remaining admission gates

The uv release uses the [MIT/Apache-2.0 license policy](https://docs.astral.sh/uv/reference/policies/license/).
The Python Build Standalone documentation says distribution archives include
license texts and that individual bundled dependencies carry their own terms
([distribution runtime and licensing](https://github.com/astral-sh/python-build-standalone/blob/main/docs/running.rst)).
Before release acceptance, retain uv notices and the applicable license files
for the actual selected CPython archives in Pumas' attribution inventory. Do not
infer the complete archive license set from the repository's top-level license.

Admission remains pending until all target artifacts are downloaded and checked
on native Linux x86_64, Windows x86_64 MSVC, and macOS arm64 hosts; clean-host
provisioning succeeds without Python on `PATH`; stable catalog filtering excludes
pre-releases, debug, free-threaded, and foreign-target artifacts; timeout,
cancellation, tamper, retry, and durable retention behavior pass; and license
notices are recorded.
