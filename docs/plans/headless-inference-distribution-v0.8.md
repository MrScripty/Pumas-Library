# Draft v0.8 inference headless distribution

This slice prepares local candidate assembly and consumer admission. It does not
authorize a release/tag, change the existing thirteen-asset release inventory,
or qualify any actual Pumas/ORT inference distribution. Source baseline:
`5e114f6d8e4559e0a4d67e56000b423120a0fde0`.

## Audited targets and acceptance boundary

| Candidate | Rust target | Native runner | Format | Actual inference qualification |
| --- | --- | --- | --- | --- |
| linux-x86_64 | x86_64-unknown-linux-gnu | ubuntu-24.04 | tar.gz | unavailable |
| windows-x86_64 | x86_64-pc-windows-msvc | windows-2025 | zip | unavailable |
| macos-arm64 | aarch64-apple-darwin | macos-15 | tar.gz | unavailable |

These are the existing headless platform candidates, not newly accepted host
binding tuples. `bindings/support-matrix.json` accepts no host-language binding
tuple. The old artifact plan says macos-14; the current workflow uses macos-15.
Record the actual native runner/toolchain rather than reuse that stale label.

`scripts/release/artifact-plan.json` currently selects five full desktop, five
no-inference desktop, and three **no-inference** headless outputs. Those headless
outputs cannot satisfy the requested inference distribution. RPC defaults enable
`inference-plugins` but omit S3; the intended combined build must explicitly use
`--no-default-features --features s3,inference-plugins`, `--locked`, the exact
native `--target`, and the production release profile. This tool never builds it.

On main, `/health` only reports status. Runtime loading is lazy, so startup does
not prove ONNX loading. `get_status` has a semver and `get_launcher_version` reads
a short launcher-root Git identity: neither binds the compiled full source tree,
protocol/schema, modalities, or shipped native closure.

## Ownership and shared contract dependency

Modality owns inference request/result/capability schema. Discovery owns readonly
discovery, advertisement and bootstrap. Packaging reserves only:

- `scripts/release/headless-inference-plan.json`
- `scripts/release/headless_inference.py`
- `scripts/release/test_headless_inference.py`
- `scripts/release/headless-inference.test.mjs`
- `scripts/release/fixtures/headless-rpc-fixture.c`
- this document

The parent assigned discovery ownership of one additive `PumasBuildInfo` and
protocol compatibility handshake type. Its actual fields and route are pending
discovery's export. Packaging proposes additive exact release version, full source commit/tree,
build ID, Rust target, compiled features, protocol version and owner-exported
schema digest. Modality supplies the required modality identifiers. No new RPC
route, discovery implementation or competing runtime wire type is introduced.

The helper's `compatibility` JSON is a **provisional packaging projection**: an
operator-pinned expected identity plus fixed local GET paths/JSON pointers into
the owner's actual responses. Its semantic field names do not define response
field names or a new HTTP protocol. The only checked-in route is the controlled
C fixture's `/fixture-handshake`, which is explicitly not a Pumas route. Before
actual consumer acceptance, bind this projection to discovery's shared type,
exact owner schema bytes and actual route, or replace it with that owner's
export. Feature/modality arrays currently compare in exact order; the owner
contract must supply the same representation. Do not infer a canonicalization
rule from these fixtures.

Lantern and Tuldok can consume the existing HTTP interface after archive SHA256
and live owner-contract validation; this does not require a new SDK. Pantograph
and Eidetic need one immutable Rust source/schema cohort. The reviewed Pantograph
revision `038dacaaa98ebd007c32e13d4608726ca5ccf63a` pins Pumas
`26a84e323cae566a46a8f76bef48fa1010aed48b`. Reviewed Eidetic main
`c4587c11911af355c2446d0befa5ac1cbde8f3e3` uses a sibling path and prepares
`a94fd92021f27fdeedb6e2de6e01c41c250ef576`. Choose a new integrated immutable
cohort only after the feature lanes are integrated; update manifest, lockfile,
preparation/CI pins and exact consumer tests together. Exported source also needs
explicit identity evidence rather than skipping verification when `.git` is absent.

