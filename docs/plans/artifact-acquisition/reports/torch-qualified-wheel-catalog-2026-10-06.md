# Finite qualified Torch recipe candidate — 2026-10-06

The optional existing bundled recipe now selects its finite exact wheel set with
supported standalone packaging APIs, then uses existing shared acquisition and
local installation custody. It invokes neither the automatic/retained-preview
resolver nor a private pip hook. Actual acquired wheel metadata and complete
closure must validate before local pip. This is a tested candidate for independent
parent review, not package, runtime, security or release acceptance.

| Scope | Commit | Tree |
| --- | --- | --- |
| Feature base | `d87f560f5e451083eaa2635bf84ba1980784cae8` | `9c6b4ec4a75054f4e01c5f9d3c8be5e0b2badb0e` |
| Tested source | `498b79611ded25151e28531b8f961dac1462061c` | `97dbe68916150da6a1375abd026066293553f9a2` |

Branch `feat/torch-qualified-wheel-catalog-d87f560f` preserves accepted main
`7c229e92726e1af7447d37e3dc03fd4bd2ccfffa`, original main
`05717338c2aea737483fb4b28c3ed4053a65de96` and original PR40
`63123fd8f9f866064a8315096ab3fb1fc81e8f0f` (tree
`163ef2442695b44555ac0e42ff79a6a88de9e817`) as ancestors. Frozen repair,
automatic, private proposal and public research branches are not rewritten.
Author/committer: MrScripty <TheEnvironmentGuy@protonmail.com>. The evidence-only
successor records its final exact head/tree at handoff.

## Accepted source boundary

The existing lock remains byte-identical: 66 exact distributions, three original
direct roots, all allowed SHA-256 values, original indexes and target. The finite
catalog reads Simple Repository metadata only at the two original HTTPS indexes.
It filters exact versions, allowed hashes, trusted wheel authorities, interpreter
tags and Requires-Python; ranks compatible tags and wheel build tags and refuses
an ambiguous best choice. Exact pinned yanked files are eligible under the pinned
selection policy. Limits are 128 KiB lock, 128 packages, 1 MiB per index response,
10,000 rows, at most 256 requests, 5-second socket timeout and a 180-second elapsed
check between index responses. The elapsed check is not a hard deadline for a
slow streaming response. Unavailable/unsupported evidence is inconclusive and
refuses; no source or dynamic resolver fallback is available.

The original selected direct root URLs, versions and hashes must agree, including
Nunchaku's original SHA-256 fragment binding. Payload manifests normalize that
fragment after binding; original lock/preview evidence stays unchanged. Rust
checks complete lock membership and trusted source/pin/hash identity before
acquisition. The Python local consumer checks original preview/lock/catalog again
and then actual whole-wheel hash, filename/metadata identity, target tags and
Python compatibility. WHEEL and METADATA belong to the same dist-info directory.
Public metadata validation, dependency markers and propagated extras establish
complete active closure. Every dependency URL, including inactive source/VCS
branches, refuses with an explicit bounded diagnostic before public pip executes.

Final pip uses exact verified local file URLs and hashes, --no-index, --no-deps,
--only-binary and --require-hashes with the existing child-only config isolation.
The shared installer retains input/stage child custody, installed RECORD proof,
existing qualified GPU/protocol validation, probe, postprobe proof/provenance
recheck and receipt settlement/publisher. It records qualified-preview.json,
original requirements.txt and honest pumas-qualified-wheel-catalog-1 resolution
rather than fabricating a pip report. Failures after acceptance propagate through
the shared path without entering candidate fallback. No acquisition identity,
retry/receipt policy or shared/native cleanup interface changed.

## Tooling and interfaces

Standalone packaging26.3 tooling is embedded before target package installation;
26 source/metadata/licence files were copied from the already installed standalone
distribution after checking its RECORD SHA-256 and size. The deterministic ZIP is
128132 bytes, SHA-256
`3453711fd71407b7d84253d5b498e7c0430fff187e3fb73dbd6f463b924014b2`.
Its [source inventory](../../../../torch-server/tooling/packaging-source.json)
and archive contain Apache-2.0 OR BSD-2-Clause licences. This is local tooling
provenance, not an upstream wheel-byte claim or new runtime/provider pin.
The qualified helpers import supported public packaging APIs, never pip's private
vendored namespace. Existing RECORD proof moved unchanged into wheel_records.py;
resolve_runtime.py keeps lazy compatibility aliases so its isolated copied-script
help/discovery path still starts without the new module.

