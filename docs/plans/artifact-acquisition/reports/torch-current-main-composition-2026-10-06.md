# Q2 current-main composition and acceptance plan — 2026-10-06

Prepare a finite current-main candidate for parent review without a PR, main merge
or expanded resolver implementation. The optional existing qualified recipe is
the only newly qualified package source boundary. Automatic and retained-preview
resolution retain their known pre-report source-preparation P1; their presence
and tests do not establish a safe resolver boundary.

## Exact inputs and proposed changes

Current main: `5e114f6d8e4559e0a4d67e56000b423120a0fde0`, tree
`cf5cfa2f5017a157f84ed5461df8b920dda2e7db`. Reviewed finite candidate:
`6308e361934fc15bd35773cfd7a3747063885fc5`, tree
`a127e52dd7e3ecefdeb888d63a8f1004e15168ed`; source498b7961/tree97dbe689.
Common ancestor: `7c229e92726e1af7447d37e3dc03fd4bd2ccfffa`.
Fresh main adds Node maintenance; its inherited HTTP budget/source remains intact.
No AGENTS.md or relevant filesystem skills were found in the selected workspace.
Frozen source/evidence branches remain unchanged; the patch is based on fresh
main, not a saved environment seed or rewritten candidate history.

The complete inventory is bound in adjacent evidence. The proposed composition
carries 16 source/contract paths and their immutable qualification/research
history (45 paths total before this plan). No source integration repair has been necessary in the 306 app-manager / 229
Python composition tests; 15 source/contract blobs remain exact. README claims are narrowly corrected to
disclose the inherited source-preparation P1 and dry-run side effects. No merge conflict exists; the predicted
combined tree before this plan is ece53cda53d5bd9d67547c0341a32c1ec4efa728.
This is a patch composition on a distinct branch, with no merge commit.

| Layer carried | Original source | Paths / purpose |
| --- | --- | --- |
| Shared retained-preview handoff | e246645b | installer/torch.rs, torch_wheel_handoff_tests.rs, version_manager/mod.rs, RPC main; complete acquired wheel set, child/use custody, local installer, proof, publication |
| Local pip config repair | fef0fb15 | install_verified_wheels.py/tests; child-only configuration isolation |
| Automatic/common packet composition | 5946c473 | torch.rs/tests, resolve_runtime.py/tests, narrow RPC integration-test cfg; common prepared packet, accepted resolution/final-payload separation |
| Finite optional recipe | 498b7961 | torch.rs/tests, qualified_wheel_catalog.py/tests, local installer, wheel_records.py/lazy aliases, licensed tooling snapshot |
| Contract/provenance | frozen candidate history | acquisition contract, Torch README, owning ledger/matrix/gates/write sets, exact original reports and evidence |

Carrying the reviewed packet/publisher implementation retains its automatic
refactoring as inherited source; it does not qualify or repair its resolver.
Public interfaces: shared Torch capability through new_with_acquisition and
with_torch_acquisition; existing constructor signatures remain. The finite catalog
CLI adds --lock/--preview/--output; local consumption adds paired optional
--recipe-lock/--preview; resolver --resolve-only is inherited unqualified support.
No new Rust/RPC DTO, Cargo dependency, provider pin, runtime lock/version policy,
uv implementation, credential, S3/native or shared cleanup write is proposed.

## Compatibility decision recorded before activation

Current main promises upstream/core automatic install, newest-compatible managed
Python selection and retained previews. The README and active public manager/RPC
routes establish that promise. Temporarily disabling those paths would change
that behavior, including non-preset releases and non-Linux-x86_64 targets. They
cannot remain explicitly unavailable product-wide without that compatibility
change. A temporary-disable policy needs parent disposition, not a hidden refusal
or an accidental new mandatory v2.9.1 policy.

The draft therefore preserves existing dynamic availability with explicit P1 /
unqualified status and qualifies only the optional finite recipe. The finite
route never invokes those resolvers. Parent was asked to choose preservation or
temporary disable before any disabling change; no answer is inferred as approval.
No disable is implemented here. Parent review must explicitly disposition whether
an otherwise publishable optional feature may coexist with the exposed existing
paths; this candidate is not a product-wide source-safety repair.

