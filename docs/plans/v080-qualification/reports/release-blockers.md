# Release decision for the v0.8 source cohort

Status: source CI qualified; release remains blocked. Original Build 38522709859
is terminal failure (eight applicable jobs passed; headless failed its one-second
fixture cutpoint). Repair Build
[38526618039](https://github.com/MrScripty/Pumas-Library/actions/runs/38526618039)
completed successfully on aa40: all nine applicable jobs passed. Tag-only package
and assembly jobs were skipped. [The final receipt](qualification.json) binds
source, jobs and artifacts. No tag or publication decision is made here.

## Source and evidence identities

- Original reviewed candidate: `df53db08e2ba224edfaf713522ddd410f7d4ac44`.
- Bounded cleanup-fixture repair: `aa40e155846cdac6cd94b6f5c2fd6a35cddce16e`.
  Only test code and qualification records changed; no production timeout,
  public API, persisted schema, dependency pin or inference policy changed.
- Pantograph's separate [consumer receipt](https://github.com/MrScripty/Pantograph/blob/3649df2d3c4c718aa278e809f27cbc2a915f8c47/docs/plans/pumas-pr70-cpu-consumer/reports/consumer-compatibility.md)
  accepts the bounded local CPU edge on source
  `039cee079d273a8d4968df56a4b35e8eb8aacbbe` with Pumas df53, public Chrema
  `508c8b8cb724b49c655dab8bc4c8336190e516c7` and Candle
  `88ed7911de9e88196b1f55b199145d22647f415e`. Handoff commit is
  `3649df2d3c4c718aa278e809f27cbc2a915f8c47`. Nineteen integration contracts,
  598 library tests (2 existing ignores), 4 HTTP contracts and independent
  consumer review passed; no API repair was needed. The original PR78 baseline
  still pins aa1d79ae. These are the consumer owner's published observations,
  not tests rerun in this executor or acceptance against aa40.

The only Rust-file change from df53 to aa40 is inside the existing `cfg(test)`
module of `hf/download.rs`. Its 284,377-byte prefix before that module is
identical, SHA-256 `1cdc30e4f30b2f41104ee153bd63f9be7709aebd832084f33be109bee095d5d8`;
no other Rust path changed. This is reviewed source lineage, not a claim of
byte-identical binaries or automatic transfer of an independently pinned receipt.

## Concrete remaining gates

| Gate | Current obstruction | Exact completion evidence / owner |
| --- | --- | --- |
| Final source review and actual consumer pin adoption | Independent consumer review accepted the df53 edge. The aa40 repair is test-only, but final pin adoption/lineage acceptance is parent-owned. No separate PR70 independent diff review has been supplied or submitted on GitHub. | Parent accepts the final source/evidence, maps the df53 consumer receipt to the selected pin or requests its locked rerun, and dispositions the referenced review report with its actual SHA/scope. This requires no new Chrema SDK or unrelated repository release. |
| Required native artifact set and workflows | The exact aa40 Linux inference review archive passed its native job. Tag-only desktop, no-inference and three-target inference publication jobs are skipped on PRs. A source-quality pass is not the sixteen-file candidate. | Produce the accepted source/version's ten desktop installers and six headless archives. Apply RELEASING.md's exact extraction, native install/start, desktop/root-recovery and termination procedures on Linux x86_64, Windows x86_64 and macOS ARM64. Retain exact source/target/features/artifact hashes. Parent/native platform owners own this gate; this task does not authorize a tag to bypass pre-tag QA. |
| Required trained model and native-runtime execution | Linux review proves import/load/finite 256-dimensional execution/unload/graceful stop of an untrained graph through pinned ORT. No reviewed trained Nomic model/tokenizer fixture was provided here. Native Windows/macOS packaged model execution is absent. | Supply one exact reviewed trained model/tokenizer fixture and retain its hashes. Run `verify-packaged-onnx.py EXTRACTED_RPC NOMIC_DIRECTORY` on Linux with packaged CPU ORT 1.24.2; on Windows/macOS retain equivalent native request, loaded packaged-runtime identity, finite inference, unload and graceful-stop evidence. The artifact plan requires this on each inference target. This does not require enabling production speech or testing every future model. |
| Final-file release metadata and maintainer review | Current source default/S3 notices regenerate byte-identically and current audits are triaged, but no final sixteen-artifact set exists from which to produce complete metadata. | On the actual final files, verify embedded license/notices, generate each SPDX inventory, aggregate in-toto provenance and SHA-256 checksums over all other final files. Maintainer accepts the notices/current vulnerability results against the shipped closure. Source inventories alone do not satisfy final-file coverage. |
| Final 0.8.0 identity and promotion | Current manifests remain 0.8.0-rc.1; they cannot be tagged v0.8.0. | Parent selects the compatible checkpoint, aligns final manifests/workspace lock entries and versioned notices without incidental dependency resolution, reruns affected checks and rebuilds final artifacts. Only then can the parent authorize the exact final tag and later publication. |

These gates follow `scripts/release/artifact-plan.json`, RELEASING.md and the
current binding support matrix. Unrelated repository releases, new SDKs, installed
production speech, custom Torch assembly, download-location reconciliation and
other proposed features are not added to the blocking list. Tuldok trained vision
acceptance is required for a demo that promises it; it is not a dependency of
Pantograph's untrained CPU source demonstration.
The acquisition plan's AQ-HTTP/AQ-S3/AQ-COMPLETE gates still own their named runtime,
provider and full-plan claims. Current controlled S3 protocol tests do not accept
live AWS/non-AWS/MinIO combinations. Those broader claims are not introduced as
hidden prerequisites for the local, zero-acquisition-byte CPU source edge.

## Closed or bounded findings

- Repair Build 38526618039 passed all nine applicable jobs, including native
  Linux/Windows/macOS source QA, Rust quality, frontend/desktop, actual Linux
  inference packaging and confined audio dependencies. Its no-default and HF
  core cohorts each passed 2,294 tests with 29 ignored; RPC passed 257 with 13
  ignored. The required ignored Linux kernel read-confinement proof was separately
  executed and passed. Enabled S3 reader/acquisition tests passed 58, signing/prefix
  tests passed 43 with 2 ignored, and S3 warning-deny lint passed. Zero-test
  feature-gated targets do not qualify disabled features.
- Earlier catalogue timing failure did not reproduce in the complete local
  no-default core cohort; retain its evidence boundary rather than resurrecting it.
- Fresh Cargo audit: zero vulnerability entries, three maintenance notices, one
  lru unsoundness notice whose trigger is absent from the exact SDK key/access
  path. Fresh pnpm production audit: zero advisories. The one moderate build-only
  sprintf-js advisory retains its existing disposition, without suppression.
  See [the dependency review](dependency-review.md).
- Default/S3 notice text and inventories regenerated identically: 378/428 entries.
- Schema-7 migration remains explicit and offline, with all readers/writers
  stopped and an exact metadata-only backup. It provides no automatic downgrade,
  model backup or dead-PID claim deletion. No user library was migrated here.

## Executor limitation separate from product failures

GitHub successfully retrieved audio artifact 11874379748 and exact df53 Linux
inference artifact 11875745665 into file references. This executor's HTTP proxy
denied both subsequent byte transfers with tunnel 403; no ZIP bytes were received
locally and no alternate host/authorization bypass was attempted. Local extraction,
inner-archive/binary hashes and local replay remain unavailable. The source-bound
successful CI package job and GitHub ZIP identities remain usable scoped evidence.
Current repaired-source artifacts from Build 38526618039 are:

| Artifact | Bytes | GitHub ZIP SHA-256 |
| --- | --- | --- |
| `review-inference-linux-x86_64`, 11877010982 | 30,659,584 | `3dfdca18419edf3b1125f2ad6e6577c2d9b480caa293e3a612d979a7bbac4303` |
| `review-inference-evidence-linux-x86_64`, 11876891933 | 35,263 | `f321f82c812a3853be83dd87a03f29f52d93b7e016ad1d422ded7dfeb91b741c` |
| `review-audio-dependency-evidence-linux`, 11876861099 | 10,686 | `979d4b4928074b2734808b43a66fc70000d2258b23c3d24e1c5b10e2de4ccea3` |

Earlier df53 artifacts remain separately identified:

| Artifact | Bytes | GitHub ZIP SHA-256 |
| --- | --- | --- |
| `review-inference-linux-x86_64`, 11875745665 | 30,659,590 | `2dc6dd91e0f7f6258b5f7c96a433d45653190cb939eb6bad12c4c03283627a91` |
| `review-inference-evidence-linux-x86_64`, 11875865202 | 38,521 | `a51626da4d0460ffd083618ffa2461ade5126bb012044789c3490c6f92e498bf` |
| `review-audio-dependency-evidence-linux`, 11874379748 | 14,477 | `effe0e880f823f305e59baad54cc450a220e9897273dd5c4a7baacb835113e9c` |

These GitHub-reported ZIP hashes identify workflow artifacts, not the contained
tarball, native binary or final release inventory. Their seven-day retention ends
2026-10-21; preserve required release evidence through the parent-approved path.
