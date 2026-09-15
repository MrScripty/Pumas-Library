# 0.7.0 CI repair

## Follow-up from run 34914326630

[The next tag run](https://github.com/MrScripty/Pumas-Library/actions/runs/34914326630)
passed frontend conformance and the IPC restart tests, then exposed two
package-facts selection failures. Execution descriptors chose the largest file
in the directory without respecting `selected_artifact_files`. The fixture's
equally sized GGUF files made the result depend on filesystem traversal order.
Making the unselected sibling larger reproduced both failures locally.

Execution descriptors now restrict file selection to the declared artifact
files. Legacy packages without a selection still use the largest model file,
with a stable path tie-break. Tests cover a larger unselected sibling and a
missing selected file, which must never silently switch to another artifact.

The same run showed native compiler and action deprecation warnings. Linux-only
router observers, mutation workers, and their test helpers now have matching
platform compilation guards. Windows-only unused parameters are explicitly
handled. Shared atomic-publication result types retain their complete contract;
their non-Unix unused-code annotations explain that this platform refuses
publication admission. These annotations do not claim Windows publication works.

The workflow explicitly installs the repository Rust toolchain and uses maintained
Node 24 actions (`pnpm/action-setup@v6.1.0`, `actions/upload-artifact@v7.0.1`, and
`actions/download-artifact@v8.0.1`). Native platform checks reject compiler warnings
before optimized compilation. The artifact archive layout remains unchanged.

Follow-up validation: the complete local Rust quality and headless suites passed,
as did all 36 package-facts tests in debug and release builds. Candidate run
[34915702501](https://github.com/MrScripty/Pumas-Library/actions/runs/34915702501)
passed Rust quality, frontend contracts, and Linux/macOS compiler-warning checks.
Its Windows warning check exposed unused publication fault-injection helpers:
these tests now compile only on Unix, where durable publication is supported.
A non-Unix regression verifies publication refusal leaves existing bytes and the
parent directory unchanged. RPC shutdown timeouts include captured diagnostics
on every platform. Native CI verification continues before moving the release tag.

## First repair

Investigated [Build run 34904132053](https://github.com/MrScripty/Pumas-Library/actions/runs/34904132053)
at commit `3a6e0309b8cabe891d0d4792d8b9704fc394773e`.

## Failures and corrections

- **Frontend and desktop contracts:** the renderer conformance suite reads the
  bundled Electron preload, but the job only type-checked Electron. Removing the
  local generated preload reproduced the CI `ENOENT`. The frontend contract test
  command now builds Electron before generating fixtures and running Vitest.
- **Rust quality, Linux release, and headless tests:** the IPC restart fixture
  could race startup reconciliation of retained declarations. Its metadata family
  disagreed with its directory and lacked classification evidence, so reconciliation
  could rewrite metadata or move the model after facts were cached. The fixture now
  supplies coherent identity and text-generation evidence, and completes required
  reconciliation before generating package facts. Adding that barrier to the old
  fixture deterministically exposed `ModelNotFound`; correcting the fixture made
  the existing native/IPC parity and durable-restart assertions pass.
- **macOS release:** held model relocation returned `Unsupported` outside Linux,
  causing migration, reclassification, merge, and preparation tests to fail.
  macOS now uses `renameatx_np` with `RENAME_EXCL`, preserving directory-relative
  authority, exclusive destination creation, identity checks, and directory sync.
  See [Apple's syscall contract](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man2/rename.2).
  Relocation and substituted-parent tests now run on both supported platforms.
  A syscall regression also checks that a destination created after preflight
  cannot be overwritten, including an empty directory that ordinary rename could
  replace.

The attribution inventory was regenerated after the frontend script change.
Only the frontend manifest hash changed; dependency versions and notice texts
remain identical.

## Verification scope

- Frontend desktop conformance: 48 tests passed after reproducing the missing preload.
- IPC integration: both tests passed; the existing subprocess helper remains ignored.
- IPC durable restart: 10 repeated passes using disk temporary storage and 10 using
  memory-backed temporary storage, which exposed the original startup race locally.
- Dependency ownership, version/tag alignment, attribution, and release-contract
  checks passed.
- The complete `./scripts/rust/check.sh` gate passed: formatting, all-feature
  check and Clippy, default workspace tests, doctests, and no-default check.
  This includes 1,409 active core unit tests and 227 RPC unit tests.
- Headless core tests passed, including 1,385 active unit tests, integration
  tests, and doctests. Three download timing tests initially exceeded deadlines
  during concurrent compilation; all three passed in isolation, and the full
  headless core rerun passed with `RUST_TEST_THREADS=4`.
- The independent headless RPC suite also passed with `RUST_TEST_THREADS=4`.
- The optimized workspace test command from the Linux release job passed:
  `cargo test --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler --release`.

Native macOS execution and installer acceptance require the new GitHub Actions
run. Local Linux tests do not establish macOS execution or installer acceptance.
The maintainer explicitly requested moving the failed `v0.7.0` tag; no GitHub
release existed for that tag when checked before the repair.
