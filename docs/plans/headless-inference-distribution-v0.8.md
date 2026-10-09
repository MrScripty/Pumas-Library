# Draft v0.8 inference headless distribution

The local producer builds and verifies inference-enabled headless candidates
from an exact clean source checkout and explicit reviewed runtime/schema inputs.
Actual inference-enabled archives and consumer acceptance remain unqualified.
The package version is 0.7.0; the strict v0.8 gate refuses this source before
Cargo. A lockstep source/version update is a prerequisite for a v0.8 build.
The ordinary release inventory continues to select no-inference headless assets.

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
native `--target`, and the production release profile. The new opt-in local
`headless_inference_build.py` producer runs that exact command and invokes the
existing assembler. Active `build.yml` tag jobs still produce only the three
no-inference headless archives. Automatic inference release workflow wiring and
activation are separate work; this local producer adds no remote asset upload.

In the pinned cohort, `/health` only reports status. Runtime loading is lazy, so startup does
not prove ONNX loading. `get_status` has a semver and `get_launcher_version` reads
a short launcher-root Git identity: neither binds the compiled full source tree,
protocol/schema, modalities, or shipped native closure.

## Shared identity and discovery contract

The package decoder consumes the existing operation, discovery and bootstrap
contracts. It defines no separate live identity or network protocol.

The shared identity contracts are `PumasBuildInfo` in
`pumas-core/src/build_info.rs`, `InstanceDescription` in discovery, and
`HttpServiceDescription` at `/.well-known/pumas`. Packaging now consumes those
exact fields rather than accepting configurable GET paths or JSON pointers.
`headless_discovery.py` is the packaging decoder, not a new network protocol.

Compatibility record version 2 pins the RPC `build_info` and `core_build_info`
objects, including package version, build ID, source revision, target, namespaced
compiled features, named protocol versions and named schema versions. Ordinary
builds may omit provenance; this distribution verifier refuses missing provenance.
The RPC and library builds must agree on their source/package/build/target context.
The existing `version`, `source_commit`, `source_tree`, `build_id`,
`schema_sha256`, `inference_enabled` and `features` fields remain package/build
provenance. Source tree and schema-byte digest are not fields of the live owner
advertisement and are not falsely projected into it. Compiled inference features
never establish model readiness or a modality capability. No live modality claim
is inferred from build metadata.

The additive `pumas-rpc --describe-local-http --launcher-root PATH` command is a
read-only projection of the existing `LocalDiscovery::borrow_http_service`. It
requires an explicit existing root, reads the selected registry, authenticates
core context, observes the registered HTTP generation and reauthenticates the
core. Failure stays failure; it never claims a root, starts an owner, downloads a
model, stops the borrowed service or exports connection tokens. The resulting
JSON is a point-in-time observation. The command exits and retains no lifetime
lease for later client calls or inference.

The packaging verifier executes that command from the verified extracted binary,
validates both pinned build objects and selected root/registry library ID/core
and HTTP generations, fetches only `/.well-known/pumas` on a numeric loopback
endpoint without redirects/proxies, then repeats the authenticated observation.
The selected root is bound to the advertised root using the native filesystem's
existing directory identity (`samefile`), including Windows ordinary versus
extended-length canonical spellings. Relative, missing, non-directory or
unavailable observed paths are refused. No prefix removal or case folding is
used to guess identity; this check does not retain a physical lifetime lease.
Any change fails. A matching arbitrary HTTP response without the existing
Pumas-owned authentication path is insufficient. Library IDs remain registry
context, not physical-library identities. Consumers must keep model acquisition
inside Pumas and query/reuse existing local libraries before considering a
separately authorized acquisition. These checks do not implement a competing
model cache, downloader or owner reclamation rule.

