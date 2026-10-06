# Q2 retained-preview verified wheel handoff — 2026-10-06

The bounded retained, unqualified resolved-preview Torch path now obtains its
accepted wheel set through shared HTTP acquisition before installing exact local
inputs. The source milestone is complete and pushed; AQ-HTTP and AQ-PACKAGES remain
not ready. This is a controlled Linux fixture result, not production-ready Torch,
network-denied acceptance, or migration of every package path.

## Source and ownership

Branch `feat/torch-verified-wheel-handoff-8756f33b` retains base
`8756f33b3ba114f5bdd84dbcf25b1aa58c287868`, tree
`508caecefaf7825a357670c6a05357653bb0f4ef`. Tested source is
`e246645bcf7d2f3bae2f68a0ba29215fafe335e9`, tree
`24eceee6dc62b0c368f666c43989c34ba3b2a3bb`.
Accepted main `1c1c7875ff8fc43b3ad1957fb2a94d44c850716c`, original main
`05717338c2aea737483fb4b28c3ed4053a65de96` and PR40
`63123fd8f9f866064a8315096ab3fb1fc81e8f0f` are ancestors. PR40 retains expected tree
`163ef2442695b44555ac0e42ff79a6a88de9e817`. The old saved seed was not used.
The frozen HTTP candidate8756, AC10/PR42 evidence and installed S3 branch refs are
unchanged. The entire core acquisition/S3/native source tree, importer/watcher,
`installer.rs`, dependencies/lockfiles, generated contracts, runtime pins and
ONNX policy remain unchanged from the base.

Production changes are confined to `installer/torch.rs`, the package-owner local
helper, adjacent Torch manager composition and RPC startup. Tests live beside
the Torch owner and under torch-server/tests. The coordinator owns PRs, external
reviews, hosted CI, integration and acceptance. The worker committed and
non-force pushed the distinct feature branch as MrScripty; no reviewer was
contacted externally and no branch was merged.

## Public API and behavior

`VersionInstaller::with_torch_acquisition(Arc<AcquisitionService>) -> Result<Self>`
is an additive async construction method. Existing constructor signatures are
unchanged. `VersionManager::new_with_acquisition` now accepts Torch as well as
llama.cpp; RPC uses that existing entry point for both. Retained-preview Torch
installation requires the shared capability. No IPC/generated DTO or store
schema changes are introduced.

The owner revalidates the existing resolution, pip report version1, source trust,
interpreter identity, original hash requirements and exact preview artifacts.
Nunchaku retains the existing embedded lock's exact URL/digest, with no new pin.
It creates an immutable selected manifest, acquires every member, and hands the
complete verified use to the local installer. HTTPS-only transport disables
ambient proxies and supplies no authorization. The existing shared HTTP owner
keeps source identity, redirect handling, streaming verification and receipts.
The selected path explicitly uses the existing download-attempt count/backoff,
URL connect timeout and a one-hour per-file source-wait budget corresponding to
the existing package-child deadline. It adds no nested downloader or retry owner.

