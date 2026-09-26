# Managed CPython full-archive license evidence

The entries below are byte-verified Python Build Standalone full archives for
CPython 3.14.7. Each manifest records the official release and asset IDs, exact
API-provided download URL, size, SHA-256, raw `PYTHON.json`, and hashes of all
license-directory files and declared license references.

| Target | Evidence status | Archive | License texts |
| --- | --- | --- | ---: |
| Linux x86_64 | Selected during accepted Torch 2.14.0 CPU/Core installation; [selected install-only archive identity](../v2.14.0-linux-cpu-rpc-restart-acceptance/README.md) | [Full-archive license manifest](linux-x86_64-cpython-3.14.7/managed-python-licenses/full-archive-manifest.json) | 19 |
| Windows x86_64 | Selected during accepted native Torch 2.14.0 CPU/Core install and restart; [selected install-only archive identity](../v2.14.0-windows-cpu-rpc-restart-acceptance/README.md) | [Full-archive license manifest](windows-x86_64-cpython-3.14.7/managed-python-licenses/full-archive-manifest.json) | 19 |
| macOS arm64 | Selected during accepted native Torch 2.14.0 CPU/Core install and restart; [selected install-only archive identity](../v2.14.0-macos-cpu-rpc-restart-acceptance/README.md) | [Full-archive license manifest](macos-arm64-cpython-3.14.7/managed-python-licenses/full-archive-manifest.json) | 19 |

The Windows license files differ from the Linux and macOS files only by line
endings; all 19 match after newline normalization. The Linux and macOS sets
currently match byte for byte. The retained full-archive manifests document
license files for the same CPython release and target as the accepted
installations; their archive hashes describe those separate full archives, not
the install-only archives selected at runtime. Selected install-only archive
identities are recorded in the platform acceptance reports. Windows and macOS
provider notices still need integration into release attribution.

The Linux evidence README describes the accepted installation that produced
its selected archive record. All per-file hashes can be checked directly
against the manifests.
