# Q2 private selected-packet local installation qualification — 2026-10-06

This slice qualifies a private live catalog→selected-packet→staged local
installation boundary with synthetic wheels. It does not publish a runtime,
rewrite the Adopted catalog receipt or activate automatic/preview selection.
Parent owns independent review, CI, PRs and merges.

## Exact lineage and source

- Accepted base: `1769b77ee037fa7fda33df84d9bdaeff22de5006`,
  tree `6eabe90bf2629da17f28a18999cfe757e33ceda2`.
- Branch: `feat/torch-selected-consumer-1769b77`.
- Tested source: `587144f6ca938451ab61234f90fbd0c1e3745b00`, tree `c7d79f9b5343cc2be480edcebbb91ab08035eb2b`.
- Source/test hashes and raw evidence hashes: [provenance](torch-selected-consumer-2026-10-06/provenance.json).
- Main observed/preserved: `5e114f6d8e4559e0a4d67e56000b423120a0fde0`.
  No main merge or old-lineage rewrite. The previous source tree correction
  `b724d505` remains `fffe76996b6d62f470ab5b9cdb1b23e699938325`.

The [admitted write set](write-sets.md) contains only private consumer/selection
composition, local pip keyword additions, embedding, tests, contracts and this
bounded evidence. No dependency, shared acquisition/cleanup interface, frozen
native/importer/watcher/S3/manifest, source/version/fallback or provider change.

## Capability, custody and API

Private `SelectedWheelPacket::consume(SelectionContext)` takes a live packet by
value. `InstalledSelectedPacket` retains that packet, its catalog receipt/grant,
original request/target and pending stage, exact installed proof bytes, and an
owned staged package path. Refusal retains the same live packet. There is no
Deserialize/cold import/acceptance path for any of these capabilities.

Managed child cleanup leases retain the entire packet, including public-pip
children and their process-group descendants. All registered blocking effects
retain that packet as well; logical refusal is nested data, not a failed cleanup
supervisor task. A separate 120-second consumer timeout bounds waiting and does
not claim to cancel blocking filesystem I/O. Cancellation and abandonment leave
custody with the child/job owner until drain. No settled shared Using lease is
recreated and no second acquisition or installation receipt is issued.

New embedded private `consume_selected_wheels.py` independently reinspects the
whole acquired catalog, original roots/direct URLs/constraints/extras closure,
target and genuine public pylock before and after local consumption. Existing
`install_verified_wheels.install` / `local_requirements` add optional keyword
`local_paths` and `requested_extras`; `validate_closure` seeds those explicit
extras. Legacy defaults, CLI resolution/recipe admission and source provenance
remain unchanged. A single extracted `validate_installation_report` serves both
legacy installation and new verification; selected paths opt into bounded report
reads and local direct/wheel/requested report-kind checks. No private pip API is
used in production. No resolver report is fabricated or supplied as a lock.

## Actual installation and final proof

Only the selected exact acquired `id/filename` paths, names, versions and SHA256
reach public pip. Alternative/unselected catalog files stay retained; no scanning,
find-links, source build, dependency resolution, ambient credential lookup or
new download is introduced. The existing public pip invocation uses `-I`,
`--isolated`, no inherited PIP options, `PIP_CONFIG_FILE=os.devnull`, `--no-index`,
`--no-deps`, `--require-hashes`, `--only-binary=:all:`, no cache/version check,
ignore-installed/no-compile, an empty owned `--target` and genuine `--report`.
The Rust child environment is cleared and uses only owned home/config/cache/temp
and fixed PATH/LANG values. Host configuration is never changed.

The genuine version1 report must match the exact selected local URL/hash/name/
version set and the selected native marker environment. The sole existing
`wheel_records.py` owner checks installed distribution identities, RECORD and
all members without importing installed package code. Rust retains bounded
report/requirements/manifest bytes before a separate read-only verification
child; that child checks their meaning and rederives the RECORD manifest. Rust
then compares retained bytes, every actual installed member and all original
catalog/packet/lock/projection/producer/interpreter evidence after the child.
The original selected/lock bytes are now retained in the live selected capability.
Private catalog request/producer comparisons use exact retained-length bounded
reads; global validation interfaces/readers remain unchanged.

Final proof schema is `pumas.selected-local-install.v1`; it binds canonical
selected packet, original catalog request/target and selected interpreter hashes,
actual pip report/requirements/installed manifest hashes and installed file
count. The private proof covers staged packages, not the venv bootstrap or
published-runtime/provider identity. Bounds are 2MiB report/requirements/packet,
32MiB installed manifest, 200,000 members and 64MiB selected expanded/installed
bytes. ZIP expanded aggregate is checked before pip; actual installed aggregate
and exact-size streaming hashes are checked afterward. Catalog limits remain.