The five-consumer cohort is **Lanternwake, Tuldok, Pantograph, Eidetic and
Chrema**. Lanternwake requires Audio→Text; Tuldok requires Image→Text captioning.
Archive and HTTP identity checks alone do not implement or qualify those
operations. The facade accepts a selected model, typed input and
desired output without a named capability, using the existing
`POST /v1/model-operations` route. It matches declared available adapters and
requires `semantic_task` when multiple operations fit the same modalities;
option shape cannot select a task. Legacy named-capability requests keep their
strict parsing and existing admission/cancellation paths. See the
[RPC request contract](../../rust/crates/pumas-rpc/README.md).

Text→Text, Text→Embeddings and Text→Image use existing adapters when declared
available. Audio→Text resolves only to the qualified owned endpoint; the
shipping installed runtime remains unavailable. PNG/JPEG Image→Text uses the
bounded dedicated llama.cpp adapter when selected task/profile and live vision
readiness match. Mixed audio message content and PCM output have no executable
adapter. See the [image-to-text contract](../contracts/image-to-text-v0.8.md).
Codec, property and synthetic backend results do not qualify real vision models.

Rust consumers must pin one immutable source/schema cohort across manifests,
lockfiles and setup steps. HTTP consumers must pin the transport/schema and
binary/archive cohort and authenticate selected-owner reuse. Each consumer
record must identify the exact source or archive hash, schema, model/runtime
bytes and device. Exported source requires explicit identity evidence when
`.git` is absent. Controlled adapter tests and tiny untrained model forwards
do not establish pretrained quality or packaged consumer acceptance.

## Explicit local input and package contract

`headless_inference.py assemble` accepts an explicit filename-to-local-file map,
caller-trusted build/runtime/compatibility records with SHA256 pins, exact schema
bytes and an output path outside the checkout. It performs no downloads, Cargo
builds, model discovery/inference or publication. Inputs must be regular,
nonempty files; no directory scan copies ambient files or credentials.

The build-record adapter requires `version`, `target`, `host`, `profile`,
`features`, `source: {head, tree}`, `build_id`, `inference_enabled`, `binary_sha256`,
`command` and `rustc`. Target and host must match the native tuple; profile must
be release. `headless_inference_build.py` now produces that record from observed
Cargo artifacts rather than asking a caller to assert that Cargo ran. Before and
after Cargo, Git's `--show-toplevel` must identify the actual build directory by
native `samefile` directory identity. A redirected `core.worktree`, unavailable
root or changed root is refused; valid linked worktrees and symlink aliases of
the same directory remain usable. Git/compiler observations use the explicit
build environment, with Git routing/config and compiler/wrapper overrides
refused. These are unsigned before/after observations under trusted local
custody, not proof that compilation inputs were immutable during a build.
It also checks a clean source head/tree, actual native compiler host, checked release
profile, forbidden flag/config overrides, current S3 attribution, and exact
core/RPC features. It forces `ORT_SKIP_DOWNLOAD=1`, offline/locked compilation,
and one Cargo build job. An uncached official dependency or missing native tool
is an explicit prerequisite; no automatic download or fallback is attempted.

The compiled executable's existing `--build-info` output must exactly match the
pinned shared RPC `PumasBuildInfo`. The core projection removes only RPC's
`discovery.rs` additions (component, RPC feature names, local HTTP protocol and
HTTP advertisement schema); it must also match the pinned core object. This is
an adapter of the shared struct, not a parallel identity definition. It does
not substitute for authenticating the running core/HTTP owner during native
acceptance. Full source tree and exact schema-byte digest remain separate package
bindings, as they are absent from the live shared struct.

Compatibility/runtime records and the explicit runtime filename-to-local-path
map require caller-reviewed SHA256 pins. Runtime target/version/loader/member
hashes and sizes are checked before Cargo and again before assembly. License and
notice bytes must match the source's checked license/S3 attribution. Source and
attribution are checked again after Cargo. Failure retains Cargo JSON/stderr;
only success writes the archive, adjacent checksum, build record and evidence.
Output must be fresh and outside the checkout. The producer obtains no runtime,
model, credentials or external account, and publishes nothing. A supplied record
hash proves bytes, not authenticity or completeness of the native runtime closure.

