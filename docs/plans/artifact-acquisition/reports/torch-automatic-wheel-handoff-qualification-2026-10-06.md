# Automatic Torch resolution and verified payload handoff — 2026-10-06

Automatic build/Python candidate selection now accepts an original version1 pip
resolution before acquiring or installing final wheels. Explicit resolver-only
mode installs no packages. Both automatic and retained unqualified-preview
callers use the same shared acquisition, exact local-pip, proof and publication
path. Candidate fallback ends when the validated packet is returned; subsequent
acquisition, installation, probe and publication failures propagate.

| Scope | Commit | Tree |
| --- | --- | --- |
| Frozen original handoff | `7a3264ac8d04383bd01c8ab71b7287a48c66f2db` | `b6d37961c54d6e5fe0e894e5223c08f1213d804b` |
| Accepted main | `7c229e92726e1af7447d37e3dc03fd4bd2ccfffa` | `508caecefaf7825a357670c6a05357653bb0f4ef` |
| Frozen pip repair | `99a55b78be42e3238c09b32576b0950ae5b21f21` | `eceaba08b9f38f7d59c52d2a7b90a61802fef7d6` |
| Feature normal merge base | `2b380b59fdc2e2896b4d3fbde53de27296076c64` | `eceaba08b9f38f7d59c52d2a7b90a61802fef7d6` |
| Tested/pushed source | `5946c4738870bdcfc613430bb98fd873d9e6d6f3` | `295eb32167dc49ced598d1082f6f3bf086ce4679` |

Branch `feat/torch-automatic-wheel-handoff-7a3264ac` has merge parents af6113c69a
and frozen repair99a55b78. Repair branch, original handoff evidence, main and
native/S3 frozen write sets remain unchanged. Source author and committer are
MrScripty <TheEnvironmentGuy@protonmail.com>. The evidence-only successor retains
source blobs and reports its exact final head/tree at handoff.