New Python CLI: qualified_wheel_catalog.py --lock --preview --output. It emits
pumas-qualified-wheel-catalog-1 for the existing CPython3.12/Linux x86_64 recipe.
The local installer adds paired optional --recipe-lock/--preview flags. Existing
local-install flags and resolver exports retain signatures. There is no new
public Rust/RPC DTO, mandatory fixed-version policy or replacement for generic
runtime/model management. Managed-Python bootstrap remains separately scoped.

## Bounded validation

Linux x86_64; Rust1.92.0; Python3.12.14; pip26.2.1; scoped Ruff0.15.2. Python tests
use the previously scoped FastAPI0.128/Starlette0.50 test dependencies rather than
changing runtime pins. Cargo checks are offline/locked and serialized, with no
ORT or provider download.

| Final check | Result |
| --- | --- |
| Full Python suite | 229 pass |
| New finite catalog/preflight/compatibility controls | 13 pass within full suite |
| Full app-manager suite | 306 pass; 1 existing ignored unit and 1 ignored doctest |
| New Rust exact recipe/embedded tooling controls | 2 pass within full suite |
| Existing shared custody/receipt/cancellation controls | 13 pass within full suite |
| Strict default workspace all-targets Clippy | Pass |
| Whole torch-server Ruff; five changed Python files formatted; Rust fmt | Pass |
| Tooling archive and all 26 member hashes; diff check; ancestor preservation | Pass |

Positive controls install inert actual wheels with propagated extras through
public local pip and RECORD proof. Negative controls cover source/archive/VCS and
inactive URL metadata, wrong hash/source/version/count, changed acquired bytes,
actual metadata identity/Python/WHEEL mismatch, missing active-extra closure,
unsupported locks and ambiguous catalog choice. Fetch callbacks observe only
approved index addresses, and no pip resolver is invoked. Shared Rust controls
exercise receipt identity, missing/changed payloads, postprobe mutation, failed or
lost-ack publication, cancellation/abandonment and cold refusal. Those controls
reuse the shared pipeline; they do not execute all 66 real recipe wheels or the
new real managed-provider/catalog/GPU sequence end to end.

Original logs retain the new Clippy nonminimal-boolean failure before correction,
one wrong-working-directory invocation, and formatter refusals before formatting.
No test oracle or lint was relaxed. The [evidence index](torch-qualified-wheel-catalog-2026-10-06/evidence.json)
binds raw logs, source blobs, tooling and cache restoration. Earlier headless
all-targets generated-contract failures are separately recorded baseline evidence;
this feature claims only its strict default workspace pass.

The raw archive contains 24 logs/records, 28588 bytes, SHA-256
`e34ee8fbff772b1beb60293eac6515237fcf3f9762c91b4aac55dbed95f3405f`.
The parked development cache was restored with all 41,201 paths, modes, sizes,
content hashes and symlink targets matching manifest SHA-256
`04a9842e7e71f924328f5ed986eaaff6f8337d0b59dc1c5c7b5cbe6bc7ce9951`.
Only this worker's joined reproducible unit binary/core rlib were parked under
the shared Cargo lock, with verified copies and byte identities retained. Source,
frozen refs and release archives were untouched. No real credentials, provider
provisioning, paid service, security/network bypass or external reviewer contact.

## Remaining disposition

Automatic and retained-preview resolution remain unqualified for the separately
reproduced pre-report source-preparation gap. Private pip hooks remain unactivated;
no contract exception was used. No real Torch/provider/GPU/download campaign,
enforced network denial, hosted/platform acceptance or whole-venv integrity is
claimed. AQ-HTTP, AC11/AC12 and AQ-PACKAGES remain open. Separate native
cancellation cleanup stays with its owner; any later interface successor requires
explicit composition/requalification. Parent owns PR/review/CI/integration.

The next existing-plan Q2 work is independent review/composition and actual
network-denied local-install/provider/platform acceptance for the exact handoff.
Generic public resolver admission remains a separate decision; this bounded
recipe does not repair or automatically authorize dynamic selection.
