# Acquisition checksum policy

The user selected this policy on 2026-10-10: verify a supplied SHA when
acquiring a file, retain the SHA, and perform subsequent content verification
only on explicit request. An installed file may legitimately differ from its
downloaded artifact. A historical checksum is not automatic startup, read,
reconciliation, or execution authority.

## Ownership and contracts

| Concern | Owner | Required behavior |
| --- | --- | --- |
| Download verification | Acquisition service and staged runtime installers | Reject a mismatch against a supplied expected SHA before acquisition succeeds. Keep the expected and observed evidence in the existing receipt. |
| Retained checksum evidence | Existing acquisition receipts, model metadata and runtime manifests | Preserve stored SHA values. Do not rewrite live libraries or substitute invented digests. |
| Explicit integrity verification | Existing user-requested model verification operation | Continue to verify retained checksums when explicitly invoked. |
| Installed library observation | Publication, package and runtime read-source owners | Check ownership, regular-file/path bindings, required members and receipt state without checking content SHA. |
| Package selection | Runtime recipe owner | Preserve package name/version/source identity and acquisition provenance. Do not compare an installed tree with a downloaded artifact or fixed tree digest. |
| Request authentication and identifiers | Existing authentication/identity owners | Hashing request signatures, metadata identities and protocol messages is separate from rechecking downloaded file contents. |

Persisted receipts remain readable and retain their existing checksum fields.
Internal Rust/Python consumers must change together wherever content-checking
semantics change. Preserve supported path and receipt failures; do not turn a
Pending or invalid receipt into successful acquisition evidence.

## Implementation and acceptance

1. Separate historical checksum evidence from current file/namespace ownership
   in model publication and acquisition read paths. Retain explicit verification.
2. Remove automatic installed-runtime, interpreter, model and copied-source SHA
   comparisons at their owning modules and coordinated Rust/Python consumers.
   Delete fixed installed-tree digest/relocation validation as an acceptance gate.
3. Keep source recipe constraints and download-artifact checks in staged install.
   Never loosen supplied download hashes to accept a newer package.
4. Update contracts, diagnostics and tests that promise automatic content
   integrity. Tests must distinguish allowed content changes from refused path,
   receipt, file replacement and required-member failures.
5. Verify supplied-download SHA success/mismatch, checksum retention and explicit
   verification; exercise library reopen/reconciliation and runtime preparation
   without content rechecking. Run affected Rust/Python suites, both GGUF feature
   configurations, and normal cold/warm audio installs before reporting acceptance.

The prior GGUF watcher-race repair and audio dependency-pin repair remain separate
from download-reconciliation implementation. This policy supersedes their older
requirements for automatic installed-byte validation. Live model files and library
databases are outside the implementation write set.

## Coordinated cutover and compatibility

The staged runtime recipe owner derives all 68 constraints from the existing
candidate JSON, passes them through the normal resolver and validates downloaded
artifact identities before publication. Installed file SHA fields remain retained
metadata, not execution gates. Explicit `RetainedRuntimeReadSource::verify_checksums`
checks retained evidence on request; it refuses when a selected member has no
historical checksum rather than inventing one. `RuntimeReadFile::sha256` now returns
`Option<&str>` and `RuntimeReadFile::observed` represents that absence. The removed
public fixed installed-byte helper has no replacement automatic gate. Direct Rust
consumers of this prerelease candidate must recompile and update these API calls.
The affected workspace consumers are compiled and tested together.

Rust and the Python child now use `pumas-cohere-owned-v2:` followed by a digest of
framed model/artifact labels and sorted member names and lengths. This private
correlation value compares layout only; the original held owner supplies authority.
It never compares freshly hashed model contents. The coordinated producer/consumer
cutover replaces v1 atomically. Python capture has no original acquisition SHA to
supply and represents that evidence as absent. Existing persisted receipts and
model checksum fields retain their schema and values; RPC request/response shapes
are unchanged. No library migration or receipt rewrite is required for this policy.
An older installed private v1 worker requires a fresh runtime installation before
using the experimental v2 session path; incompatible hello correlation remains a
typed startup refusal with child drainage. Reading a runtime does not rewrite its
code. The shipping audio admission catalog remains closed, so this is not a newly
qualified production audio path.

The acquisition workspace owns first verification and caches its receipt against
the actual file binding. Owned partial writes invalidate that cache. Reopen admits
an authenticated complete receipt by checking metadata and current bindings;
publication and use do not rehash completed files. An interrupted, hashless member
without a durable completion receipt is still unavailable as verified evidence.
It is preserved with any fresh transfer for reconciliation, without hashing the
old file into a fabricated receipt or overwriting it. Existing source-supplied SHA
recovery remains first acquisition verification. The ongoing HTTP partial-prefix
checkpoint belongs to incomplete acquisition, not a post-download check.

No automatic checksum bypass switch was added. Each existing owner now implements
the same policy for its consumers; physical ownership, namespace, length, required
members and Ready/Confirmed receipt checks continue to refuse their own failures.
Local import/copied-source operations can record digests while streaming bytes;
those observations cannot become automatic integrity gates. Explicit deep scan
and integrity operations still compare contents when requested.

