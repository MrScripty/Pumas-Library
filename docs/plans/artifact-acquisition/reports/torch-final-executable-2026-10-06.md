# Final selected-executable proof repair — 2026-10-06

The final owned proof check now revalidates selected-consumer executable bytes
against the retained observation after all validation/probe children and before
publishing the bound runtime or issuing its receipt. The separate provider hash
cannot satisfy this check. Unbound legacy behavior, observation schemas, source
policy, package proofs and resolver/catalog paths are unchanged.

## Exact base, design checkpoint and tested source

Separate branch `fix/torch-final-executable-2ce91b43` starts at frozen
`2ce91b431afacdb4eb402af91f55b1258afe04fc`, tree
`e1dc46a0e45926bf1d1906813c456d2fed7e259c`. Tested repair source is
`7ceac3a17b1c5fd3c3e870eddac0b3601a27b2ce`, tree `f3483d0d918451e7f1d25b77026d2c0da738a49e`; committed and pushed without force.
The final evidence-only successor head/tree are reported after push.

Before this repair, the requested catalog-authority design checkpoint was saved
and pushed separately at `e6b5b876c282e6eb9786366994a6d212e6f3c0fc`, tree
`ae03b4a41ee03a8d4c06b87c38c0ebc2a4d29c7a`, on
`design/torch-catalog-authority-2ce91b43`.
[That design](https://github.com/MrScripty/Pumas-Library/blob/e6b5b876c282e6eb9786366994a6d212e6f3c0fc/docs/plans/artifact-acquisition/reports/torch-catalog-authority-design-2026-10-06.md)
changes only documentation/source inventory and specifies the existing caller's
repository/target/version authority, bounded observed-universe completeness,
three implementation slices and decisive controls. It introduces no mandatory
Torch version or mirror and needs no new user preference for existing-source
implementation. It is not merged into this source-repair branch.

## Executed frozen reproduction

Independent review reported that selected bytes were checked before consumption,
then runtime validation/probe children could change the executable before final
package/provenance proof and receipt publication. The new actual controlled
fixture reproduces against the unchanged frozen production validator:

1. The existing owned producer captures the actual copied selected venv; the
   controlled finite catalog, shared acquisition, qualified local CLI/pip and
   installed RECORD proof complete normally.
2. A supervised probe launched by that same selected executable atomically
   replaces its stage-owned copy with synthetic changed bytes. It never writes
   the executing inode or the managed provider. Original observation/approval,
   resolution/recipe/preview and installed package proof bytes stay unchanged.
   The fixture checks changed selected hash, unchanged provider bytes and exact
   retained observation/proof/provenance before the final validator.
3. Frozen validation still succeeds: the runtime and receipt publish, the
   acquisition reaches Adopted and installed metadata/output exist. The intended
   refusal assertion fails. Its log records all five facts and successful owned
   cleanup drainage. This is an executed reproduction, not only source reasoning.

[Frozen log](torch-final-executable-2026-10-06/frozen-regression.log),
[regression-only patch](torch-final-executable-2026-10-06/regression-before-fix.patch)
and [byte-identical frozen-source proof](torch-final-executable-2026-10-06/reproduction-source.json)
retain that evidence. The deliberate frozen failure is distinct from the final
passing suite.

## Narrow repair and verification

`validate_torch_final_proof` accepts a private optional selected-target/path pair
and calls the existing `validate_torch_target_evidence` after the installed-member
and provenance checks. Production passes the actual selected venv path and a
clone of the already accepted observation into the same registered
`AcquiredArtifactUse::run_blocking` job after both validation/probe children.
Nothing recaptures or renews approval; the original selected SHA256 remains
expected authority. The provider's receipt field stays separate. Absence of
accepted target passes None and preserves the existing legacy path.

The production diff is seven added lines and one changed call line. No public
Rust/Python API, DTO, durable schema, recipe/dependency, resolver, provider or
source-access policy changes. The [target contract](../../../contracts/wheel-target.md)
records the final check.

- **31 Rust handoff controls pass**, including the new executable-only probe
  mutation refusal, retained Using inputs, no receipt/publication and joined
  cleanup. The valid unmodified qualified producer→catalog→local-consumer path
  still publishes the digest-bound receipt and reaches Adopted. Existing provider
  separation, approval/provenance/package mutation, legacy, cancellation,
  abandonment and cold-replay controls remain passing.
- **33 feature graphs pass before Cargo**; strict offline/locked serialized
  one-job Clippy with `-D warnings`, scoped Rust formatting and diff checks pass.
- Python production/tests, target capture, finite catalog, automatic/preview and
  all core/native/importer/watcher/S3/manifest sources remain unchanged. The
  predecessor's Python qualification is not claimed as a new run here.

[Final handoff](torch-final-executable-2026-10-06/handoff-final.log),
[Clippy](torch-final-executable-2026-10-06/clippy.log),
[feature graphs](torch-final-executable-2026-10-06/feature-graphs.log) and
[exact commands/source/log hashes](torch-final-executable-2026-10-06/provenance.json)
provide the supporting evidence. All commands and managed child/effect cleanup
were joined. Same selected executor, Rust/Cargo 1.92.0, existing isolated /tmp
build target; no old seed, credential/provider/model downloads, paid service,
security/network bypass or external reviewer contact. Parent owns PR/review/CI/
merge coordination. Frozen producer candidate and main remain untouched by this
worker, as recorded in [remote refs](torch-final-executable-2026-10-06/frozen-refs.log).

The reported P2 has a reproduced negative and passing narrow successor. It does
not claim a whole real Torch/GPU recipe run, installed/native-platform or
OS-enforced network-denial acceptance. Next Q2 work is separate implementation
admission for the saved catalog-authority design; upstream/tool/target/selection
qualification and P1/AQ gates remain open.
