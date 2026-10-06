# Q2 private selected-runtime lifecycle qualification — 2026-10-06

This slice carries the reviewed live selected local-install capability through
runtime/provider validation, the actual probe and owned metadata publication.
Its evidence is synthetic local arithmetic/protocol execution. Automatic/preview
adoption of this new staged pipeline remains OFF; existing callers and catalog
settlement are unchanged. Parent owns independent
review, hosted CI, PRs and integration.

## Exact lineage

- Reviewed base: `6bddf85867364f9b31f000ed6aa7436a5fb50872`,
  tree `4ca5fbe6480acb6dac227af3018411fb8106f5f7`.
- Branch: `feat/torch-selected-lifecycle-6bddf858`.
- Tested source: `568a5f1fca76b2253088d97f71b1d950e5beacc9`, tree `5e5d26a3540d468a7a737ff5d54ccbebe0d78260`.
- Main observed/preserved: `5e114f6d8e4559e0a4d67e56000b423120a0fde0`.
- [Admission](write-sets.md), [source/evidence hashes](torch-selected-lifecycle-2026-10-06/provenance.json).

No dependency/lock, shared cleanup interface, frozen native/importer/watcher/S3/
manifest, original recipe/pin or provider provisioning changes. The only existing
Torch embedding change extracts its unchanged compiled source table for validation.
The private selection fence accepts an explicit relocation supplied by the live
publication owner; ordinary selection keeps its original location and behavior.

## Private capability and validation

Private `InstalledSelectedPacket::publish(RuntimeLifecycleContext)` consumes the
live installed packet. The context provides the same catalog-owner runner,
selected runtime/interpreter, approved managed provider identity and executable
SHA256, original release/tag and CPU/no-adapter profile. No public Rust API,
Deserialize, cold import or caller adoption path is added. Refusal retains the live
installed packet; publication refusals also retain the validated runtime/provider.
Success retains that same capability in `PublishedSelectedRuntime`.

Provider identity remains distinct from the selected venv executable. The managed
path binds the existing managed-provider record and full Python version to the
approved observation and rechecks actual provider bytes. Tests explicitly use a
copied existing local fixture provider with `kind=existing-local-fixture-provider`;
they do not fabricate a provisioned uv/CPython archive claim.

Before movement, the owner verifies compiled embedded runtime source bytes and
source namespace, the original selected proof, exact installed manifest and every
selected installed member. It writes a new bounded
`pumas.selected-runtime-profile.v1` and truthful private CPU runtime recipe, then
uses existing verified package movement into the owned venv. No resolver report
is fabricated. `probe_runtime.py --selected-profile` reads this explicit input;
its default resolution-based mode remains unchanged. Profile input is a regular
nonlinked file capped at2MiB with an independent bounded read and schema check.

