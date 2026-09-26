# Managed CPython full-archive license evidence

The entries below are byte-verified Python Build Standalone full archives for
CPython 3.14.7. Each manifest records the official release and asset IDs, exact
API-provided download URL, size, SHA-256, raw `PYTHON.json`, and hashes of all
license-directory files and declared license references.

| Target | Evidence status | Archive | License texts |
| --- | --- | --- | ---: |
| Linux x86_64 | Selected during accepted Torch 2.14.0 CPU/Core installation | [Manifest](linux-x86_64-cpython-3.14.7/managed-python-licenses/full-archive-manifest.json) | 19 |
| Windows x86_64 | Selected during accepted native Torch 2.14.0 CPU/Core install and restart; archive SHA-256 `5363ec4aab59c24417f9877217aae95ca17f9ae6eb99c3bbfb25e4a76dcadafe` matches the retained manifest | [Manifest](windows-x86_64-cpython-3.14.7/managed-python-licenses/full-archive-manifest.json) | 19 |
| macOS arm64 | Official target archive candidate; native selection unverified | [Manifest](macos-arm64-cpython-3.14.7/managed-python-licenses/full-archive-manifest.json) | 19 |

The Windows license files differ from the Linux and macOS files only by line
endings; all 19 match after newline normalization. The Linux and macOS sets
currently match byte for byte. The accepted native Windows install confirms
CPython 3.14.7 selection and matches this candidate full-archive manifest,
including all 19 license-file hashes. Windows notices still need integration
into release attribution. The macOS collection remains a candidate until its
native installation confirms the selected archive; its notices also remain
outside release attribution.

The Linux evidence README describes the accepted installation that produced
its selected archive record. All per-file hashes can be checked directly
against the manifests.
