# Torch wheel child-pip configuration repair — 2026-10-06

Frozen Q2 handoff `7a3264ac8d04383bd01c8ab71b7287a48c66f2db` allowed
ambient `install.requirement` configuration to append another requirements file
before the exact-report check. On pip26.2.1, `--isolated` retains global/site
and PIP_CONFIG_FILE configuration. This corrects the earlier local-only source
claim; the historical handoff report is preserved rather than rewritten.

Separate branch `fix/torch-wheel-pip-config-7a3264ac` has these exact identities:

| Scope | Commit | Tree |
| --- | --- | --- |
| Frozen handoff | `7a3264ac8d04383bd01c8ab71b7287a48c66f2db` | `b6d37961c54d6e5fe0e894e5223c08f1213d804b` |
| Accepted main | `7c229e92726e1af7447d37e3dc03fd4bd2ccfffa` | `508caecefaf7825a357670c6a05357653bb0f4ef` |
| Normal merge base | `8eb1e00fd61400532b4c55b9dc167f0a9438fc0c` | `b6d37961c54d6e5fe0e894e5223c08f1213d804b` |
| Tested/pushed source | `fef0fb15833128b8a329a686e69ae95cb0c90624` | `6c961236f13e638253882cedfaeb4b89b0646bdb` |

The merge base has the frozen handoff and accepted main as its two parents.
Source/helper and test blobs are respectively
`b64d242f68d556a4fe7148ab13490b05c0918133` and
`5b07036ad41fe456399ccbbaae671aad5c2e1273`. Source author and committer are
MrScripty <TheEnvironmentGuy@protonmail.com>. The evidence-only successor
preserves those source blobs; its exact final head/tree is reported at handoff
rather than creating a self-referential commit identifier here.

The helper copies its environment, removes inherited case-insensitive PIP_*
keys and sets PIP_CONFIG_FILE=os.devnull only in the child. Public pip argv,
exact local file/hash inputs, METADATA closure validation, exact-report checks,
RECORD proof, child/input custody, postprobe validation and receipt identity
are unchanged. No public API, Rust/native/S3/dependency/pin/ONNX-policy writes.
No host or user configuration file/environment mutation. This uses the documented
[pip configuration control](https://pip.pypa.io/en/stable/topics/configuration/);
pip internals occur only in parser test seams, not the production invocation.

Qualification used Python3.12.14, pip26.2.1 and existing scoped Ruff0.15.2:

| Check | Result |
| --- | --- |
| New controls before production repair | 18 helper tests;3 failures +1 error |
| Helper after repair | 18 pass |
| Full Python suite with existing pinned test dependencies, final formatted blobs | 212 pass |
| Shared handoff controls with newly embedded helper | 13 pass |
| Ruff complete torch-server scope | Pass |
| Ruff format changed helper/test files | Initial check failed; formatter applied; final pass |
| Strict app-manager default all-targets Clippy | Pass |
| Strict RPC default all-targets Clippy | Pass |
| Two independent read-only final-source reviews | ACK, no actionable finding |

The three parser scenarios substitute owned global/site/env config files and
parse both old inherited and repaired child environments. The old options
contain a synthetic HTTPS requirement plus the local generated requirements;
the repair leaves exactly the local requirements, empty find-links/constraints,
and no-index/no-deps/require-hashes enabled. Parsing stops before installation;
no remote URL is executed. An actual public-pip control injects an owned local
requirements file pointing at a missing local file: old source fails; repaired
source installs exactly the accepted synthetic wheel, with report and RECORD
proof. Parent environment and fixture configuration bytes remain identical.
The 13 shared Rust controls recheck success/receipt identity, substitution,
missing closure, cancellation and abandoned-waiter drainage, publication failure,
cold refusal, postprobe mutation/ancestor escape and existing qualified pins.
They retain the previous fixture seams and scope, not a complete provider run.

On a separate pristine worktree at accepted main7c229e9, strict headless
all-targets Clippy fails on11 pre-existing generated-contract dead-code errors;
targeted integration_tests Clippy independently fails on the unused
create_valid_test_diffusers_bundle helper at line57. These are genuine baseline
findings, preserved in logs. No lint suppression or unrelated Rust repair is
included here. This repair does not claim the full headless gate is clean.

The [evidence index](torch-wheel-pip-config-repair-2026-10-06/evidence.json) binds
all20 raw archive members by SHA-256/size. The
[logs archive](torch-wheel-pip-config-repair-2026-10-06/logs.tar.gz) is
5288 bytes; SHA-256
`3d2fec12d15c670bda7a7b938f08e671bcfcb2f0218e759b82b942af60dbb0e5`. It retains red/green, formatter, baseline,
review and resource records. The historical first cache-copy assertion failed
before original-source deletion; the later complete copy and restoration were
independently verified. All41,202 original dependency-cache entries match modes,
paths, links and contents with digest
`7b37fb0447757adef9ab516e10d683403128f6c6843f836410e4518fb9be908f`.
Only this run's joined reproducible unit binary/core rlib were removed afterward,
with sizes/hashes recorded; evidence, release archives and frozen refs remain.

Parser controls prove configuration isolation, not network denial. The earlier
ordinary network-denial probe could not create its namespace on this host;
no bypass or security-setting change was attempted. Enforced-denial, actual
Torch/provider/platform/hosted acceptance and full AQ-PACKAGES remain open.
Parent owns PRs/reviews/merges; no external reviewer was contacted. Next existing
plan feature is automatic-resolution/final-payload separation, preserved at
`feat/torch-automatic-wheel-handoff-7a3264ac`, head
`af6113c69a42e6582c55b460277219e980f68a8c`, with unimplemented admission notes
in scratch and no automatic-resolution source edits in this repair.
