# ADR 0003: Explicit local acceptance of moved legacy download outputs

Status: Accepted design, implementation in progress.

Historical reclassification moved a live download workspace without rebinding
its admitted attempt. Existing shared mutation authority prevents recurrence,
but retained Error attempts still point to absent old destinations while the
catalog records their files at classified paths.

Keep attempt history, current local availability and verified HF completion as
separate facts. Offer explicit local acceptance of an exact artifact with all
recorded selected files present. Preserve the original Error rather than
rewriting it to Completed or guessing a historical upstream revision.

Use the existing HF lifecycle, root/destination capabilities, destination queue
owner and canonical acquisition-store transaction. Publish the disposition and
exact admission release together; settle the exact dormant live claim before
success. No second journal, path alias registry, model move, payload hash/copy or
SQL repair is required. Keep older readers from silently dropping this meaning
with a partition-owned HF format-6 discriminator, while retaining envelope 7.

The contract is [retained download reconciliation](../contracts/download-location-reconciliation.md).
The [execution plan](../plans/download-location-reconciliation/plan.md) owns pending
compatibility, composed-lifecycle, desktop and artifact acceptance evidence.
