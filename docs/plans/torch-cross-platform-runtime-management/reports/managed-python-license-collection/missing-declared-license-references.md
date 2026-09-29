# Missing declared upstream license references

**Status:** Open release attribution blocker. Owner: Pumas release/licensing review.

CodeRabbit identified a reference to `licenses/LICENSE.zstd.txt` in the Windows
CPython 3.14.7 `PYTHON.json` that is absent from the retained license files.
On 2026-09-29 the exact official Windows full archive was downloaded again:

- Asset: `cpython-3.14.7+20260901-x86_64-pc-windows-msvc-pgo-full.tar.zst`
- SHA-256: `5363ec4aab59c24417f9877217aae95ca17f9ae6eb99c3bbfb25e4a76dcadafe`
- Size: 49,262,780 bytes; both size and digest match the retained manifest.
- The archive itself contains 19 `python/licenses/` files and omits the declared
  zstd file. This is an upstream evidence gap, not an omitted extraction.

The retained metadata and manifest remain byte-exact. Do not insert a guessed
license text or misrepresent an externally sourced file as an archive member.
The current inventory covers collected files; it does not certify every declared
reference. Audit of all three retained metadata records found:

- `windows-x86_64-cpython-3.14.7`: `licenses/LICENSE.zstd.txt`.
- `macos-arm64-cpython-3.14.7`: `licenses/LICENSE.zlib-ng.txt`, `licenses/LICENSE.zstd.txt`.
- `linux-x86_64-cpython-3.14.7`: `licenses/LICENSE.zlib-ng.txt`, `licenses/LICENSE.zstd.txt`.

Before release acceptance, obtain authoritative terms and exact source/version
mapping for the missing bundled component from the upstream provider, retain
separate provenance for any supplemental legal files, and update the notice
generator and inventory from that evidence. Native runtime test success does
not close this licensing gate.