The genuine probe runs under the existing managed child/process-group owner with
cleared environment and retained packet custody. `-I -B -X pycache_prefix=<fresh
owned directory>` isolates lookup from previous stage bytecode; `-B` alone only
suppresses bytecode writes. The fresh cache must still be empty afterward. See
[Python command-line options](https://docs.python.org/3/using/cmdline.html) and
[cache-prefix behavior](https://docs.python.org/3/library/sys.html#sys.pycache_prefix).

Probe acceptance binds the actual environment/profile-byte digest, original
selected interpreter hash/path, selected distribution versions and passing core
status. After the child, original producer/target/request/catalog identities,
whole acquired inputs, exact selected lock/projection/roots/constraints/packet,
installed report/requirements/manifest, provider, runtime sources/namespace/
profile/recipe/probe bytes and selected installed members are rechecked. These
checks repeat at the relocated runtime after publication movement and before
acknowledgment. The venv bootstrap baseline remains separately owned.

## Publication and uncertainty

One registered blocking publication job owns the complete validated capability
through no-replace rename, directory sync, existing durable installed metadata
publication/readback, final fences, acknowledgment and failure cleanup. There is
no await gap inside these effects. Abandoning the caller cannot revoke its lease.
The validation phase has a separate120-second deadline; it does not promise to
interrupt in-progress blocking filesystem I/O.

Separate `pumas.selected-runtime-install.v1` records bind the complete original
catalog receipt canonical hash/acquisition id, genuine local proof, provider,
probe, exact installed metadata and native owned-directory identity. The Adopted
catalog snapshot/receipt remains byte-for-byte unchanged in the live capability.
The2MiB record limit is checked before publication. A distinct pending marker
owner is deliberately rejected by legacy cleanup; no cold acceptance/replay is
implemented here.

Known pre-metadata failures remove only the matching owned destination and
pending marker. Matching committed metadata with a lost acknowledgment produces
`CommittedWithoutAck` and retains output/marker. Unknown metadata or foreign
directory identity produces `RecoveryRequired`, preserving evidence and foreign
output. The retained typed refusal holds custody while the caller handles it.
This slice does not qualify every interruption, hard-kill/power-loss recovery or
post-restart approval of these new records.

## Qualification and retained evidence

- [126 affected Python controls](torch-selected-lifecycle-2026-10-06/python.log):
  pass, no skips, including selected-profile schema/size/link checks and legacy
  probe/catalog/target/lock/local-pip/RECORD regressions.
- [Six new Rust lifecycle tests](torch-selected-lifecycle-2026-10-06/lifecycle.log):
 28 actual full-flow fixture controls: successful publication; real failed CPU
  probe;17 post-probe mutations; six publication/movement faults; probe cancellation
  and abandonment; registered publisher abandonment after rename.
- [64 existing affected Rust controls](torch-selected-lifecycle-2026-10-06/rust.log):
  pass, all selected ignored qualification controls explicitly included. New tests
  are separately qualified above, so they are skipped only in this regression run.
- [33 pre-build dependency graphs](torch-selected-lifecycle-2026-10-06/graphs.log),
  [strict Clippy](torch-selected-lifecycle-2026-10-06/clippy.log),
  [Ruff](torch-selected-lifecycle-2026-10-06/ruff.log),
  [rustfmt](torch-selected-lifecycle-2026-10-06/rustfmt.log): pass.

Post-probe mutations cover selected module/RECORD, provider/selected executable,
original producer/target/catalog, lock/projection/selected packet, installed
manifest/report, runtime recipe/profile/probe and original/added source. Each
mutation follows actual completed probing. Movement controls corrupt an installed
member or installation record; both refuse and roll back owned output. Foreign
replacement preserves its keeper and moved original with the unresolved marker.
Before/after rename failures reclaim known owned output; after-metadata failure
preserves the committed output. Probe cancellation/abandonment drain actual child
and grandchild; paused publication abandonment proves custody with an absent
caller through actual metadata acknowledgment.

[Retained publication evidence](torch-selected-lifecycle-2026-10-06/publication/selected-runtime-install.json)
contains the genuine probe/profile, original receipt/target/request/lock/projection,
local installation report/manifest/requirements, three exact selected wheel ZIPs
and all21 actual installed members (including three RECORD files). A
[read-only verifier](torch-selected-lifecycle-2026-10-06/verify_evidence.py)
rederives RECORD with the unchanged single production owner, checks all wheel/
member hashes, and proves record/profile/probe/local-proof/catalog bindings:
[result](torch-selected-lifecycle-2026-10-06/evidence-verification.log).
This is evidence verification, never capability reconstruction. Original temporary
runtime directory, interpreter/provider files and metadata store were disposed
with the fixture; their live identity/final metadata checks are supported by the
executed owner/tests rather than retained directory identity.

The separately retained actual public-pip
[trace](torch-selected-lifecycle-2026-10-06/python-pip-network.trace),
[invocation](torch-selected-lifecycle-2026-10-06/python-pip-invocation.json) and
[control](torch-selected-lifecycle-2026-10-06/network.log) observe its IPv6 loopback
support probe and no IP retrieval calls. No firewall/namespace/network policy was
changed or enforced. This control belongs to the local installation leg, not a
claim of denied network throughout the whole lifecycle.

## Limits and next existing-plan work

The native target is still glibc2.41 and remains unsupported by the pinned exact
public-uv target projection. Positive fixtures explicitly declare a synthetic
compatible2.40 floor; production never downgrades it. The synthetic Torch module
actually computes matrix multiplication and the explicit protocol fixture builds
and executes health, but neither proves real Torch/provider/native-platform
compatibility, sidecar socket startup, GPU/adapter/model execution, full recipe
closure or installed distribution parity. Image generation/startup remain
truthfully `not tested` in the genuine probe.

Q2/AQ-PACKAGES/P1 blocker classification remains unchanged. Real-provider
acceptance, target/tool/source/provider parity and enforced network denial remain
separate. No Library transfer retry, model download, account credential use,
external review contact or main integration occurred. The next existing Q2 work
is retained publication reconciliation under the existing owner (plan AD8 and contract interruption
requirements / AC05), followed by remaining exact target/tool/source/provider parity before
coordinated automatic/preview caller adoption.

## Reproduction

Use the already qualified local public uv; do not invoke a downloader. All Cargo
commands were offline/locked, jobs1, debug0, incremental disabled and serialized
under `/tmp/pumas-cargo-admission/cargo.lock` with the shared bounded target dir.

```sh
source /workspace/.pumas-tools/env.sh
export PUMAS_QUALIFIED_UV=/tmp/pumas-uv-public-eval/tool-env/bin/uv
export CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/pumas-uv-offline-build/target
flock /tmp/pumas-cargo-admission/cargo.lock cargo test --offline --locked \
  --manifest-path rust/Cargo.toml -p pumas-app-manager --features test-support \
  selected_runtime_ -- --include-ignored --test-threads=1 --nocapture
flock /tmp/pumas-cargo-admission/cargo.lock cargo test --offline --locked \
  --manifest-path rust/Cargo.toml -p pumas-app-manager --features test-support \
  version_manager::installer::torch::torch_ -- --include-ignored --test-threads=1 \
  --skip selected_runtime_ --nocapture
flock /tmp/pumas-cargo-admission/cargo.lock cargo clippy --offline --locked \
  --manifest-path rust/Cargo.toml -p pumas-app-manager --features test-support \
  --tests -- -D warnings
PYTHONPATH=/tmp/pumas-q2-test-packages:torch-server/tests python3 -m unittest -v \
  test_probe_runtime test_consume_selected_wheels test_offline_wheel_selection \
  test_wheel_catalog_owner test_wheel_target test_target_observation \
  test_install_verified_wheels test_qualified_wheel_catalog
PYTHONPATH=/tmp/pumas-q2-test-packages python3 \
  docs/plans/artifact-acquisition/reports/torch-selected-lifecycle-2026-10-06/verify_evidence.py
```