## Explicit local input and package contract

`headless_inference.py assemble` accepts an explicit filename-to-local-file map,
caller-trusted build/runtime/compatibility records with SHA256 pins, exact schema
bytes and an output path outside the checkout. It performs no downloads, Cargo
builds, model discovery/inference or publication. Inputs must be regular,
nonempty files; no directory scan copies ambient files or credentials.

The build-record adapter currently requires `version`, `target`, `host`, `profile`,
`features`, `source: {head, tree}`, `build_id`, `inference_enabled`, `binary_sha256`,
`command` and `rustc`. Target and host must match the native tuple; profile must
be release. A later trusted build producer must bind these assertions to observed
Cargo artifacts/features, actual compiler/runner, and immutable source. This
structural adapter is not a replacement for `s3_build_provenance.py` or independent
production-build evidence.

Metadata admission rejects unknown fields in every generated manifest, build,
source, runtime, compatibility, handshake-request and file-item object before
serializing metadata. It accepts bounded ASCII identity tokens, fixed local
GET paths and JSON pointers, exact digests, typed sizes, and a bounded single-line
`rustc -V` version. Additional compiler details stay in separate reviewed evidence.
Features are exactly `s3` and `inference-plugins`; modality identifiers remain the
owner-supplied bounded tokens. No values are silently redacted or reinterpreted.

Build command metadata accepts only explicit offline, locked, native-targeted
`cargo build` release argv for `pumas-rpc`: the repository `--manifest-path
rust/Cargo.toml` form or Rust-workspace form, with `--no-default-features` and
the two required features. It also accepts the reviewed repository invocation
with `--bin pumas-rpc`, flags ordered as `--target TARGET --release`, and
`--message-format=json-render-diagnostics`. Both feature order spellings are
admitted; arbitrary extra flags, environment assignments, shell command strings,
and command arguments carrying unrelated data are rejected. Preserve the actual
admitted invocation rather than relabeling it. Synthetic canary tests show extra
metadata and appended arguments cannot reach `build-record.json` or
`manifest.json`; these are disclosure-admission controls, not a credential
scanner or proof that supplied metadata is authentic.

The runtime record requires native target, version `1.24.2`, loader entry,
trusted source archive SHA256 and explicit native-library closure `{name:
{sha256, bytes}}`. The exact loader basename is `libonnxruntime.so`,
`onnxruntime.dll`, or `libonnxruntime.dylib`. Materialize a versioned Unix library
as a regular file at the exact loader basename and bind those bytes; archives
reject links. The current loader's C API/minor-version checks do not verify the
exact runtime pin or source bytes. The existing staging glob also does not
establish dependency closure. A trusted target-specific runtime producer and
native loaded-library evidence are required.

Output is `pumas-rpc-inference-{version}-{target}.{tar.gz|zip}` with flat files:
the plain RPC executable, explicit native library closure, `LICENSE.txt`,
`THIRD-PARTY-NOTICES.txt`, `protocol-schema.json`, `build-record.json`,
`manifest.json`, and `SHA256SUMS`. An adjacent `.sha256` binds final archive bytes.
The schema digest binds the exact supplied valid JSON bytes; this slice invents
no schema normalization algorithm. Metadata is bounded to 1 MiB; payload to 4 GiB,
130 archive members. Tar uses USTAR and rejects PAX/GNU extension headers before
their bodies can allocate. Checksums prove byte identity, not authenticity or
license completeness. All manifests remain `unverified_candidate`.

Example interface, using separately reviewed local records and hashes:

```bash
python3 scripts/release/headless_inference.py assemble \
  --inputs /tmp/pumas-inputs.json \
  --build-record /tmp/build-record.json --build-record-sha256 "$BUILD_RECORD_SHA" \
  --runtime-record /tmp/runtime-record.json --runtime-record-sha256 "$RUNTIME_RECORD_SHA" \
  --contract /tmp/owner-contract.json --contract-sha256 "$OWNER_CONTRACT_SHA" \
  --schema /tmp/owner-protocol-schema.json \
  --output /tmp/pumas-rpc-inference-0.8.0-linux-x86_64.tar.gz
python3 scripts/release/headless_inference.py verify \
  --archive /tmp/pumas-rpc-inference-0.8.0-linux-x86_64.tar.gz \
  --sha256 "$ARCHIVE_SHA" --destination /tmp/pumas-fresh-extraction \
  --contract /tmp/owner-contract.json --contract-sha256 "$OWNER_CONTRACT_SHA"
```

`verify` hashes and parses one private archive snapshot, rejects unsafe names,
duplicates/case collisions, links, sparse/extension members, unexpected files,
declared-size and inner/outer checksum mismatches. Extraction must be fresh.
The operator must retain custody of the extracted directory; concurrent edits
during execution are outside this harness's acceptance contract.

`verify --start` additionally rehashes extracted payloads, verifies the actual
native host, starts the exact extracted executable with a fresh launcher root and
ephemeral loopback port, projects the live owner response and compares identity
with strict JSON types, then requires graceful stop. Loader injection/search
variables (`LD_*`, `DYLD_*`) and ambient `ORT_*`/`PUMAS_*`/Python overrides are
removed; handshake HTTP requests bypass proxies. XDG/AppData/registry paths are
isolated. HOME, PATH and system
runtime dependencies remain; this is not a filesystem/network sandbox or a loaded
dependency audit. Unix uses SIGINT; Windows uses a new process group and
CTRL_BREAK_EVENT, requiring a usable native console and actual native evidence.
Forced shutdown fails acceptance. Output always keeps native runtime loading
and real inference **unqualified**.

## Tested slice and remaining release gates

The Python suite covers both archive formats across all three target labels,
malicious members/extension headers, bounded metadata, trusted hashes, private
snapshot custody, schema/source/feature/content mismatches, and a controlled
Linux C process's exact extraction, live identity, sanitized loader environment
and graceful shutdown. Boolean/integer confusion and live schema/revision
mismatches fail. Neither synthetic runtime bytes nor the C executable are real
Pumas/ORT evidence. Windows/macOS process and inference evidence is unavailable.
The Node wrapper is discovered by the existing source release test glob:

```bash
python3 -m unittest discover -s scripts/release -p test_headless_inference.py -v
node --test scripts/release/*.test.mjs
```

Before changing release inventory/workflow or publishing, complete:

1. Integrate S3, required modalities and discovery; discovery exports one shared
   protocol/build-info type and bind the packaging projection to it.
2. Produce exact production-build and trusted native-runtime closure records
   for each actual native runner, with no implicit ORT/model downloads.
3. Exercise exact consumer extraction/start/owner handshake/clean shutdown on
   each tuple. Add early-exit, timeout, invalid-port, redirect and forced-stop
   lifecycle tests when binding the real bootstrap contract.
4. With separately authorized, explicitly provided real model fixtures, prove
   local model load, required modality inference, unload, actual loaded native
   bytes and failure boundaries. Current health and synthetic tests are inadequate.
   Existing Linux `verify-packaged-onnx.py` inherits `ORT_DYLIB_PATH`; remove ambient
   runtime substitution in the eventual acceptance harness before claiming closure.
5. Review current target notices/SBOM, complete dependency/security and platform
   signing/notarization requirements, and assemble final-file checksums/provenance.
   The adjacent archive hash is not a complete release metadata inventory.
6. Obtain the parent/owner's release decision. This slice changes no tag, release,
   existing artifact plan, modality-owned gateway file, PR46 or main merge.

Assembly binds explicit inputs but is not reproducible-build proof: archive
timestamps/compression metadata are not normalized. Version manifests remain
on main's current version until the integrated version bump owner acts.