`install_verified_wheels.py` uses the existing pip-vendored packaging support and
public pip commands. It validates canonical distribution identities, wheel tags,
METADATA/Requires-Python and SHA-256, then computes marker/extras dependency
closure. Missing or contradictory dependencies, any dependency direct URL,
substituted same-name/version bytes, links and unselected files are refused.
The generated local requirements use exact file URLs with hashes; pip uses
`--no-index --no-deps --require-hashes --only-binary=:all: --no-cache-dir
--ignore-installed --no-compile` into an owned empty target. Pip's networked
update check is disabled for this local invocation. Original
resolution, pip report and requirements remain separate, unchanged provenance.
The local report must have the supported version and exact distribution,
version, file-URL and digest set. The existing RECORD/distribution verifier
produces installed-file proof before installed code executes. See the public
[pip install interface](https://pip.pypa.io/en/stable/cli/pip_install/),
[secure installs](https://pip.pypa.io/en/stable/topics/secure-installs/) and
[core metadata specification](https://packaging.python.org/en/latest/specifications/core-metadata/).

After packages move into the venv, the existing CPU/identity probe runs with
bytecode writes disabled. Before receipt issuance/publication the owner rechecks
selected installed files and cached proof/provenance bytes, including no-link
ancestry beneath the owned runtime root. The compact receipt binds the existing
acquisition manifest/use to output directory identity, accepted resolution,
interpreter fingerprint, installed-manifest digest/count and recipe digest.
This proof covers selected installed members; venv bootstrap files are separately
owned and are not claimed as an exact whole-runtime inventory.

The borrowed owner pipeline and existing shared worker exchange verified use,
proof, issued receipt and final publication acknowledgment without detaching a
new coordinator. A composite managed-child lease holds stage plus input use
through actual exit/cleanup, including an abandoned waiter. The success output
retains its input use until durable shared receipt settlement. Existing owner
rename/metadata publication and rollback remain authoritative.

Inputs occupy retained sibling directories outside the automatically cleaned
Torch stage. Failed or uncertain uses keep their inputs. Cold construction
refuses unresolved rows before ordinary Torch startup cleanup and never replays
package code or infers installation from an issued receipt. Adopted inputs also
remain retained; input reclamation is a future explicit policy.

## Exact local evidence

All **301 app-manager unit tests pass**, with one existing SIGKILL child test
ignored; its parent passes. One existing doc test is also ignored. These include
**13** new shared-handoff controls:

- Real shared HTTP acquisition of two synthetic dependent wheel ZIPs, actual local
  pip, the existing Torch publisher and Adopted receipt settlement. Last input
  release observes Adopted.
- Changed same-name/version wheel bytes refuse preparation; an absent transitive
  dependency refuses local installation/publication.
- Failure after owned rename before metadata publication, and lost final
  acknowledgment after publication, retain Using/issued receipt and refuse cold
  replay before startup cleanup.
- Cancellation and abandoned waiter after a descendant opens a verified wheel
  retain custody through child cleanup. Final input release sees Using and a
  non-running descendant; acquisition drains first in the abandonment control.
- Actual subprocess mutations of a selected member, proof JSON, or provenance,
  and relocated package/proof directories replaced by ancestor symlinks, refuse
  publication via the production final-proof predicate.
- The existing Nunchaku lock URL/hash remains accepted and altered values refuse.

These fixtures exercise the actual helper and publisher seam plus the production
final-proof predicate. Their success recipe/proof is synthetic; they do not run
real Torch inference or the complete production managed-Python/probe pipeline.
Fixture assertions follow owner/source drainage. Both shutdown result layers are
checked; source timeout aborts and joins the owned server; unsuccessful drainage
or a cold constructor timeout retains the disposable root.

The complete Python suite passes **210/210** with the repository's existing pinned
test tools installed only under `/tmp/pumas-q2-test-packages`. It includes **16**
local-wheel controls and **59** existing resolver tests. Strict all-target
app-manager and default RPC Clippy pass with warnings denied. Headless production
RPC binary Clippy passes. Full headless all-target Clippy remains failed on the
unchanged `create_valid_test_diffusers_bundle` integration-test helper's dead-code
lint; no unrelated source repair or all-target pass is claimed. Scoped rustfmt,
full CI Ruff scope (`torch-server scripts/release`) and `git diff --check` pass.
App-manager has an empty default feature set; it is not claimed as an independent
inference-enabled versus headless feature qualification.

Rust/Cargo1.92.0 checks are serialized with one job, incremental disabled,
offline/locked resolution and explicit dev/test debug0. Python3.12.14/pip26.2.1
are fixture tool versions, not new runtime pins; Ruff0.15.2 is the existing CI
pin. The unchanged ort declaration disables defaults and enables load-dynamic,
without download-binaries. ORT_SKIP_DOWNLOAD is unset; no runtime/model was
provisioned or downloaded. Test-only registry isolation sets XDG_CONFIG_HOME on
the child process, with no host/network/security configuration changes.

Both independent read-only reviewers ACK final source blobs: Torch
`1ab61cc803ea11aa605a8c77b4063287a2311567`, helper
`2daee7593f8c6cfac705ac113f84945035991054`, manager
`7a07fe2811cdb3eb857ed981b341b84a5656da94`, RPC
`c47f1f7e7f9e0c2852a2367f550f6ab4575783f9`, and fixture
`6377a9e3811e095cae5b22e7c964bed7d439f677`. Review is not provider, platform or
acceptance certification.

## Diagnostics, custody and remaining gates

Intermediate compile errors (manifest error conversion, UUID/string, borrowed
closure/input lifetimes), needless-borrow lint errors and their repairs remain in
raw logs. The initial full app-manager run failed on nine native fixtures writing
the read-only home registry and one associated SIGKILL parent timeout. The
child-only disposable registry rerun passes. The initial full Python run failed
seven unchanged route tests on ambient FastAPI0.141.1/Starlette1.6.0; the existing
FastAPI0.128.0 test pin plus its ordinary dependencies passes. No runtime pin or
unrelated serving code changed. The first descendant oracle used kill(pid,0),
which also observes exited unreaped zombies; the corrected Linux observation
checks running state at final input release. No process-cleanup production repair
was needed. Intermediate logs do not qualify final source by themselves.

The ordinary child-only bwrap network-denial probe failed with
`bwrap: setting up uid map: Read-only file system`. The evidence records its argv,
exit and transcribed tool result; there was no original redirected raw log.
No bypass, host setting change or simulated denial was attempted. Therefore
**enforced network-denied installation remains unqualified**, as do real Torch,
real provider, installed desktop/UI, supported platforms and hosted current-head
qualification. AQ-HTTP is a release dependency, and AQ-PACKAGES/AC11/AC12 stay open.
No account credentials were obtained, transmitted or provisioned.

Inactive own reproducible qualification binaries/dependency archives were hashed
before removal with no active Rust build. Protected release archives, retained
custody/evidence roots, source and frozen refs remain intact. All three temporary
PR42 dependency directories were restored to their original paths with matching
mode/path/content/link digests before completion.

[Evidence JSON](torch-verified-wheel-handoff-2026-10-06/evidence.json) binds exact
base/source/tree, final commands/profiles/exits, review blobs, cache manifests and
SHA-256 inventory of the validated 51-member raw archive. Archive SHA-256 is
`bbd29847b0e326284359f2fe8e4fb424c52d487176cfecf1e760915771708404`.
Original scratch is `/workspace/scratch/q2-wheel-handoff/`.

The next existing-plan source slice is Q2 automatic-selection separation of
accepted resolution from final payload installation. Its current `--install`
resolver path, managed-Python bootstrap and the qualified bundled recipe retain
written scope rather than being represented as shared-acquisition migrations.
Enforced-denial and real Torch acceptance need an authorized capable environment;
Q3 real-provider acceptance remains separately open. Runtime R2 and Q4 acceptance
remain gated. Parent coordinates review/CI/integration; no merge is proposed here.
