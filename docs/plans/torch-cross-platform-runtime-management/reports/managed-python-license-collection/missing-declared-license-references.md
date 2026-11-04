# Missing declared upstream license references

**Status:** Resolved for 0.8.0 by supplemental texts from the pinned source
archives. Owner: Pumas release/licensing review.

The CPython 3.14.7 `PYTHON.json` files from python-build-standalone `20260901`
declare license files that the official full archives do not contain:

- `windows-x86_64-cpython-3.14.7`: `licenses/LICENSE.zstd.txt`.
- `macos-arm64-cpython-3.14.7`: `licenses/LICENSE.zlib-ng.txt`, `licenses/LICENSE.zstd.txt`.
- `linux-x86_64-cpython-3.14.7`: `licenses/LICENSE.zlib-ng.txt`, `licenses/LICENSE.zstd.txt`.

The Windows archive was downloaded again on 2026-09-29
(`cpython-3.14.7+20260901-x86_64-pc-windows-msvc-pgo-full.tar.zst`, SHA-256
`5363ec4aab59c24417f9877217aae95ca17f9ae6eb99c3bbfb25e4a76dcadafe`,
49,262,780 bytes, matching the retained manifest). It holds 19
`python/licenses/` files and no zstd file, so this is an upstream gap, not an
extraction error. The retained manifests and `PYTHON.json` files stay byte-exact.

## Resolution

`pythonbuild/downloads.py` at python-build-standalone tag `20260901` pins the
two components, and both source archives were downloaded and matched their pins:

| Declared file | Component | SPDX | Source archive | Size | SHA-256 | Text taken from |
| --- | --- | --- | --- | ---: | --- | --- |
| `LICENSE.zstd.txt` | zstd 1.5.7 | BSD-3-Clause | [`zstd-1.5.7.tar.gz`](https://github.com/python/cpython-source-deps/archive/refs/tags/zstd-1.5.7.tar.gz) | 2,440,298 | `f24b52470d12f466e9fa4fcc94e6c530625ada51d7b36de7fdc6ed7e6f499c8e` | `cpython-source-deps-zstd-1.5.7/LICENSE` |
| `LICENSE.zlib-ng.txt` | zlib-ng 2.2.4 | Zlib | [`zlib-ng-2.2.4.tar.gz`](https://github.com/python/cpython-source-deps/archive/refs/tags/zlib-ng-2.2.4.tar.gz) | 2,415,819 | `00bbd88709bc416cb96160ab61d3e1c8f76e106799af7328d0fe434dc7dd5004` | `cpython-source-deps-zlib-ng-2.2.4/LICENSE.md` |

The texts are kept byte-exact, apart from the archive evidence, in
[`scripts/release/licenses/managed-python-supplemental/`](../../../../../scripts/release/licenses/managed-python-supplemental/).
Each has a `.provenance.json` record naming the declared path, release, targets,
source archive identity, path inside that archive and the text's SHA-256. They
are not archive members and are never listed as such.

- The Torch acceptance collector accepts a declared path missing from the
  archive only when a record for that exact path, target and release exists
  and its text hash matches. Any other missing declared path still fails. Its
  manifest lists archive files under `files` and supplemental texts under
  `supplemental_files`.
- The notice generator and attribution checker require every declared path to
  be an archive member or a verified supplemental text. Each supplemental text
  gets its own notice, labelled with the source archive it came from.