## Standards and review boundary

Coding standards MCP routing selected 33 applicable policies with no unresolved
facts under snapshot `snapshot:v1:062e8bee-26c4-43c3-8920-d4cd96fa2135`.
The implementation follows the canonical acquisition/runtime/recipe owners, the
coordinated contract evolution rules, explicit persisted/API compatibility
semantics and per-commit staged review. This is one project-wide checksum-policy
cutover, including reproducible acquisition of the audio recipe. The prior GGUF
watcher repair is already a separate commit. No download-location reconciliation
implementation or inference acceptance is included.

The active branch is `fix/main-test-validation`, based on main `ad31e391`, retained
for Jeremy's independent review and push under the existing protected-worktree
contract in the download-location reconciliation execution ledger. The original
dirty work branch and live library remain outside this write set. Generated source
attribution inventories are regenerated through their canonical producer; their
input freshness hashes are separate from acquired-file integrity checks.

Generated outputs may use the existing `rust/target`, `frontend/dist` and
`electron/dist` owners and task-specific ignored directories beneath the original
repository's `tmp/`. Runtime installation tests and their environment stay in the
repository. A separate ignored S3 build directory avoids exhausting the filesystem
containing the validation checkout. No live library or model paths are build inputs.

Declared source/documentation write set:

- `.github/workflows/build.yml`
- `docs/contracts/artifact-acquisition.md`
- `docs/contracts/audio-custody-foundation.md`
- `docs/contracts/cohere-native-cohort.md`
- `docs/contracts/installed-audio-constructor.md`
- `docs/contracts/installed-audio-session.md`
- `docs/contracts/owned-audio-lifecycle.md`
- `docs/contracts/owned-audio-transport.md`
- `docs/investigations/pr69-audio-ci-followup-2026-10-10.md`
- `docs/investigations/pr69-gguf-ci-followup-2026-10-10.md`
- `docs/plans/acquisition-checksum-policy.md`
- `docs/release-notes-0.8.0-rc.1.md`
- `docs/release-attribution/0.8.0-rc.1-s3/inventory.json`
- `docs/release-attribution/0.8.0-rc.1/inventory.json`
- `rust/crates/pumas-app-manager/examples/qualify_audio_dependencies.rs`
- `rust/crates/pumas-app-manager/src/version_manager/audio_native_cohort/tests.rs`
- `rust/crates/pumas-app-manager/src/version_manager/audio_runtime_recipe.rs`
- `rust/crates/pumas-app-manager/src/version_manager/installer.rs`
- `rust/crates/pumas-app-manager/src/version_manager/installer/torch.rs`
- `rust/crates/pumas-app-manager/src/version_manager/managed_python.rs`
- `rust/crates/pumas-app-manager/src/version_manager/mod.rs`
- `rust/crates/pumas-app-manager/src/version_manager/torch_preview.rs`
- `rust/crates/pumas-app-manager/src/version_manager/torch_read_source.rs`
- `rust/crates/pumas-app-manager/src/version_manager/torch_read_source/tests.rs`
- `rust/crates/pumas-core/src/acquisition/service.rs`
- `rust/crates/pumas-core/src/acquisition/workspace.rs`
- `rust/crates/pumas-core/src/api/installed_audio.rs`
- `rust/crates/pumas-core/src/model_library/artifact_use.rs`
- `rust/crates/pumas-core/src/model_library/download_recovery.rs`
- `rust/crates/pumas-core/src/model_library/download_recovery/import_custody.rs`
- `rust/crates/pumas-core/src/model_library/hf/download.rs`
- `rust/crates/pumas-core/src/model_library/importer/publication.rs`
- `rust/crates/pumas-core/src/model_library/importer/staging.rs`
- `rust/crates/pumas-core/src/model_library/importer/staging/tests.rs`
- `rust/crates/pumas-core/src/model_library/importer/staging/tests/publication.rs`
- `rust/crates/pumas-core/src/platform/process.rs`
- `rust/crates/pumas-core/src/runtime_profiles/audio_runtime.rs`
- `rust/crates/pumas-core/src/runtime_profiles/audio_runtime/installed.rs`
- `rust/crates/pumas-core/src/runtime_profiles/audio_runtime/installed_tests.rs`
- `rust/crates/pumas-core/src/runtime_profiles/audio_session.rs`
- `rust/crates/pumas-core/src/runtime_read_source.rs`
- `rust/crates/pumas-core/src/runtime_read_source/candidate_recipe.rs`
- `rust/crates/pumas-core/src/runtime_read_source/dependency_probe.rs`
- `rust/crates/pumas-core/src/runtime_read_source/tests.rs`
- `rust/crates/pumas-core/tests/acquired_gguf_vision.rs`
- `rust/crates/pumas-core/tests/acquired_model_bridge.rs`
- `rust/crates/pumas-core/tests/acquired_onnx_package.rs`
- `rust/crates/pumas-core/tests/s3_acquisition.rs`
- `rust/crates/pumas-core/tests/s3_acquisition/manifest.rs`
- `torch-server/installed_cohere_source.py`
- `torch-server/loaders/owned_cohere_source.py`
- `torch-server/resolve_runtime.py`
- `torch-server/tests/test_installed_cohere_source.py`
- `torch-server/tests/test_owned_cohere_source.py`
- `torch-server/tests/test_resolve_runtime.py`