The public Python CLI adds `--resolve-only`, using supported public pip
`install --dry-run --ignore-installed --report` with binary-only resolution.
It rejects install/target/progress/discovery/interpreter-selection conflicts,
previous output evidence, nonobject/unsupported-version reports, and disables
ambient pip configuration only in its child. Legacy `--install` and ordinary
exact-wheel preview requirements retain their boundaries. Sources:
[pip install](https://pip.pypa.io/en/stable/cli/pip_install/),
[report](https://pip.pypa.io/en/stable/reference/installation-report/) and
[configuration](https://pip.pypa.io/en/stable/topics/configuration/).

The private prepared packet contains the validated resolution/artifacts,
unchanged original report/requirements/resolution bytes, owned staged runtime,
original managed-provider executable hash and actual Python publication label.
Automatic reports retain the resolver's actual staged-venv executable path;
preview reports retain their original selected managed path. No fabricated
preview object or rewritten interpreter identity. The managed-provider hash is
rechecked after automatic resolution. Existing conclusive exit2/4 fallback and
candidate order remain unchanged. Publication now records the actual selected
managed-Python label for automatic installs. No Rust/RPC DTO or persisted-format
change; direct automatic installation requires the existing shared acquisition
capability, as the retained-preview path already did.

The shared handoff preserves reserved sibling input roots, exact selected SHA
verification, closure/METADATA/tag validation before local pip, no index/dependency
retrieval in final installation, RECORD proof, cached proof/provenance/ancestry
checks after probe, acquired-use/child leases and durable receipt settlement.
Transport, attempt/deadline policy, receipt format and qualified Nunchaku pins
remain unchanged. Qualified bundled recipes and managed-Python bootstrap retain
scoped existing tool responsibility; metadata/candidate resolution may fetch
wheel bytes and is separately recorded, not claimed to use shared acquisition.
No core/native/S3/dependency/runtime-pin/credential or ORT build-policy changes.
The only RPC test edit gates an existing inference-only helper under its matching
feature; its existing caller is already gated there. No lint is suppressed.

Qualification on Linux x86_64 with Rust1.92.0, Python3.12.14, pip26.2.1 and
existing scoped Ruff0.15.2:

| Check | Result |
| --- | --- |
| New resolver controls before implementation | 62 tests;3 failures +1 error |
| Initial resolver implementation | 62 pass |
| Final full Python suite | 216 pass |
| Final full app-manager suite | 304 pass;1 existing ignored unit +1 ignored doctest |
| Shared custody/proof and packet/candidate controls | 13 existing +3 new pass within full suite |
| Strict default app-manager/RPC all-targets Clippy | Pass |
| Strict headless RPC integration target | Pass |
| Strict headless RPC production binary | Pass |
| Strict headless RPC all-targets | 11 baseline generated-contract errors unchanged |
| Rust fmt; complete torch-server Ruff; changed-file format | Pass |
| Two independent final-source read-only reviews | ACK; no actionable finding |

The resolver transport is mocked for deterministic accepted-report controls;
those controls verify one dry-run invocation, no installed manifest/target,
original interpreter/report preservation, child config isolation and refusal
before accepted outputs. Packet controls refuse version/venv/hash-lock drift;
candidate-engine controls exercise preserved order, first-acceptance exit and
fatal-error propagation. The existing13 shared controls run actual synthetic
wheels through real HTTP/public pip and the existing publisher seam, including
receipt identity, missing/changed inputs, postprobe/provenance/ancestor changes,
failed/lost-ack publication, acquired-input child cancellation/abandonment and
cold refusal. They do not run a complete automatic managed-provider/Torch probe.
Final-install failure cannot enter fallback by the production call boundary;
source review supports that composition, not a claimed real-provider experiment.

The first full Rust run had303 pass,1 failure and1 ignored. The unchanged
admission/cancellation test also failed alone: automatic capability refusal
preceded cancellation already recorded under tracker contention. The correction
checks cancellation before capability validation/provider work. The same test
then passed, followed by the full304-pass suite. An earlier isolated invocation
used an incomplete exact filter and ran0 tests; it is retained as an invalid
qualification attempt. The initial check also exposed a newly unused legacy
validate/move wrapper, now compiled only for its original tests. All original
failed evidence remains in the archive; no tests were relaxed. Later report
shape controls ensure null/array/string reports receive bounded exit3.

On pristine accepted main7c229e9, full headless Clippy had11 generated-contract
errors and focused integration Clippy found the unused helper. The narrow helper
gate repairs the focused target. Final full-headless errors match the baseline
messages exactly; generated code was not edited or silenced. Full headless
all-target readiness remains blocked by those existing errors.

The [evidence index](torch-automatic-wheel-handoff-2026-10-06/evidence.json) binds
all33 [raw log archive](torch-automatic-wheel-handoff-2026-10-06/logs.tar.gz) members
by size/SHA-256. Archive: 21234 bytes, SHA-256
`9f4f296f4f5aed27734f0b65657be4d381f0754f220e5765dca4de852506893a`. Own temporary dependency cache is restored; all41,202
modes/paths/content/links match digest
`7b37fb0447757adef9ab516e10d683403128f6c6843f836410e4518fb9be908f`.
Only own joined reproducible unit binary/core rlib were evicted under the shared
Cargo lock, with byte sizes/hashes preserved. Source/evidence/release archives
and frozen refs remain untouched. No provider resources, real credentials,
build-time ORT download, network/security bypass or external review contact.

This completes the bounded automatic-resolution source feature, not AQ-PACKAGES.
Enforced network denial, actual automatic-provider/Torch/inference and supported
platform/hosted acceptance remain open; whole-venv integrity is outside the
selected-member proof. The earlier ordinary denial probe was blocked by host
namespace setup; no settings were changed or bypass attempted. Prepared-use
child cancellation controls do not qualify the separate native early-transfer
withdrawal gap owned by another worker; no shared/native cleanup interface was
changed here. Parent owns PRs/reviews/CI/merges. Next existing Q2 slice is the
remaining qualified bundled-recipe payload handoff/disposition and package
acceptance, including actual network-denied/provider evidence.
