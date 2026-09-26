# Managed CPython full-archive license evidence

The entries below are byte-verified Python Build Standalone full archives for
CPython 3.14.7. Each manifest records the official release and asset IDs, exact
API-provided download URL, size, SHA-256, raw `PYTHON.json`, and hashes of all
license-directory files and declared license references.

| Target | Evidence status | Archive | License texts |
| --- | --- | --- | ---: |
| Linux x86_64 | Selected during accepted Torch 2.14.0 CPU/Core installation | [Manifest](linux-x86_64-cpython-3.14.7/managed-python-licenses/full-archive-manifest.json) | 19 |
| Windows x86_64 | Official target archive candidate; native selection unverified | [Manifest](windows-x86_64-cpython-3.14.7/managed-python-licenses/full-archive-manifest.json) | 19 |
| macOS arm64 | Official target archive candidate; native selection unverified | [Manifest](macos-arm64-cpython-3.14.7/managed-python-licenses/full-archive-manifest.json) | 19 |

The Windows license files differ from the Linux and macOS files only by line
endings; all 19 match after newline normalization. The Linux and macOS sets
currently match byte for byte. The Windows/macOS collections do not prove that
native Torch previews selected CPython 3.14.7. Keep full provider attribution
pending until native Windows/macOS installation confirms the selected archives
and the corresponding target notices are added to release attribution.

The Linux evidence README describes the accepted installation that produced
its selected archive record. All per-file hashes can be checked directly
against the manifests.
