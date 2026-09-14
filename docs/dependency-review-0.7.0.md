# Dependency review for 0.7.0

The dependency review below records the Electron 39 baseline from 2026-09-14.
**Electron has since been upgraded to 43.7.0 and reverified on Linux.** The old
`extract-zip` dependency is absent from the new lockfile. Fresh pnpm audit reports
no advisories against actual installed dependency versions; its workspace-importer
false positives remain visible. See [upgrade evidence](release-evidence/0.7.0/electron-upgrade.json).

All 172 desktop tests pass, including the real sandboxed preload oracle, along
with type/lint, contract, launcher, and extracted Linux installer checks. CI now
installs Electron explicitly with a timeout before native tests and launcher
smoke. Skipped native tests no longer trigger a download during test discovery.
Native macOS/Windows validation remains outstanding.

The remaining dependency work is the precise `der` update, complete third-party
attribution, and final per-installer SBOMs. The baseline audit and inventory files
below remain historical evidence for the older installers, not the new cohort.

## Electron 39 baseline: support and the ZIP extractor

The candidate uses Electron 39.8.10. Electron 39 reached end of life on
2026-05-05; supported majors on this review date are 42, 43, and 44. The earlier
patch-only upgrade did not resolve this support problem.
[Electron support schedule](https://releases.electronjs.org/schedule),
[support policy](https://www.electronjs.org/docs/latest/tutorial/electron-timelines).

**Completed fix: upgraded to Electron 43.7.0 and requalified locally.** This is
a currently supported release, published September 10, with support scheduled
through January 5, 2027. It uses the newer Electron-maintained native ZIP extractor
in its installation script. This also removes the specific legacy extractor path
in the current Electron dependency tree, subject to checking the resolved new
lockfile. It is not an assertion that every dependency of the new version is free
of advisories. [43.7.0 release](https://releases.electronjs.org/release/v43.7.0),
[installation source](https://raw.githubusercontent.com/electron/electron/v43.7.0/npm/install.js).

The current chain `electron@39.8.10 -> extract-zip@2.0.1` has two high-severity
advisories with no patched release of that old package:

- [GHSA-jmr9-qjv8-65gv](https://github.com/advisories/GHSA-jmr9-qjv8-65gv): unvalidated archive symlinks can point outside extraction destinations.
- [GHSA-7pqw-9j4j-h8q3](https://github.com/advisories/GHSA-7pqw-9j4j-h8q3): archive symlinks can enable writes outside extraction destinations.

I reproduced the overwrite with the installed package: a ZIP containing a
symlink followed by a regular entry with the same name overwrote a sibling
sentinel. All files were inside a newly created temporary test directory. The
[result](release-evidence/0.7.0/extractor-reproduction.json) confirms that this is
an actual vulnerable dependency, not just a scanner label.

The current build has useful mitigations: Electron's installed script passes
bundled checksums to its downloader, and the locally cached Linux Electron ZIP
matches its bundled SHA-256 exactly. No Electron mirror, remote-checksum,
skip-download, or distribution-override variables were present during the review;
the checked-in workflow does not configure such overrides. This is local evidence,
not inspection of future remote runner configuration. The archive SHA-256 is
`92e8b031fa5327c78a972279fd75fc8503fcd1773401809f4557e4de583eabd1`.

`extract-zip` is absent from the extracted application's `app.asar`; its only
packaged Node dependency is `electron-log`. The old extractor is an installation
and build-machine exposure, not a demonstrated AI-model import path. These
mitigations do not justify selecting an end-of-life Electron version when a
supported migration path exists. No exception or audit suppression was added.

Fresh pnpm audit still reports the two real extractor advisories. Its other
findings use workspace importer version `0.7.0` as if it were the Electron runtime
version; the installed runtime is 39.8.10. The
[triaged results](release-evidence/0.7.0/pnpm-audit.json) retain the raw counts and
separate those importer false positives from actual installed dependencies.

## Rust findings

A fresh [Cargo audit](release-evidence/0.7.0/cargo-audit.json) reports **zero
vulnerabilities and no unsoundness warnings**, with four maintenance notices and
one yanked version.

`der 0.8.0` was yanked to fix RustCrypto's minimal-version CI, not because of a
security incident identified in the changelog. The non-yanked 0.8.2 patch fits
`ureq`'s dependency constraint and this repository's Rust compiler. Recommend a
precise update and a rebuild of the RPC/ONNX path. The active path is through the
ONNX build script, not a demonstrated DER parser in the shipped RPC executable.
The compatible-range assessment is not a completed rebuild.
[RustCrypto changelog](https://github.com/RustCrypto/formats/blob/master/der/CHANGELOG.md).

The four maintenance notices can remain for 0.7.0 with tracked upstream follow-up;
they do not report a specific exploitable flaw:

| Dependency | Checked scope and disposition |
| --- | --- |
| `instant 0.1.13` | Native timestamp alias to `std::time::Instant`, through `notify`; retain, update the notification dependency graph in a separately tested change. |
| `paste 1.0.15` | Procedural macro in tokenizer/tooling paths; retain, follow upstream migration. |
| `proc-macro-error2 2.0.1` | Compile-time diagnostics through `getset -> lnk`; retain, follow upstream migration. |
| `bincode 1.3.3` | UniFFI macro tooling, absent from standalone default RPC resolution; retain, address with binding-tooling updates. |

The [Rust source review](dependency-review-rust-0.7.0.md) contains primary-source
citations and detailed scope. Build-time code can still affect build machines and
generated output; this classification is not a blanket exemption.

## Installer attribution and dependency inventory

The [review inventory](release-evidence/0.7.0/dependency-inventory.json) records
input and installer hashes, package versions, declared licenses, and notice
presence. **It is not a final SPDX SBOM or a certification of the complete native
dependency closure.**

A separate renderer build instrumented Rollup's rendered-module inventory and
reproduced every `frontend/dist` file byte-for-byte. Its ten included package
identities are React, React DOM, scheduler, react-aria, react-stately,
framer-motion, motion-dom, motion-utils, loglevel, and lucide-react. Their installed
packages contain license text, with declared MIT, Apache-2.0, or ISC licensing.
The Electron main archive also preserves `electron-log`'s MIT license.

For the native RPC, the selected Linux normal/build dependency graph contains
342 package identities. This includes build dependencies and procedural macros,
not 342 proven runtime libraries. All have declared licenses; declarations alone
do not establish complete notices. The selected graph includes MPL-2.0
`option-ext`, so the release notice needs a source-availability reference for
that covered code. Mozilla describes this requirement for executables compiled
from unchanged MPL source. [MPL FAQ, Q8](https://www.mozilla.org/en-US/MPL/2.0/FAQ/).

Both actual Linux installers contain the project license, `LICENSE.electron.txt`,
and `LICENSES.chromium.html`. Both **lack `resources/THIRD-PARTY-NOTICES.txt`**, and
neither candidate has its required per-installer SPDX sidecar. Existing Chromium
notices are not proof of coverage for Pumas's renderer or Rust/ONNX components.

The ONNX build uses the pinned Pyke static ONNX Runtime 1.24.2 archive. Its cached
payload contains `libonnxruntime.a` without adjacent notices. The download-table
hash and version are recorded in the inventory. Microsoft publishes
[1.24.2 third-party notices](https://raw.githubusercontent.com/microsoft/onnxruntime/v1.24.2/ThirdPartyNotices.txt),
but their coverage must be reconciled with the Pyke-built native archive rather
than assumed identical. Some Rust packages also lack top-level license files
(`binrw`, `binrw_derive`, `governor`); their source notices/upstream texts need
collection. `crc-catalog` stores its license texts in `LICENSES/` instead.

## Concrete remaining work

1. Completed locally: Electron 43.7.0 installation, desktop tests, launcher smoke,
   packaging and extracted-installer verification. Native macOS/Windows checks
   still run through the release workflow.
2. Update `der` precisely to 0.8.2, inspect the lockfile delta, rebuild the RPC/ONNX
   path, and rerun audit. Retain the four maintenance warnings with the follow-up
   above; do not suppress all future warnings.
3. Generate the combined notice file from the verified renderer, Rust and native
   component inventories, including relevant source references; embed it in every
   candidate and recheck extracted contents. Preserve Electron/Chromium notices.
4. Produce per-installer SPDX files from the final artifacts, then regenerate
   checksums. Reconcile native component coverage before calling the SBOM complete.

These are release-preparation tasks, not a request for a general legal review or
an assumption that every audit warning must block publication. Native Windows
and macOS behavior remains outside this Linux review. No push, tag, or
publication was performed. The follow-up Electron upgrade and Linux installer
rebuild are recorded separately above.