Opt-in native production, with a v0.8 source and reviewed inputs:

```bash
python3 scripts/release/headless_inference_build.py \
  --target linux-x86_64 \
  --contract /tmp/owner-contract.json --contract-sha256 "$OWNER_CONTRACT_SHA" \
  --runtime-record /tmp/runtime-record.json --runtime-record-sha256 "$RUNTIME_RECORD_SHA" \
  --runtime-inputs /tmp/runtime-inputs.json --runtime-inputs-sha256 "$RUNTIME_INPUTS_SHA" \
  --schema /tmp/owner-protocol-schema.json \
  --license LICENSE --notices docs/release-attribution/0.7.0-s3/THIRD-PARTY-NOTICES.txt \
  --output-dir /tmp/pumas-inference-candidate
```

`runtime-inputs.json` contains exactly the runtime record's native-library names
mapped to explicit local regular files. The resulting archive and evidence stay
`unverified_candidate`; the command does not start a service, load ORT or run a
model. Review/rebuild target notices for the final source version before
acceptance; the current path reflects the unchanged source inventory.

Metadata admission rejects unknown fields in every generated manifest, build,
source, runtime, compatibility, build-info advertisements and file-item object before
serializing metadata. It accepts bounded ASCII build identity tokens, named protocol/schema versions,
exact digests, typed sizes, and a bounded single-line
`rustc -V` version. Additional compiler details stay in separate reviewed evidence.
Build command features are exactly `s3` and `inference-plugins`; the shared
build-info objects additionally list the actual namespaced compile features. No values are silently redacted or reinterpreted.
The assembler creates a detached projection of only those public fields and
revalidates it before staging payloads. Nested lists/maps cannot retain aliases
to caller-owned metadata, and the actual admitted command is selected from the
closed local argv forms. Later mutations of the input records cannot add data to
staged metadata or change the returned manifest.

`read_pinned_json` checks byte identity against a caller-supplied SHA256; its name
does not imply confidential content or authenticated provenance. The pinned
records, including build IDs and protocol/schema advertisements,
are intended for public distribution and require caller review before their pins
are supplied. Format bounds and projections do not identify secrets deliberately
placed inside otherwise valid public fields. Schema, license and notice files
remain explicit reviewed payload inputs.

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
ephemeral loopback port, authenticates and compares the actual owner response with strict JSON types,
then requires graceful stop of only the exact child it launched. Loader injection/search
variables (`LD_*`, `DYLD_*`) and ambient `ORT_*`/`PUMAS_*`/Python overrides are
removed; handshake HTTP requests bypass proxies. XDG/AppData/registry paths are
isolated. HOME, PATH and system
runtime dependencies remain; this is not a filesystem/network sandbox or a loaded
dependency audit. Unix uses SIGINT; Windows uses a new process group and
CTRL_BREAK_EVENT, requiring a usable native console and actual native evidence.
Forced shutdown fails acceptance. Output always keeps native runtime loading
and real inference **unqualified**.

`verify --attach-root EXISTING_ROOT --registry-db EXISTING_REGISTRY` instead
observes the already-running selected owner. Both explicit paths are required;
`--attach-root` and `--start` are mutually exclusive. It executes only the
read-only observer and HTTP reads, never starts a server or sends a shutdown
signal. A missing, incompatible, changed or unreachable owner is refused without
fallback. Choose a registered library through Pumas's local discovery before
passing these paths; the verifier does not guess a most-recent library.

## Tested slice and remaining release gates

