# Current locked dependency and notice qualification

Candidate: `df53db08e2ba224edfaf713522ddd410f7d4ac44`, version 0.8.0-rc.1.
Inspection date: 2026-10-14. The cleanup-fixture repair changes no dependency
manifest, lockfile or notice input. This review concerns the current closure,
not a claim that a clean scanner proves the complete artifact secure.

## Cargo

`cargo-audit 0.22.2 audit --file rust/Cargo.lock --db
/tmp/pumas-v080-advisory-db --json` exited 0. Official advisory database commit
`6aed84668c23d7f535077a9fe918bf1fa0f6a5bc`, updated 2026-10-13, contains
1,310 advisories; the lockfile contains 514 packages. The
[raw result](cargo-audit.json) reports **zero vulnerability entries**, three
maintenance notices and one unsoundness notice. No ignore/suppression was used.

| Notice | Actual locked consumer and disposition |
| --- | --- |
| `bincode 1.3.3`, RUSTSEC-2025-0141 | UniFFI macro/build tooling; existing maintenance follow-up, not a new security exploit finding. Host bindings still have no accepted release tuple. |
| `instant 0.1.13`, RUSTSEC-2024-0384 | notify-types → notify → core; retain the maintenance follow-up with the filesystem-watcher owner. |
| `paste 1.0.15`, RUSTSEC-2024-0436 | Macro dependency through tokenizers and UniFFI tooling; retain existing maintenance tracking. |
| `lru 0.16.4`, [RUSTSEC-2026-0253](https://rustsec.org/advisories/RUSTSEC-2026-0253.html) | Present in the S3 shipping closure only through aws-sdk-s3 1.137.0. The advisory's specific panic-safety trigger is absent from the inspected consumer; see below. |

The lru advisory concerns `pop()`: a stored key's destructor panics before the
node is detached, then a later cache operation follows a dangling pointer. It
requires unwinding/catching that panic and a potentially panicking key destructor.
The locked SDK's sole LruCache owner is `S3ExpressIdentityCache`, whose private
`CacheKey(String)` has derived traits and no custom Drop. Its operational access
is `get_or_insert_mut`; it exposes no caller-supplied key implementation or `pop`
call. A plain String key cannot supply the required panicking custom destructor.
This source observation disposes the current trigger; it does not repair or
declare the upstream generic container sound.

Inspected official crate member:
`aws-sdk-s3-1.137.0/src/s3_express.rs`, SHA-256
`033da02f9bc2ae4f74fbd5393503cf770fd5a19b53c3565af2a72c862e29749d`.
`cargo tree --locked --offline --manifest-path rust/Cargo.toml -p pumas-rpc
--no-default-features --features s3 -i lru --edges normal,build --prefix none`
establishes the actual S3 path. Keep the raw warning visible. The S3 dependency
owner must revisit on SDK/key/cache use changes or an accepted compatible upstream
upgrade; no forced incompatible lru override is part of this release slice.

## JavaScript

Fresh pnpm 10.33.0 audits used the unchanged frozen lockfile:

- [Production audit](pnpm-audit-production.json): zero advisories, exit 0.
- [All-dependency audit](pnpm-audit-all.json): one moderate advisory, exit 1,
  [GHSA-hp3w-g68c-fv3c](https://github.com/advisories/GHSA-hp3w-g68c-fv3c),
  sprintf-js 1.1.3; no patched version reported.

The current path remains `electron-builder → app-builder-lib → @electron/get →
global-agent → roarr → sprintf-js`. This is build tooling, absent from the
production audit closure. The existing [dependency decision](../../../dependency-review-0.7.0.md)
retains that risk pending a compatible parent/upstream removal. This continuation
adds no suppression and no major dependency replacement. GitHub's generic
52-alert push notice is not substituted for these exact-lock results or treated
as 52 confirmed shipped vulnerabilities.

## Notices and license inventory

Frozen pnpm installation used `--ignore-scripts` and task-owned data/cache/store
paths. The first default-home attempt failed before installation; no home access
or lockfile rewrite was used. `cargo fetch --locked` supplied missing target crate
sources before the existing generator could run offline.

Regenerated into disposable output directories using
`python3 scripts/release/generate-notices.py --output DIR` and its `--features s3`
variant. Both notice text and inventory are **byte-identical** to the checked-in
versioned collections:

| Collection | Entries | Notice SHA-256 | Inventory SHA-256 |
| --- | --- | --- | --- |
| Default | 378 | `5e5e273c6d4a24563d91de8d9d349413481da992d714438d80e884d61eb13dfc` | `ed9af5ce215f4d1d7a4f868bcd00e1d2ca14945addbe759911ee2cbddcc28d60` |
| S3 | 428 | `abf171b832a35b70752979877aaab3b9ad3fdbf2e9cb2a034e2bce3a832134df` | `a4ff1463884e0e891143db7e3868afdcacce4569da97796ae2bb3fd9bf499501` |

This checks authoritative collected text, closure and freshness under the existing
license policy. Final per-artifact inclusion, native dependency coverage, SBOM,
provenance and final-byte checksums remain artifact-specific release gates.
