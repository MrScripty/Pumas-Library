# Dependency, attribution and installer follow-up

Completed locally on 2026-09-14. The exact Linux artifacts and metadata hashes
are recorded in [final-linux-candidate.json](final-linux-candidate.json).
Candidate files are in `/tmp/pumas-070-final-linux/`; nothing was pushed, tagged,
or published and no remote workflow was dispatched.

## Dependency follow-up

Only `der` changed in Cargo.lock: 0.8.0 to 0.8.2, plus its checksum. The default
optimized RPC/ONNX build and optimized RPC suites passed. The
[final Cargo audit](cargo-audit-final.json) contains no vulnerabilities or yanked
versions. The four reviewed unmaintained-dependency notices remain tracked in
[the Rust review](../../dependency-review-rust-0.7.0.md), without blanket suppression.

## Attribution and metadata

The [attribution report](../../release-attribution/0.7.0/README.md) describes the
401 package notice entries, exact upstream sources, source-availability links,
SQLite dedication, preserved Electron/Chromium notices and ONNX/Pyke notices.
The exact ONNX archive's upstream signature/provenance was verified; its embedded
source commit matches the versioned notice source.

Both installers embed the current combined notice and project license. Packaging
and CI now reject stale attribution inputs or missing/altered notice material.
Negative checks for dependency drift, altered upstream text and missing notices
passed. Python lint/format checks and Actionlint also passed.

Both per-installer SPDX 2.3 inventories passed the official JSON schema's
structural validation (format annotations were disabled in the validator).
They describe exact extracted application files and a conservative dependency
attribution closure; vendor notices retain native transitive attribution without
inventing individually resolved native component versions. The local provenance
statement is explicitly unsigned/self-reported and claims no SLSA level.
Checksums cover both installers, both SPDX files, the combined release notice and
provenance statement. CI builds must regenerate their own metadata.

## Linux installer evidence

| Claim | Result and evidence |
| --- | --- |
| Exact AppImage/Deb extraction, embedded notices and build-input hashes | Passed: [resource checks](linux-installer-resources.txt) |
| Packaged RPC health in both formats | Passed in the same resource checks |
| Real ONNX import, load, 256-dimensional finite gateway embeddings and unload | Passed: [AppImage](appimage-onnx.txt), [Deb](debian-onnx.txt) |
| Renderer readiness, preload-to-RPC access and category-menu interaction | Passed: [AppImage](appimage-desktop.txt), [Deb](debian-desktop.txt) |
| Window close, exit code zero and backend port shutdown | Passed in both desktop records |
| Desktop readiness, menu interaction and shutdown with the default Electron sandbox | Passed: [AppImage](appimage-default-sandbox.txt), [Deb](debian-default-sandbox.txt); GPU rendering disabled |
| Corrupt root preference, native directory selection, durable replacement and restart into ready state | Passed: [AppImage](appimage-root-recovery.txt), [Deb](debian-root-recovery.txt) |
| Debian install, maintainer scripts/triggers, alternatives registration, version identity and removal | Passed: [isolated installation](debian-install-remove.txt) |

The ONNX fixture was copied into a temporary library and was never modified in
place. The Debian check used user/mount/network namespaces, copies of package
state and the required writable configuration directories, with the host root
read-only. It exercised real dpkg installation/removal and maintainer scripts,
not just archive extraction. Live host AppArmor policy loading was outside that
namespace's acceptance claim.

Initial chooser automation was sensitive to focus and GTK's distinction between
opening a directory and accepting it. Real desktop selection and restart passed;
the separate virtual-display portal backend was unavailable. These results do
not qualify every desktop portal implementation. The recorded recovery path
used the real native chooser, not a substituted dialog or direct preference edit.
The corrupt preference was the input fixture; the application wrote its repair.

## Reproduce

- `python3 scripts/release/smoke-linux-packages.py CANDIDATE_DIRECTORY`
- `python3 scripts/release/verify-packaged-onnx.py PACKAGED_RPC NOMIC_FIXTURE_ROOT`
- `python3 scripts/release/verify-deb-install.py CANDIDATE_DEB`
- `python3 scripts/release/write-linux-metadata.py CANDIDATE_DIRECTORY EXTRACTED_DIRECTORY`
- `sha256sum -c checksums-sha256.txt` from the candidate directory.

For desktop evidence, launch each extracted executable with isolated
`XDG_CONFIG_HOME` and a temporary library, confirm the category filter and library
view, and close the window. Confirm both the desktop and its backend terminate.
For recovery, put malformed JSON in the isolated application's
`launcher-root.json`, start without an explicit library override, select a valid
root in the native chooser and confirm the restarted app is ready and the new
preference survives. Model payloads and personal libraries are not test fixtures.

## Remaining release boundaries

GitHub CI has not run this candidate. Native macOS and Windows installation and
behavior remain unverified; Windows remains best effort and its known non-Unix
mutation/publication limitations are unchanged. The Apple bridge upstream
licensing statement retains its SDK-related caveat for native platform review.
No Torch GPU runtime distribution or host-binding artifact is newly qualified.
These local results are not authorization to publish unverified target artifacts.
