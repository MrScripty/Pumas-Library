# Silent model deletion refusal

The user held the delete control to completion for the 8.11 GB
`CLM-v0.1-8B-GGUF` entry. The operation left the row visible without an error.
This repair concerns deletion outcome reporting and exact model identity; it
does not change acquisition custody, locking or the reconciliation plan.

## Observed identities and state

Read-only SQLite, download-registry and directory observations distinguish two
artifacts at upstream revision `c97b04a092bc76b152faf6b57b522b766e159bec`:

| Expected payload | Catalog bytes | Artifact identity component |
| --- | --- | --- |
| `Qwen3-8B-Q8_0.gguf` | 8,709,518,976 (8.11 GiB in the current GB-labelled UI) | `files_9ee061e1214b` |
| `Qwen3-8B-Q8_0-outq2.gguf` | 8,252,495,488 (7.69 GiB) | `files_715cae0a2180` |

Both are recorded separately under `reranker/clm-v0_1/`, despite sharing their
display name and Q8_0 label. At observation, the selected 8.11 GB directory
contained only `metadata.json`; its earlier acquisition was withdrawn and its
queue admission released. The other directory contained a partial payload and
had current acquisition/admission custody. Catalog size is the expected payload
size, not evidence of bytes currently present.

The live `pumas-rpc` process held the native exclusive lock on the library-root
directory. There were no CLM intent declarations or deletion claims, unsettled
CLM admission attempts, or lifecycle quarantines in the observed stores. Two
nearby log entries reported RPC conflict code `-32011`; their method labels were
generic, so timestamps alone do not prove which requests produced them.
These observations describe a point in time, not current or future file state.
No real model files or metadata were changed, copied, hashed or deleted.

## Reproduction and cause

`pnpm exec vitest run src/hooks/useModelLibraryDelete.test.tsx` reproduced the
silent refusal through the real hold control, library-action hook and local
model list: the API received only the selected model ID, but an exception
produced no accessible error. The same-name sibling remained visible. The
minimized fixture uses a partial catalog entry without recovery information,
so an unrelated recovery warning cannot count as deletion feedback.

Ranked hypotheses were retained cancellation custody, cross-destination
contention and another library mutation. Read-only custody excludes the first
for the observed target. `LibraryMutationAuthority::acquire` requests a native
root-wide execution grant before checking destination custody. An active
transfer retains that grant; deleting another artifact can therefore return
`DownloadRootBusy` even though artifact identities differ.

A disposable repository-local fixture exercised the actual release RPC. A
native library-root lock produced the exact `-32011` conflict and left both
fixture directories intact. Releasing that lock allowed immediate deletion of
the selected directory while preserving the sibling payload. This proves the
lock precondition and exact-target deletion behavior; it does not exercise an
actual network transfer or mutate the user's library.

The frontend caught both thrown errors and unsuccessful responses but only
logged them. Separately, RPC projected `DownloadRootBusy` to a generic conflict
message. Both projections prevented useful user feedback.

## Repair contract

The existing library-action hook owns deletion state per exact model ID. It
observes each started request, prevents duplicate pending requests for that
ID, clears its error on retry/success and refreshes models only on success.
Independent model requests retain independent state. The row renders a
dedicated accessible deletion alert, separately from partial-recovery errors,
and disables its delete control while pending. No name, repository or quant
label selects a deletion target or cancels another download.

RPC retains code `-32011`, class `conflict` and the same response shape. The
specific busy-library error uses a bounded public message explaining that the
owning download or operation must finish or be cancelled before retry. Other
conflicts keep their existing generic projection. No private path, credential,
model name or raw internal error is added to that message.

Root-wide exclusion remains an existing backend constraint. This repair does
not authorize concurrent deletion during transfer, add deferred destructive
work, or relax destination/acquisition protections. It does not satisfy the
separate reconciliation plan's C1–C10 claims.

## Verification authority

Coding standards MCP snapshot
`snapshot:v1:1106bd39-e9a8-48d9-897e-8a186918dd48` selected 33 policies with no
unresolved routing categories. Existing policy content was compared with
previous reads; additional TypeScript async and Rust release/dependency
profiles were read. Production edits are limited to frontend deletion state
and projection plus the RPC public-error projection. Dependencies, generated
contracts, persistent formats and mutation authorities remain unchanged.

Focused tests cover failed responses, exceptions, absent failure detail,
pending controls, duplicate requests, independent exact identities, retries,
successful refresh and clearing errors. Component evidence proves accessible
role/name/state and projected interaction semantics; it does not qualify native
pointer capture, keyboard behavior, foreign platforms or model inference.
Completed suite and candidate evidence belongs in the
[execution ledger](../plans/download-location-reconciliation/execution-ledger.md).