## Verification on 2026-10-11

The local target is Linux x86_64, Rust/Cargo 1.92.0, Python 3.12.3 in the ignored
original-repository `.venv`, Node 24.12.0 and pnpm 10.33.0. Cargo used the locked
offline graph, four jobs and disabled development/test debug symbols. Debug
assertions and normal test semantics stayed enabled. Native lifecycle/HTTP tests
ran with the local socket/process permissions their fixtures require.

- Full default-feature Rust workspace, excluding only the repository-documented
  BEAM-hosted `pumas_rustler`, with `pumas-app-manager/test-support`: passed.
  App-manager: 323 passed, three ignored; core: 2,103 passed, 14 ignored;
  RPC: 410 passed; UniFFI: 17 passed. All integration binaries passed, including
  12 acquired-GGUF tests under default features. Core doc tests: six passed,
  eight ignored; the app-manager example doc test remains ignored.
- Headless core unit suite: 2,072 passed, 13 ignored. All headless integration
  binaries subsequently passed: 222 test executions, eight designated helper
  ignores. The 12 acquired-GGUF cases passed without default features too.
- S3-only integration binaries: 82 passed, one explicit HTTPS/signature
  qualification cohort ignored. The cold model and auxiliary-content changes
  reconcile with their original receipt/SHA; changed lengths, source SHA,
  revision, identity, missing member and invalid receipt cases remain refused.
- Final focused acquisition workspace cohort: 15 passed, including reopened
  read-only receipt reuse and invalidation before verifying a rewritten partial.
  This follow-up preserves read-only reuse on Windows: only first acquisition
  verification opens GENERIC_WRITE for its FlushFileBuffers operation.
- Windows GNU core/app-manager `--no-default-features --features test-support
  --all-targets` compilation passed. Remaining warnings are in unchanged
  platform/test helpers; this does not establish native Windows execution.
- Complete Torch Python suite after the coordinated source-ID change: 346 passed.
  Release Python suite: 103 executed, one existing skip, all remaining passed.
- Frontend: 921 passed, one skipped. Electron: 268 passed, one skipped after
  regenerating the attribution input inventory. Both production builds passed.
- Rustfmt, Ruff lint/format, affected-core/app-manager Clippy with warnings
  denied, both generated attribution inventories and workflow syntax passed.
  Workflow syntax validation did not include unavailable ShellCheck/Pyflakes.
- Real normal cold/warm audio installations and both confined dependency probes:
  passed. All 68 name/version/URL/SHA acquisition identities and managed-Python
  distribution/provider match the source recipe. Child trees drained, with
  `production_available=false`; no model, inference or ASR was exercised.

The pre-policy safetensors/ONNX/S3 integration assertions initially refused
post-verification content changes. They now distinguish valid content changes
from receipt and physical ownership failures, and explicitly prove retained SHA
values can detect the changes on request. Their original failed results are not
acceptance evidence. Earlier Python/release fixture runs lacked socket permissions;
complete native reruns passed without assertion changes or delays.

Windows GNU cross-target compilation is compilation evidence only. Native Windows
and macOS execution require hosted reruns after the maintainer pushes this branch;
prior successful jobs do not attest this source. Repository-designated manual,
helper, external-fixture and real-model ignores remain explicit. No inference,
signing, publication, live library upgrade or reconciliation-plan claim is accepted.

## Local candidate artifact plan

Rebuild the existing 0.8.0-rc.1 Linux x86_64 desktop candidate from the committed
source with the locked default-feature release RPC backend, canonical frontend
and Electron production outputs, and the previously acquired pinned ONNX Runtime
1.24.2 native pair. No new version promise, publication, binding distribution,
signature or inference qualification is selected. The Debian amd64 package and
AppImage use the repository's existing names in a fresh, ignored output directory.
They bundle the backend, native runtime and generated attribution notices; the
frontend and Rust/Python adapters are internal bundle inputs. User-visible changes
and prerelease consumer migration instructions are in
[the candidate release notes](../release-notes-0.8.0-rc.1.md).

Use the original repository's ignored Cargo target for this release because the
validation filesystem has limited space. Copy the fresh RPC and the previously
verified native pair into the canonical staging input before invoking stage-rpc.
The required local artifact evidence is staged RPC startup/shutdown, packaged
Debian/AppImage resources and RPC startup, and Debian install/remove inside the
release verifier's isolated namespace. Packaging and these smokes remain pending
until recorded below. This candidate is for local review; foreign-platform and
production inference obligations remain unsatisfied and it will not be published.