The inherited shared handoff also changes direct constructor behavior: old
constructor signatures remain, but Torch installs require the explicit existing
acquisition capability. RPC injects it. Direct callers must use
with_torch_acquisition/new_with_acquisition. This is an intentional acquisition
contract boundary and a consequential compatibility item for parent review.
No hidden fallback or second service/store is added to preserve old calls.

## Conflict and dependency surfaces

Current-main Node/package locks and HTTP acquisition source/fixtures must match
main byte-for-byte. Torch runtime/requirements.lock and selected roots remain
exact. Cargo manifests/lock and feature declarations are unchanged. Relevant
surfaces are torch.rs common packet/installer/lifecycle, version-manager startup
reconciliation, RPC consumer wiring, Python copied-script embedding and public
packaging tooling licensing. Research/private adapter files remain evidence only;
no private-hook production import or activation is allowed.

Native cancellation repair `3a601625` is independently accepted but unpublished
because of its owner's authentication block. It is not available in this clone
and is not integrated here. Native/shared cleanup source is unchanged. Later
composition of that exact repair needs parent-provided source, identity checks
and fresh lifecycle qualification; prepared-use fixture passes cannot substitute
for the native early-transfer withdrawal repair.

## Build and acceptance sequence

1. Before any build, run current-main scripts/release/check-dependency-features.py
   --s3 (offline/locked cargo tree). Then run the same graph on composed source.
   Both pass all 33 checks across Linux, macOS and Windows graphs: default/headless,
   explicit fixtures, S3 single transport, workspace/all-feature ONNX no-download.
2. Validate exact carried source blobs, main Node/HTTP sources, unchanged Cargo
   graph/recipe, embedded licensed tooling and diff/format contracts.
3. Run full scoped Python suite, full app-manager tests, strict default workspace
   all-target Clippy, focused headless RPC binary/integration checks. Keep bounded
   source/proof/receipt/cancellation controls and original negative logs.
4. Full headless all-target generated-contract warnings had an exact accepted-main
   baseline before this task. Repair only a new source integration/compile error;
   do not suppress those warnings or broaden this feature into baseline cleanup.
5. Record exact candidate source/tree and raw checks, restore the worker-owned
   temporary cache, commit/push the feature branch for parent review. No PR or
   main merge is authorized here. Documentation/evidence successor changes no
   tested source blob.

## Remaining objective gates

| Gate | Exact remaining observation / disposition |
| --- | --- |
| Parent compatibility/review | Explicit coexistence/temporary-disable and direct-capability migration decision; review exact current-main candidate |
| AQ-HTTP dependency | Objective HTTP, deployment, public-client/resource/platform acceptance remains as recorded; passing graph is not acceptance |
| AC11 / AQ-PACKAGES | Actual unchanged 66-wheel recipe acquired and installed, GPU/protocol/provider probe and exact receipt; missing/changed inputs refuse |
| AC12 / AQ-PACKAGES | Enforced network-denied final local install, hidden locator refusal, actual selected/installed identity; parser/fixture flags alone insufficient |
| Hosted/platform | Exact-head ordinary/default/headless CI, real managed-provider/Torch targets, packaged consumers and supported platforms |
| Lifecycle composition | Parent supplies unpublished native3a601625 and separately qualifies its interaction; no present integration claim |
| Dynamic resolver | Supported admission design must prevent preparation before accepted metadata; automatic/retained-preview P1 remains open |

## Later supported public uv alternative

Public uv pip compile is viable for a separate bounded evaluation, not a proven
replacement. Maintainer documentation specifies --only-binary enforcement on
direct URLs; --no-build has editable and cached-wheel details requiring explicit
root/cache restrictions. Index, prerelease, resolution and transitive URL behavior
differ from pip. A later admission must pin/provision a licensed executable per
platform, isolate configuration/environment/cache, preserve chosen index/fallback
policy and adapt immutable output provenance to the existing acquisition handoff.
Inert direct-source/VCS/editable/marker and no-fallback controls plus actual
custody/network-denial qualification are required. No uv dependency, executable
probe or implementation is added in this composition.
Sources checked 2026-10-06:
[uv compatibility](https://docs.astral.sh/uv/pip/compatibility/#only-binary-enforcement),
[public CLI](https://docs.astral.sh/uv/reference/cli/#uv-pip-compile).
