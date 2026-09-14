# 0.7.0 release attribution

`THIRD-PARTY-NOTICES.txt` is generated from exact resolved package license and
notice files by `python3 scripts/release/generate-notices.py`. It is embedded as
`resources/THIRD-PARTY-NOTICES.txt` and copied to the release directory as
`THIRD-PARTY-NOTICES-0.7.0.txt`. `inventory.json` records input and source-text
hashes. `check-attribution.cjs` runs in CI and before Electron packaging; stale
inputs, altered texts, and missing notices refuse packaging.

The inventory includes the normal/build Rust dependency closure for the three
desktop targets and the JavaScript production dependency closure. It is a
conservative attribution superset: build-only, target-specific and tree-shaken
code is identified as such, rather than asserted to be inside every executable.
All bundled license/notice texts found in those packages are retained, including
nested native-library notices. SQLite's source dedication is included separately.
The unmodified MPL-2.0 `option-ext` source is available at its exact crates.io
source archive URL in the notice file.

Published archives for binrw, governor and several Apple bridge crates omit
license files. Their upstream licensing statements/texts were collected at the
revisions recorded in `.cargo_vcs_info.json`. The fetched sources and SHA-256
values are in `scripts/release/licenses/sources.json`. The Apple bridge upstream
statement explicitly discusses its SDK-derived material; this report preserves
that statement and does not independently resolve Apple's SDK terms. Native macOS
release acceptance still needs the platform owner's review and execution evidence.

## Native runtime provenance

The exact Linux CPU ONNX archive SHA-256 is
`acc1cba79c337594ead1d88ca72516147aa60054c84217b53399a31caa5ba671`.
GitHub CLI verified its upstream Sigstore attestation against `pykeio/ort-artifacts`.
The recorded result is
[onnx-archive-verification.json](../../release-evidence/0.7.0/onnx-archive-verification.json).

The attested build recipe is commit
`f267947bba782f315bd667b9b2a607ab51363445`, workflow invocation
[22202674684](https://github.com/pykeio/ort-artifacts/actions/runs/22202674684).
Its Linux CPU job builds the upstream ONNX dependency tree and bundles static
link dependencies. The archive's embedded build info names ONNX commit `058787c`;
GitHub resolves it to `058787ceead760166e3c50a0a4cba8a833a6f53f`, exactly tag
`v1.24.2`. The complete upstream ONNX license and third-party notice collection
at that tag are included, together with the Pyke recipe/patch license. These are
notice supersets, not inferred license text reconstructed from binary symbols.

Electron's own `LICENSE.electron.txt` and `LICENSES.chromium.html` remain beside
the executable and are required parts of attribution. They are included in the
exact extracted-file SPDX inventories. The Pumas project license remains under
`resources/LICENSE.txt`. The combined notice supplements these files.

## Final artifact metadata

`write-linux-metadata.py CANDIDATE_DIRECTORY EXTRACTED_DIRECTORY` writes one
SPDX 2.3 inventory per Linux installer, an in-toto/SLSA-format local provenance
statement, the release notice, and SHA-256 checksums over all other final files.
The extracted directory contains `appimage/squashfs-root` and
`deb/opt/Pumas Library` from those exact installers. Attribution input and embedded
notice hashes must match before metadata is generated.

SPDX records exact extracted application files plus the conservative package
attribution inventory. It does not claim every build dependency is a runtime
component, nor invent individual versions for native components represented only
in vendor notices. Symlinks are excluded from file verification codes; the final
installer SHA-256 covers the complete archive, including symlinks and metadata.

Local provenance is explicitly **unsigned and self-reported**. It records the
source revision, working diff hash and material input hashes; it is not a signed
GitHub build attestation and claims no SLSA level. A release built by CI must
regenerate its metadata from that run's actual outputs and provenance.

Verbatim legal text is exempt from whitespace rewriting in local hooks and Git's
whitespace checks. Hash validation still checks its integrity. Other source,
JSON, workflow and private-key checks remain enabled.
