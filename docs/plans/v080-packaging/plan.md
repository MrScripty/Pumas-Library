# v0.8 native package qualification

**Status:** Verifying. **Acceptance:** partial. **Operation:** verify the implemented
slice admitted by the parent's explicit 2026-10-14 packaging continuation.
**Current phase:** local verification complete; CI and independent parent review.
**Next slice:** inspect exact-candidate checks and the parent's cumulative review.

## Objective and boundaries

From accepted main `055222931100eeedc43307271dbf7c504787888d`, make official CPU
ORT provisioning explicit for full desktops, support actual packaged ONNX lifecycle
verification on Linux x86_64/Windows x86_64/macOS ARM64, and generate complete
artifact-level release metadata for all sixteen planned files. Reuse existing
jobs for an opt-in, build-only manual rehearsal. Outputs remain unsigned,
unverified candidates; no tag, release publication, trained-model acceptance by
mock, ASR, vision, SDK, product API or schema change is authorized here.

Isolation: `release/v080-package-qualification-20261014`, rooted at accepted main;
one serial integration owner, draft PR to main, preserve history and branch until
parent adoption. Existing checkpoint/receipts and user inference work are read-only.

## Binding design and owners

- `onnx-runtime-pins.json` stays the sole runtime identity authority. Full desktop
  jobs explicitly stage/probe the official 1.24.2 archive, pass its directory to
  RPC staging, and include its license/notices. No-inference packages retain their
  exact inference-disabled backend and do not gain native runtime payloads.
- Extend `packaged_onnx_support.py` and the existing lifecycle verifier rather
  than add a second harness. Use Python copying, native OS module observations,
  isolated child environments and platform-correct graceful shutdown. Linux
  retains mapped device/inode checks; Windows/macOS record native module paths
  and current pinned file bytes with their narrower identity limits explicit.
  Native source contract tests do not prove packaged model execution.
- Reuse existing artifact-plan identity checks before generating a per-artifact
  SPDX package inventory for all sixteen final files, aggregate unsigned in-toto
  provenance, combined accepted source/native notices, and checksums over every
  other final file. `filesAnalyzed=false` is explicit for archive-level SPDX;
  no exhaustive extracted-file/native transitive component SBOM is claimed.
  Preserve the old Linux extracted-file helper and its historical receipts.
- Add a default-false manual `build_candidates` input and extend existing tag
  job guards to this explicit build-only dispatch. No publication credentials or
  tag writes; ordinary PR/main checks stay bounded. Candidate mode omits the
  separate review archives/dependency probes and excluded Torch sidecar E2E;
  ordinary manual/tag behavior is retained. Parent reviews the exact
  SHA/dispatch before any full rehearsal. Add native untrained lifecycle execution
  to the reused inference matrix, without promoting it to trained acceptance.
- Investigate exactly the documented Nomic v1.5 FP16 fixture and immutable public
  source/license/file identities. The executor's first HF metadata request was
  proxy-denied (403); no model bytes transferred. Do not bypass that restriction,
  substitute another model or download the whole repository. A supplied fixture
  receives byte hashes in its lifecycle receipt; missing authoritative material
  remains visible rather than guessed. No private library scan.

## Acceptance claims

| ID | Observable criterion | Kind / environment / mode | Status |
| --- | --- | --- | --- |
| PK1 | Full desktop staging refuses absent/drifting official runtime and includes verified native files/notices; no-inference configuration excludes them | contract / representative source fixtures plus required-real final packages / automated | source regressions satisfied; actual final packages pending approved run |
| PK2 | The shared verifier supports each native OS module observer, isolated startup, import/load/finite embedding/unload and graceful stop; actual native results remain separately identified | system / required-real selected native target / automated | implementation and Linux contract tests satisfied; native/model results pending |
| PK3 | Sixteen exact artifact identities produce sixteen SPDX inventories, provenance subjects and final-byte checksum coverage, with closure/assurance limits explicit | release-artifact / controlled complete fixture and later real final files / automated | complete synthetic inventory satisfied; real artifact set pending |
| PK4 | Manual build-only dispatch reuses the matrix with read-only contents permissions and creates no tag/release; PR/main packaging remains off | contract / workflow analysis and focused dispatch fixtures / automated | source guard/opt-in/no-publish checks and actionlint satisfied; dispatch not executed |
| PK5 | Exact Nomic source/license/revision/weight identity, bounded download feasibility and unavailable inputs have authoritative evidence | contract / actual public upstream and executor / manual | research satisfied; trained bytes/config hashes and execution unavailable |
| PK6 | Focused Python/Node, formatting/lint, workflow validation and independent cumulative review have retained outcomes; commit/push/draft PR are concrete | integration / available local/CI and parent review / either | local checks satisfied; CI and independent parent review pending |