## Qualification and retained evidence

- [115 Python controls](torch-selected-consumer-2026-10-06/python.log), including
  actual traced pip install, root extras/direct roots, report/RECORD mutations and
  all affected catalog/target/legacy local-consumer regressions: pass, no skips.
- [64 shared Rust controls](torch-selected-consumer-2026-10-06/rust.log):
  25 catalog/selection/consumer tests, 31 unchanged handoff and eight unchanged
  upstream contract regressions. All configured tool controls ran, no ignores/skips.
  Focused [5 adversarial controls](torch-selected-consumer-2026-10-06/adversarial.log)
  also pass, including all 17 successful-verifier-followed mutations.
- [33 pre-build feature graphs](torch-selected-consumer-2026-10-06/graphs.log),
  serialized offline/locked [strict Clippy](torch-selected-consumer-2026-10-06/clippy.log),
  [Ruff](torch-selected-consumer-2026-10-06/ruff.log),
  [rustfmt](torch-selected-consumer-2026-10-06/rustfmt.log),
  [diff check](torch-selected-consumer-2026-10-06/diff-check.log) and
  [evidence verification](torch-selected-consumer-2026-10-06/evidence-verification.log): pass.
- [Exact commands](torch-selected-consumer-2026-10-06/commands.txt); all seven
  source/test hashes remained unchanged throughout final qualification.

[Actual shared selected packet](torch-selected-consumer-2026-10-06/packet.json),
[genuine public lock](torch-selected-consumer-2026-10-06/pylock.toml),
[genuine installation report](torch-selected-consumer-2026-10-06/installation-report.json),
[installed member manifest](torch-selected-consumer-2026-10-06/installed-files.json)
and [private proof](torch-selected-consumer-2026-10-06/installation-proof.json)
come from an actual shared HTTP acquisition of five tiny fixture wheels,
public uv selection of three, and public pip installation of those three.
Twenty-one actual installed members, including three RECORD files, pass the
single proof owner and final Rust hash checks. Original HTTPS URLs remain in the
catalog, while the actual report contains exact acquired local file URLs.
The controlled source records exactly five payload requests throughout; the
original catalog receipt/payload and Adopted phase remain unchanged, and installed
runtime metadata remains absent. Recorded file paths are temporary fixture paths,
not live Library/model roots.

Python's actual public-pip trace and invocation exercise exact original direct
roots, root extras, constraint-selected alternatives and ambient PIP URL traps.
No IP connect/send retrieval syscall was observed. Public pip's transport import
performs an IPv6 support probe by binding `::1`; this is retained in the raw
trace, not erased or called network denial. No enforced egress-denial claim is
made. Python checker fixtures construct synthetic lock data for semantic tests;
the shared Rust evidence uses the genuine public uv lock and live capability.

Seventeen post-successful-verifier mutation cases replace report, manifest,
requirements, packet, lock, projection, roots, constraints, approved target,
original producer, unselected acquired wheel, installed member, RECORD and
selected executable, and grow three output files to sparse 32MiB. Each verifies
that actual installation and actual verification completed before the mutation,
then refuses private success with original receipt/grant retained. Cancellation
and abandonment controls add a live descendant to the actual verification child,
prove runtime/grant custody before drain, and prove no live child/descendant after
drain. Abandonment permits stage reclamation only after joined cleanup.

The [evidence verifier](torch-selected-consumer-2026-10-06/verify_evidence.py)
checks all retained identity/digest relationships. Original temporary package
bytes are not retained or reopened as capabilities; their actual RECORD/hash
checks are in the controlled test execution. No fabricated cold acceptance is
inferred from the saved JSON files.

## Remaining gates and next existing-plan work

Actual native glibc2.41 remains unsupported by the pinned public uv projection;
its original observation still refuses without downgrade. Positive controls
explicitly declare synthetic compatible2.40 fixture evidence and validate that
native tags cover the declared target. They do not qualify production2.41,
real Torch wheel volume/backend/provider operation, other tooling/platforms or
production tool provisioning.

AQ-HTTP/AQ-PACKAGES/Q2/P1 and the original blocker classification remain unchanged.
The next existing-plan work is runtime/provider validation and owned lifecycle/
publication composition for the privately checked staged packages, plus the
remaining exact target/tool/selection/source/provider parity before coordinated
caller adoption. A catalog snapshot receipt cannot be relabeled an installation
receipt. Automatic/preview activation remains OFF. Both approved Library archive
transfers stay paused. No real model/provider download, credential operation,
network/security bypass, external reviewer contact or paid service was used.

Standards checked: [public pip report](https://pip.pypa.io/en/stable/reference/installation-report/)
and [public pip install](https://pip.pypa.io/en/stable/cli/pip_install/).
