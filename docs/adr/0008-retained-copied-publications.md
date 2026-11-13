# ADR 0008: Finishing retained copied publications

Status: Accepted (2026-11-10).

Affected boundaries:

- the copied-import publication protocol: `ModelLibrary::finalize_import_index`
  and `importer::publication`;
- the `finish_retained_import` RPC;
- the catalog's `unavailable_records`.

## Context

A copied import publishes in a fixed order:

1. It writes a Pending index row and a Pending receipt.
2. It makes its single hash pass over the payload.
3. It confirms the receipt and writes Ready `metadata.json`.
4. It commits the Ready index row, but only if the row still equals the
   Pending row from step 1.

Observation never promotes a publication. A Ready document that the index
does not acknowledge stays unavailable.

On 2026-11-07 two imports (LFM3-Audio-2B and Qwen3-Reranker-0.6B, both GGUF)
ended Ready and confirmed on disk but Pending in the index. A metadata refresh
re-projected their rows during step 2, so step 4 conflicted. Every repair path
requires an acknowledged Ready row first, so nothing could finish them:

- the catalog failed on them, and before 0.8.0 took the whole library down
  with them;
- the error said they had "no supported recovery API";
- they could not be deleted from the library view.

## Decision

- **A re-projection of the same Pending publication does not strand the
  import.** When the final commit conflicts, the import re-reads the row. If
  the row still holds this publication, unacknowledged, it is no newer
  evidence about that publication, and the import commits against it, up to
  three times. Any other change still refuses the commit, as before.
- **`finish_retained_import` finishes a publication that is already
  stranded.** It is explicit, never automatic, and runs under held root
  authority. It requires all of the following:
  - the index row claims a publication and is not Ready;
  - `metadata.json` names the same publication and model, Confirmed and
    Ready;
  - the confirmed receipt matches the held payload bindings;
  - every payload file rehashes to the receipt's SHA-256.

  If they hold, it commits the Ready row from the canonical metadata.
  Otherwise it refuses with the reason; a publication that never confirmed
  must be deleted and imported again. While downloads hold the library root,
  it refuses as busy.
- **The catalog names the repair.** For a left-out record that carries a
  copied-publication diagnosis, `unavailable_records` gives that diagnosis's
  `code` and message, not the projection field it leaves missing. A finishable
  record also gets `recovery: "finish_retained_import"`. The library view
  offers "Verify and finish import" for it.

## Alternatives considered

- **Promote on observation (at listing or startup).** Rejected. It reverses
  the protocol's rule that observation never promotes a publication, and it
  would rehash payloads on every listing.
- **Drop the conflict check.** Rejected. A conflicting change can be real new
  evidence, such as a replaced payload or a different publication.
- **Rebuild the index from disk.** A cold index can already discover a
  Ready document after an independent receipt and payload check. Deleting a
  row by hand to get there is not a supported operation.

## Consequences

- Imports no longer strand when a refresh overlaps their hash pass.
- A stranded publication can be finished in place, at the cost of one full
  rehash.
- Readiness checks still observe names, sizes and file identity without
  rehashing; only this explicit operation reads the payload bytes.