## Milestone, write set and gates

One coherent milestone: release-tool preparation, **Verifying**. Write set:
`.github/workflows/build.yml`; `scripts/release/stage-rpc.py`,
`packaged_onnx_support.py`, `verify-packaged-onnx.py`,
`verify_synthetic_packaged_onnx.py`, `write-release-metadata.py`,
`test_inference_release.py`, `test_release_metadata.py`, and focused release
workflow/packaging fixtures; `electron/package.json` and
`electron/electron-builder.no-inference.cjs` resource filters and directly
derived default/S3 attribution inventories; `RELEASING.md`; this plan directory.
Add a directly affected file only with an explicit ledger disposition before edit.
No Cargo or pnpm resolution updates. Preserve legacy verifier positional CLI and
receipt fields consumed by synthetic qualification.

Gates: regression negatives must decide absent/drifted runtime, ambient module,
unsupported host, forced shutdown, missing/duplicate/stale artifact, incomplete
metadata/checksum, and unintended packaging dispatch. Run the focused existing
release suites and native-source tests, Ruff, Node release tests, actionlint or
available workflow checks. Retain failed attempts and platform not-run boundaries.
Use an independent parent review of the final material diff; no self-review is
called independent. Actual full matrix/trained/native model qualification may
remain pending with exact runnable procedures/owners; source fixtures cannot
close those release gates.

## Composed-design review (applicable)

1. Runtime pins/staging own dependency bytes; lifecycle support owns observation
   and process disposal; metadata owns final-file descriptions; workflow owns
   explicit dispatch. Maintainer owns acceptance/publication, parent owns run/tag
   approval. Their contracts are referenced, not merged into one authority.
2. Necessary interleaving is stage/probe before native execution and final
   collection before hashes. Model data never enters source/package payloads.
3. Callers provide target, verified runtime directory, binary/fixture or final
   artifact directory; helpers hide OS module enumeration and metadata encoding.
4. Runtime pin updates stay with pins/staging; new supported targets touch the
   artifact plan/native observer; release file additions touch the plan and its
   generator consumer; dispatch policy stays in workflow and its focused check.
5. Stable values are target IDs, exact file digests and existing artifact-plan
   identities. OS observation and signal details stay behind lifecycle support.
6. Each helper has independently testable/refusable inputs. Metadata cannot grant
   runtime/model acceptance; workflow success cannot grant publication.
7. Deleting native support would spread platform branches across verifiers;
   deleting the final metadata generator would leave sixteen partial/manual
   descriptions. No registry, generic plugin, second lifecycle framework or
   duplicate artifact-plan authority is introduced.
8. Inherent complexity is three native observers plus sixteen artifact identities;
   existing deep helpers contain it. Retain one lifecycle path and existing
   producers, not a replacement release workflow.

## Standards, blockers and re-plan triggers

Canonical Coding-Standards main is unchanged at `188beda1fa477d21d576c233dd8c7f4c4c267d23`.
Planning/Implementation/Verification/Oracles/Platforms/Proportionality,
Build/Release/Commit and Architecture/Dependencies/Security/Licensing apply.
No repository AGENTS.md or relevant `.agents/skills` exists. The application
standards router is unavailable; manual canonical reads confer no executable
standards certification.

Blockers: executor HF transfer denied; Windows/macOS final package execution and
the full build-only dispatch require their actual native jobs/run approval;
independent review is parent-owned once the candidate is concrete. Replan for
new dependency resolution, production runtime/schema change, unverifiable model
authority, new permanent composition, publication permissions or artifact-plan
support expansion. Stop at preparation plus exact remaining release gates.

[Ledger](execution-ledger.md) · [Issues](issues.md) ·
[Verification receipt](reports/verification.json) · [Nomic investigation](reports/nomic-fixture.md)
