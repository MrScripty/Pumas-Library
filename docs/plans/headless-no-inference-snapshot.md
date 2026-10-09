# Local Linux no-inference snapshot

`scripts/release/headless_snapshot.py` is an opt-in local Linux x86_64 producer
for a clean, explicitly pinned source checkout. It preserves the actual source
package version (currently 0.7.0) and emits local build records and archives.
The existing v0.8 inference producer still requires
its reviewed runtime closure, compatibility record, and v0.8 version gate.

The snapshot uses the repository release profile, `--locked --offline`,
`--no-default-features`, no S3, and the native
`x86_64-unknown-linux-gnu` target. It forces `ORT_SKIP_DOWNLOAD=1`, one Cargo job,
and no incremental compilation. Actual Cargo JSON must show RPC features `[]`
and library features `["hf-client"]`, with a successful release build. The
executable's observed shared `PumasBuildInfo` must match the supplied source
revision, source version, build ID, target, and actual compile features. Its
library identity uses the existing `core_projection` adapter. The manifest
records both the clean package source HEAD/tree and the separate clean producer
HEAD/tree. These are unsigned local observations, not authenticated attestations.

The manifest sets `inference_compile_enabled: false`. Inference routes and live
model capabilities are unavailable. An unconditional codec build dependency
does not establish a reachable codec operation in this package. Native loading,
startup, authenticated discovery, route absence, graceful shutdown, and consumer
acceptance need separate observed qualification. A build on Debian Linux does
not establish the Ubuntu release runner or other native platform compatibility.

The desktop contract comes from the real Rust DTO exporter
(`rust/crates/pumas-rpc/src/contract/export.rs`, CLI
`--export-desktop-contract` under `export-contract`). There is no core contract
export directory at this source snapshot; the shared build identity is
`rust/crates/pumas-core/src/build_info.rs`. The schema export record must bind
the package source HEAD/tree, exact schema bytes/hash, export command, exporter
binary hash, and observed exporter shared build identity. An existing broader
exporter may declare DTOs for unavailable routes; its features remain in this
separate export record and are never projected into package support. The six
Image→Text DTO schemas use `PUMAS_IMAGE_CONTRACT_EXPORT_DIR` in their existing
test exporter and are separate from this desktop schema file.

Default-feature release attribution is checked using the existing genuine
inventory/license closure checker. Its notices are included as an explicitly
conservative attribution superset; the manifest does not claim an exact
no-default dependency inventory. The archive allows exactly nine regular files:
the RPC executable, project license, notices, RPC/core shared build identities,
desktop schema, schema export record, manifest, and `SHA256SUMS`. File sizes and
hashes are bound in the manifest; the checksum inventory also binds the manifest.
The producer emits an exact archive SHA256 and refuses existing outputs,
symlink path components, output inside either checkout, and unclean source.
Assembly and extraction use the same per-member bounds: each legal text file
(`LICENSE.txt` and `THIRD-PARTY-NOTICES.txt`) is at most 8 MiB; each JSON, schema,
and checksum file, including generated manifest/checksum metadata, remains at
most 1 MiB. The executable and complete payload retain the 4 GiB bound. Real
checked notices at the implementation's recorded source are 4,423,904 bytes.
Legal-text limits remain separate from the stricter JSON/schema metadata limits.

All supported producers refuse global or target-prefixed `TURBOJPEG_*` variables
by presence, including empty values. `turbojpeg-sys` 1.2.0 checks
`TARGET.upper().replace('-', '_') + '_'` first; empty static/dynamic/shared values
mean true. Source, library/include directory/path, binding, and link-kind routes
cannot silently select different native bytes. The snapshot also refuses native
compiler and CMake routing overrides. Tools resolved through `PATH` remain a
local provenance boundary that independent native dependency inspection must
record; these script guards do not constrain direct Cargo invocations.
The sole allowed CMake environment option is `CMAKE_BUILD_PARALLEL_LEVEL=1`;
the producer forces it to keep native build work at one job as well.

## Produce a pinned snapshot

Use fresh paths outside both checkouts and the exact reviewed package HEAD/tree.
The producer itself must be committed and clean before running it. It accepts
only local schema bytes and records; it does not obtain runtimes or models.

```sh
python3 scripts/release/headless_snapshot.py \
  --repository /path/to/exact-package-checkout \
  --source-head FULL_PACKAGE_HEAD --source-tree FULL_PACKAGE_TREE \
  --build-id pumas-headless-no-inference-REV-linux_x86_64 \
  --schema /path/to/schema-records/desktop-contract.json \
  --schema-sha256 SCHEMA_SHA256 \
  --schema-export-record /path/to/schema-records/schema-export-record.json \
  --schema-export-record-sha256 RECORD_SHA256 \
  --output-dir /path/to/fresh-snapshot-output
```

A completed local build can be reused without another Cargo invocation. Supply
`--completed-build-record` and its `--completed-build-record-sha256`. That pinned
JSON has exactly `binary`, `binary_sha256`, `cargo_jsonl`,
`cargo_jsonl_sha256`, and `command`. `command` is the exact argv declared as
`COMMAND` in the producer. Preserve the package binary before a differently
featured exporter rebuild replaces the target-path executable. The producer
checks the preserved bytes and their actual `--build-info`; it records the
completed evidence as caller-supplied local custody, not proof of authenticity.

The schema export record has exactly `schema_version: 1`, `source: {head, tree}`,
`export_command: ["/path/to/pumas-rpc", "--export-desktop-contract"]`,
`exporter_binary_sha256`, `exporter_build_info`, `schema_sha256`, and
`schema_bytes`. Preserve any richer original exporter evidence separately.

`headless_snapshot.extract_verified(archive, expected_sha256, fresh_destination)`
copies the bounded archive into owned temporary storage before verifying its
pin. It refuses additional, duplicate, linked, sparse, PAX/GNU extension, or
privileged-mode members. It validates the closed payload, every checksum,
manifest byte counts/hashes, shared identities and schema bindings before
creating the destination. It streams executable bytes through files rather
than allocating the full binary in Python memory.
