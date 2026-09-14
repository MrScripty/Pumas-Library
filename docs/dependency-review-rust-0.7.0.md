# Rust dependency findings for 0.7.0

Current dependency-minimization results and macOS follow-up are tracked in
[the minimization report](release-evidence/0.7.0/dependency-minimization.md).

Earlier follow-up: `der` was updated precisely to 0.8.2; release RPC/ONNX
build and optimized RPC tests passed. [Final Cargo audit](release-evidence/0.7.0/cargo-audit-final.json)
reports no vulnerabilities or yanks and retains four maintenance notices.
The research below records the pre-update assessment.

Reviewed on 2026-09-14 against upstream sources and the current local lockfile.
This review changes no dependency versions and does not approve publication.

## Yanked `der 0.8.0`

The yank reason is established: RustCrypto says the release was yanked to repair
its minimal-version CI check. The changelog does not attribute the yank to a
security incident. Its subsequent releases also improve parsing: 0.8.1 limits
nesting to 64 messages, and 0.8.2 rejects trailing data within nested messages.
[RustCrypto changelog](https://github.com/RustCrypto/formats/blob/master/der/CHANGELOG.md)

The crates.io API reports 0.8.2 as non-yanked with Rust 1.85 as its minimum compiler
version. The repository pins Rust 1.92.0. `ureq 3.3.0` requests `der = "0.8.0"`,
which permits the 0.8.2 patch release. These facts make **a precise update to
0.8.2 the recommended disposition**, rather than retaining a yank exception.
[Registry metadata](https://crates.io/api/v1/crates/der),
[ureq 3.3.0 manifest](https://github.com/algesten/ureq/blob/3.3.0/Cargo.toml)

Locally, `cargo tree --locked --offline -i der` resolves only through
`ureq -> ort-sys` build dependencies. This finding concerns the ONNX build
dependency graph, not a demonstrated DER parser in the shipped RPC executable.
Compatibility is inferred from the declared version range and compiler floor;
this research has not rebuilt with 0.8.2. Apply the precise update, inspect the
lockfile delta, rerun Cargo audit, and rebuild the default RPC/ONNX path before
calling it verified.

## Maintenance warnings

These four RustSec entries are informational unmaintained-package notices. They
do not identify a specific exploitable flaw or a patched version. The recommended
0.7.0 disposition is to retain the tested versions with the follow-up recorded
below, without suppressing the warnings globally. This recommendation is a
release engineering assessment, not a claim that abandoned code cannot contain
undiscovered defects.

| Locked dependency | Verified local scope | Assessment and follow-up |
| --- | --- | --- |
| `instant 0.1.13` | `notify-types 1.0.1 -> notify 7.0.0 -> pumas-library` | Native targets select the implementation whose `Instant` is an alias for `std::time::Instant`; `notify-types` uses that type for debounced-event timestamps. Lower urgency for the native desktop release. Move to a maintained `notify` dependency graph in a separately verified update; RustSec recommends `web-time` for direct consumers. [RUSTSEC-2024-0384](https://rustsec.org/advisories/RUSTSEC-2024-0384.html) |
| `paste 1.0.15` | Compile-time procedural macro through tokenizers/macros; also UniFFI tooling in workspace builds | Retain for this release; migrate through tokenizer/UniFFI upstream updates. RustSec lists `pastey` and `with_builtin_macros` as alternatives, but a transitive replacement needs compatibility testing. [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html) |
| `proc-macro-error2 2.0.1` | `getset 0.1.6` procedural macro through `lnk 0.6.3` | Build-time diagnostic code. Track migration in `getset`/`lnk`. The original package and this fork are both unmaintained; switching back to the original is not a remedy. RustSec recommends considering `manyhow` or `proc-macro2-diagnostics`. [RUSTSEC-2026-0173](https://github.com/RustSec/advisory-db/blob/main/crates/proc-macro-error2/RUSTSEC-2026-0173.md) |
| `bincode 1.3.3` | `uniffi_macros 0.28.3` in workspace builds; absent from the standalone default `pumas-rpc` graph | Retain for the desktop release and review when upgrading binding tooling. RustSec reports development ended and the maintainers consider 1.3.3 complete. Alternative serialization formats are not automatic protocol-compatible replacements. [RUSTSEC-2025-0141](https://rustsec.org/advisories/RUSTSEC-2025-0141.html) |

Build-time dependencies still execute on build machines and can affect generated
code; the scope distinction is not a supply-chain exemption.

## Evidence and limits

Dependency paths were rechecked with locked, offline inverse `cargo tree` queries
for all five packages. A `cargo tree -p pumas-rpc --locked --offline -i bincode`
query reports no matching package. The cached registry source for
`instant-0.1.13/src/{lib,native}.rs` and
`notify-types-1.0.1/src/debouncer_full.rs` establishes the native timestamp path;
it does not make the same assessment for future WebAssembly deployments.

The recorded post-preparation audit (`/tmp/pumas-070-cargo-audit-updated.json`)
contains zero vulnerabilities, the four maintenance warnings above, and the
`der 0.8.0` yank. This is a recorded result, not a fresh audit of a subsequently
modified lockfile. No lockfile update, build, target-specific runtime test, or
audit suppression was performed by this research task.