The Python suite covers both archive formats across all three target labels,
malicious members/extension headers, bounded metadata, trusted hashes, private
snapshot custody, schema/source/feature/content mismatches, and a controlled
Linux C process's exact extraction, live identity, sanitized loader environment
and graceful shutdown. Boolean/integer confusion, missing owner identity, build/schema/revision
mismatches, wrong roots and generation changes fail. Compatible borrowed attach
leaves the controlled owner running; refused attach has no startup fallback.
Early exit, readiness timeout, invalid port, redirects, forced-stop failure and
observer timeout without stopping a borrowed service are also covered. Neither synthetic runtime bytes nor the C executable are real
Pumas/ORT evidence. Windows/macOS process and inference evidence is unavailable.
The existing three-platform native QA job also runs `OwnerContractTests`; on
Windows this executes the ordinary/extended-length root identity regression.
Linux-only results cannot establish that native Windows check. The Node wrapper
is discovered by the existing source release test glob:

```bash
python3 -m unittest discover -s scripts/release -p 'test_headless_inference*.py' -v
node --test scripts/release/*.test.mjs
```

Inference-enabled release acceptance requires:

1. Select one immutable source/schema cohort. Integration and controlled
   contracts do not qualify model inference.
2. Produce exact production-build and trusted native-runtime closure records
   for each actual native runner, with no implicit ORT/model downloads.
3. Exercise exact consumer extraction/start/owner handshake/clean shutdown on
   each tuple, including forced-stop failure and actual authenticated attach.
   The controlled C fixture deliberately stands in for authentication; it is not
   real Pumas binary acceptance.
4. With verified, explicitly provided real model fixtures, prove
   local model load, required modality inference, unload, actual loaded native
   bytes and failure boundaries. Current health and synthetic tests are inadequate.
   Existing Linux `verify-packaged-onnx.py` inherits `ORT_DYLIB_PATH`; remove ambient
   runtime substitution in the eventual acceptance harness before claiming closure.
5. Review current target notices/SBOM, complete dependency/security and platform
   signing/notarization requirements, and assemble final-file checksums/provenance.
   The adjacent archive hash is not a complete release metadata inventory. Wire
   inference-enabled archives into the release workflow only after the
   source/version/native acceptance gates; current tag jobs remain no-inference.
6. Complete the final release review against those exact source and artifact
   identities. Source integration alone is not release acceptance.

Assembly binds explicit inputs but is not reproducible-build proof: archive
timestamps/compression metadata are not normalized. Version manifests remain
consistent with the selected source version. Physical-store crash recovery
requires separate native acceptance; historical cold-reopen observations and
hosted CI results do not establish current deployment/platform qualification.

## Remaining implementation and qualification gaps

- `AudioRuntimeOwner::for_installed_runtime(candidate, &selected)` implements
  conditional ownership transfer from retained interpreter/dependency/sidecar
  capabilities and the exact prepared model allocation. Its shipping policy
  resolver remains unavailable. A trusted runtime recipe, complete enforced
  loader/model read closure and pinned real execution/lifecycle evidence are
  required before audio availability. Fixed positive policies exist only in
  unit tests. See [conditional construction](../contracts/installed-audio-constructor.md).
- The dedicated Image→Text adapter validates bounded PNG/JPEG input and provider
  envelopes. Real runtime/model/projector forward, caption semantics, managed
  teardown and consumer acceptance remain unqualified.
- The composed acquisition bridge admits bounded validated GGUF, Safetensors,
  selected Transformers/Diffusers packages and the documented static FLOAT ONNX
  graph class through shared acquired-model qualification. Verified acquisition
  bytes and registered library assets do not establish runtime or inference
  readiness. The bounded Transformers class excludes Cohere ASR, and GGUF bundles
  do not admit a second GGUF projector. See the [acquired-package contracts](
  artifact-acquisition/reports/acquisition-model-bridge-2026-10-08.md).
  Real AWS, independent-provider and MinIO acceptance remain separate gates.
- Actual native runtime loading, inference-enabled archive acceptance,
  per-platform inference/disposal, final consumer pins and release workflow
  activation remain unqualified. Controlled archive/process fixtures exercise
  their stated boundaries only.
