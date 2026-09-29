# Acquisition issues and dispositions

Q1 is active; production repairs and acceptance claims remain pending until evidence is recorded. Evidence references [the source audit](reports/codebase-audit.md), [the canonical contract](../../contracts/artifact-acquisition.md), and its linked primary sources. Severity is consequence within this scope, not an asserted exploited vulnerability.

| ID | Severity / issue | Owner / disposition | Deciding evidence and revisit condition |
| --- | --- | --- | --- |
| AQ-I01 | High: model-specific destination authority cannot become the generic artifact root | Acquisition/core: fix in Q1 | AC01/AC04/AC09; re-plan if safe neutral capability extraction cannot preserve model custody |
| AQ-I02 | High: retained model/HF records and recovery transitions need explicit migration | Core/recovery: fix in Q1 | AC04–AC06; actual source/deployment inventory before mutation |
| AQ-I03 | High: cancellation/restart must retain worker and byte-use ownership | Acquisition: fix in Q1 | AC05/AC06; no dropped-future or timeout-as-cleanup proof |
| AQ-I04 | High: acquisition/import/install completion and duplicate transfer owners | Acquisition plus consumers: consolidate in Q1 | AC03/AC05/AC08/AC18 |
| AQ-I05 | High: cross-plan duplicate authority or prerequisite cycle | Integrator: fixed in plan design; implementation evidence pending | Gate graph and Q1/Q2 old-consumer evidence; review after any milestone dependency change |
| AQ-I06 | High: generic retrieval can be mistaken for package compatibility or executable trust | Runtime/package/security: Q2, with Q1 trust contract | AC07/AC11/AC12; keep origin and byte digest separate |
| AQ-I07 | Medium: new generic module could accidentally require inference or overclaim feature isolation | Core/composition: Q1/Q3/Q4 | AC09/AC15/AC17; no unsupported minimal-build claim |
| AQ-I08 | Medium: source roadmap can expand prerequisite beyond useful consumer result | Integrator: Q1 contract first; Q3 required S3 scope; future features deferred below | AC18 and gate-status record |
| AQ-I09 | Medium: hashless source comparison is not isolation from an uncooperative local writer that can modify a file in place and restore metadata before model import | Acquisition/import handoff: Q1 claim is explicitly limited; resolve through a same-owner handle/lifetime handoff or document and verify the supported local-writer threat model before any broader claim | AC05–AC07; revisit at shared durable handoff and final composed review |
| AQ-D01 | Deferred: native Xet reconstruction | Acquisition/source owner | Revisit when a required HF source cannot use the supported existing file path or Xet transfer benefits are an admitted goal; evaluate maintained Rust implementation, no homemade protocol |
| AQ-D02 | Deferred: peers and concurrent multi-source failover | Acquisition/distribution owner | Revisit with actual node/authorization contract and content-equivalence proof; no cross-origin ETag comparison |
| AQ-D03 | Deferred: chunk CAS, reflink optimization and coalescing | Acquisition/storage owner | Revisit with measured duplication/network/storage need and explicit retention consumers; ordinary files and safe release are required now |
| AQ-D04 | Scoped retained dependency: package-resolver metadata and managed-Python-provider traffic | Package/runtime owner | Q2 inventories and accepts exact scope; migrate only through supported integration when it changes a meaningful transfer claim |
| AQ-D05 | Deferred: universal remote write/search/browsing or dynamic executable source plugins | Source owner | Revisit only for an explicit product/trust/API requirement; direct HTTP/S3 object acquisition is in scope now |
| AQ-E01 | Pending: required-real AWS/non-AWS/MinIO/native/desktop evidence | Assigned source/distribution integrator | AC14/AC17; missing environment blocks those claims, not fictional acceptance |

Pending cleanup replay and unrelated whole-runtime remediation are owned by the existing Rust/library plan. Preserve current refusal while migrating the selected transfer family. Do not mark those unrelated work items accepted from this plan's link/schema checks.

Q1 source preparation confirmed a public `DownloadManager` with no in-repository production caller, but did not establish its external compatibility population; removal or a compatibility disposition remains open. The supported download-store reader currently accepts schema 5 and upgrades schema 4, while deployment inventory and older-writer isolation are unavailable. Q1 must preserve both facts and keep live-root mutation blocked until that deployment evidence exists. The native llama.cpp cache currently uses size/filename admission and direct-final-directory extraction; these are admitted Q1 repair findings under AQ-I04/AQ-I07.
