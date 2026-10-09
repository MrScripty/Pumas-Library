# Canonical model lookup v1

`PumasApi::lookup_model(model_id)` and JSON-RPC `lookup_model` are additive Full-profile operations. Existing `get_model` and `get_models` contracts remain unchanged. CatalogQuery clients continue to use their indexed `get_model` operation; the new reconciliation operation requires Full capability.

Request parameters are exactly `{ "model_id": "llm/family/name" }`. IDs are canonical relative paths: nonempty, at most 4096 UTF-8 bytes, with no empty/dot/parent segments, backslashes, drive prefixes, whitespace, or control characters. Input is rejected rather than trimmed or rewritten.

The response has `contract_version: 1`, `requested_model_id`, and a tagged `resolution`:

| Resolution | Meaning | Consumer action |
| --- | --- | --- |
| `{ "status": "found" }` | An indexed record exists at the requested ID after this read's reconciliation. This is not an inference or readiness assertion. | Read the selected record through the existing API. |
| `{ "status": "reclassified", "replacement_model_id": "unknown/family/name" }` | This read's admitted model reconciliation returned that exact replacement after a legitimate reclassification. | Refresh the catalog and deliberately select the replacement, subject to its current availability/readiness. |
| `{ "status": "missing" }` | No indexed record exists at the requested ID. | Refresh the catalog or clear the selection. Earlier moves are not reconstructed. |

For example, a container without LLM architecture evidence may legitimately move from `llm/fixture/model` to `unknown/fixture/model`. The reporting read returns the actual replacement. A later lookup of the original path returns `missing`, and the legacy `get_model` still returns `None`; no alias or redirect history is persisted.

The read uses forced admission through the existing runtime-owned model reconciliation. Current payload projections can still skip reclassification internally. Overlapping model/full reconciliation and busy download custody return the existing typed conflict (`-32011`) rather than a misleading missing outcome. Other model scopes can run concurrently. The admitted reconciliation retains existing intent-service reconciliation side effects, nested-effect drain, error handling, and shutdown ownership.

Migration facts are confined to this owned run. Scheduled dirty-event and desired-state followups are excluded. A replacement is a historical operation observation, not a guarantee the path remains available after the response. No successor record is fetched implicitly, and canonical path-based identity and classification rules are unchanged.

Generated consumers export `ModelLookupParams`, `ModelLookupOutcome`, and `decodeModelLookupOutcome`. When composing this source with another contract change, regenerate from the combined producer schemas; do not replace a generated client with either branch's snapshot.
