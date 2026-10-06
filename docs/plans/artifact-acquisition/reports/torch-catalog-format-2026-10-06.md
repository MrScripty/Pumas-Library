# Catalog format review repairs — 2026-10-06

Both independent findings reproduce on the unchanged frozen owner. This narrow
successor repairs required Simple JSON version data and actual wheel metadata
directory identity. Automatic/preview adoption, source grants, target/custody,
completion/fallback policy and remaining independent budget review are unchanged.

## Exact source and reproduction

Branch `fix/torch-catalog-format-4c85b60d` starts from frozen
`4c85b60d47a736c066a17e673754099f8f6e5870`, tree
`8b8fa5fc6de0ff52db04f5d6f800d44f420023e8`. Tested source is
`a30ad50bc11e4100ce19ef166e6711faa8424490`, tree
`0fb14070482d51c03e404f22194865c35ecbc810`. Main and the original owner branch
are preserved. Source changes only the Python owner/direct tests, the Rust
controlled fixture and the prior write-set admission; no Rust production owner,
dependency or frozen native/importer/watcher/S3/manifest change.

The [frozen reproduction](torch-catalog-format-2026-10-06/frozen-reproduction.log)
runs three focused tests with unchanged owner bytes: valid API 1.1–1.3 version
lists are rejected; missing mandatory lists are accepted; wrong name/version or
nested `.dist-info` metadata is accepted. Six refusal assertions fail and three
positive subtests error, as expected. [Owner hash proof](torch-catalog-format-2026-10-06/frozen-proof.json)
and the exact [reproduction fixture patch](torch-catalog-format-2026-10-06/frozen-reproduction-tests.patch)
preserve that measurement before production edits. Apply only that fixture patch
to the frozen base and run the three test names recorded in provenance to repeat.

## Format corrections

[PEP 700](https://peps.python.org/pep-0700/) and the
[Simple repository contract](https://packaging.python.org/en/latest/specifications/simple-repository-api/)
require a top-level version-string list from JSON API 1.1 onward. The repaired
finite parser recognizes `versions`, requires it for 1.1–1.3, checks list/string/
unique-string shape under the existing record bound, and checks wheel versions
against its parseable entries using public version equality. Legacy strings and
versions without files remain valid data; they are neither sorted nor converted
into a narrowed candidate universe. API 1.0 may omit the field. Required file
size evidence for 1.1+ also applies to excluded non-wheel rows. Existing stricter
unsupported-extension/source admission remains; this is not adoption of a
general live Simple client. Receiving URLs, hashes, target, budgets and no
incomplete fallback remain unchanged.

The [wheel format](https://packaging.python.org/en/latest/specifications/binary-distribution-format/)
binds the root `.dist-info` directory to distribution name/version. Inspection
now requires root-level metadata with exactly one distribution/version separator,
valid canonically matching name, normalized version spelling and public version
equality with the admitted artifact. Historical uppercase/dotted names are
accepted per the wheel contract; equivalent normalized release spellings such
as 1.0 and 1.0.0 compare semantically. Local version identity is retained.
Additional `.dist-info` roots, including those lacking METADATA/WHEEL, refuse.
Actual content identity, namespace, SHA/size/tag and selected-target checks still
run through the unchanged owned inspection/shared acquisition path.

## Fresh qualification

- **89 Python tests:** 28 owner tests plus the existing 61 target/observation/
  local-consumer/finite-catalog tests. [Raw log](torch-catalog-format-2026-10-06/python.log).
- **51 Rust controls:** conformant API 1.3 project observations exercise the
  complete shared snapshot and previous refusal/custody/handoff cases. An actual
  inert wheel with correctly bound expected hash/size but renamed
  `wrong-1.0.dist-info` refuses after five shared transfers, publishes no receipt,
  retains Using, refuses cold replay and drains cleanup.
  [Raw log](torch-catalog-format-2026-10-06/rust.log).
- **33** platform/headless/S3/ONNX no-download graphs ran before Cargo.
  [Graph log](torch-catalog-format-2026-10-06/feature-graphs.log).
- Serialized offline/locked strict [Clippy](torch-catalog-format-2026-10-06/clippy.log),
  scoped [Ruff](torch-catalog-format-2026-10-06/ruff.log),
  [Rustfmt](torch-catalog-format-2026-10-06/rustfmt.log),
  [syntax](torch-catalog-format-2026-10-06/syntax.log) and source/document
  [diff checks](torch-catalog-format-2026-10-06/diff-check.log) pass. Faithful raw
  logs/patches are excluded from whitespace normalization.

[Provenance](torch-catalog-format-2026-10-06/provenance.json) records exact source,
commands, unchanged owners and evidence hashes. No real-provider fetches,
credentials, backend/model execution, new dependencies or network bypasses.
All fixture servers/children/effects joined. No public Rust API/schema change;
valid version lists are now admitted and malformed missing-list/directory input
refuses. Independent completion/budget review and all production/provider/solver/
target/selection/caller gates remain open. Next existing Q2 work remains the
reviewed public offline solver/checked packet and coordinated caller composition.
