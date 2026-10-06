# Supplied-null target approval repair — 2026-10-06

The consumer CLI now distinguishes an omitted `--target-observation` flag from
supplied invalid approval data. Supplied JSON null, malformed/non-object JSON and
missing files refuse before package/output staging or pip. Valid absent-context
legacy and bound consumption remain unchanged. This is a two-line production
parser guard; no target/packet/custody, resolver or catalog policy is changed.

## Frozen reproduction and exact source

Branch `fix/torch-null-approval-64d62cdc` starts at frozen
`64d62cdc90fa2c3ec9424d99cde0865bd919e793`, tree
`f6f5e269a361f2270bfc6676ab705025cd10b514`. Tested repair source:
`c35a2801d730bcf0bf6c0a510844967fdcdf6186`, tree
`a133da4e53760031e4316f73afdfbd5ab8427cc1`.

Independent review's exact hypothesis reproduces with an actual tiny generated
wheel. Both resolution binding fields are absent, the CLI flag points to a file
containing `null`, and frozen code converts it to Python `None`. `resolution_target`
then selects its intentional absent-context legacy mode. The frozen CLI exits 0,
invokes local pip and creates package staging plus the installed RECORD proof.
The new boundary regression also catches this case reaching pip before the fix.
The separate Rust-bound flow already retains approval/provenance; this defect is
the standalone CLI's interpretation of explicitly supplied data.

The successor checks that supplied JSON decodes to an object before calling
`resolution_target`. Only omission of the flag leaves the local observation as
`None`. Existing complete observation/target/binding validation still owns object
contents. The exact actual-CLI reproduction now exits 3 with the existing bounded
diagnostic and creates neither package nor output staging. Generated package code
is not imported; these controls do not install real Torch or a provider/backend.

## Boundary controls and evidence

- Two new regression methods exercise both absent-binding and bound packets:
  nine data values in each mode (null, empty/truncated JSON, empty object, list,
  Boolean, number, string and unsupported schema), plus missing files in both
  modes. Each asserts refusal, no pip invocation and no package/output staging.
- Existing actual CLI controls still prove valid legacy installation with the flag
  omitted and valid approved native/selected-venv consumption with local report and
  RECORD evidence. Existing changed/missing/bound/unsupported/interpreter controls
  remain passing.
- Combined Python run: **54 pass** — 13 target, 10 observation, 18 existing
  consumer and 13 finite-catalog tests.
- Rust materialization/packet/shared-handoff suite: **25 pass**, including valid
  legacy/bound receipt publication and settlement, explicit-context refusals,
  retained custody/no-receipt, provider identity, cancellation/abandonment and
  existing finite/cold-replay controls.
- Dependency-feature graphs: **33 pass before Cargo**, including supported
  platform S3/headless and ONNX no-download contracts. Cargo runs offline/locked,
  serially under the shared admission lock, one job, debug information/incremental
  disabled and the existing isolated /tmp target. All commands joined.
- Ruff, two Python syntax checks and scoped source/document diff checks pass.

[Frozen actual reproduction](torch-null-approval-2026-10-06/frozen-reproduction.log),
[pre-guard regression](torch-null-approval-2026-10-06/before-guard.log),
[fixed actual reproduction](torch-null-approval-2026-10-06/fixed-reproduction.log),
[Python results](torch-null-approval-2026-10-06/python-final.log),
[shared handoff results](torch-null-approval-2026-10-06/rust-handoff.log) and
[exact commands/source/log hashes](torch-null-approval-2026-10-06/provenance.json)
retain the supporting evidence. The frozen and pre-guard failures are deliberate
reproduction evidence; final suites have zero failures.

## Preserved boundaries and next work

The [target contract](../../../contracts/wheel-target.md) clarifies CLI absence
semantics without changing an API/schema. Frozen `64d62cdc`, its WOW64/debug
repairs, selected-consumer/provider identity separation and packet/custody source
remain intact. The target owner, Rust source, native cleanup/importer/watcher/S3,
resolver/catalog/finite recipe, dependencies/locks, main and frozen candidates are
unchanged. The worker creates no PR/merge or external reviewer request. No real
account credential, paid service, security/auth/network setting or old seed is
used. Same selected executor; no applicable filesystem AGENTS/skills were found.

No catalog-authority decision blocks this parser repair. Next Q2 work remains
separately admitted production observation-producer/accepted-resolution
integration and the parent's approved upstream catalog universe, complete bounded
enumeration/byte/time budgets and no-fallback disposition. Production resolver
adoption, catalog authority, P1 and AQ gates remain unchanged/open. Actual provider,
installed/native-platform, real Torch and enforced-network-denial acceptance are
not established by these local fixtures.
